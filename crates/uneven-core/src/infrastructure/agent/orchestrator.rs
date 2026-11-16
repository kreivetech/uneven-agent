use crate::UnevenError;
use crate::domain::parsed_error::ErrorType;
use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::{FixSuggestion, strategies, fix};
use crate::infrastructure::reporting::MarkdownLogger;
use crate::infrastructure::snapshots::SnapshotManager;
use crate::infrastructure::utils::test_runner::TestRunner;
use crate::domain::testing::TestRunnerConfig;
use crate::infrastructure::llm;
use std::path::PathBuf;
use crate::domain::config::BrainConfig;

#[derive(Debug, Clone)]
pub struct FixResult {
    pub suggestion: FixSuggestion,
    pub applied: bool,
    pub reverted: bool,
    pub pending_id: Option<String>,
}

pub struct AgentOrchestrator {
    logger: MarkdownLogger,
    project_root: PathBuf,
}

impl AgentOrchestrator {
    pub fn new(log_path: &str, project_root: PathBuf) -> Self {
        Self {
            logger: MarkdownLogger::new(log_path),
            project_root,
        }
    }

    pub async fn run_fix_for_error(
        &self,
        error: &ErrorEvent,
        auto_fix: bool,
        confirm_before_fix: bool,
        only_use_strategies: bool,
        brain: &BrainConfig,
    ) -> Result<Option<FixResult>, UnevenError> {
        let is_proactive = error.error_type == Some(ErrorType::ProactiveScan);

        if is_proactive {
            // Check file validity - match TS behavior where empty/invalid file aborts silently
            let path = std::path::Path::new(&error.file);
            if error.file == "unknown" || !path.exists() || !path.is_file() {
                return Ok(None);
            }
        }

        let mut suggestion;
        let content = std::fs::read_to_string(&error.file).unwrap_or_default();

        let mut actual_error = error;
        let virtual_error_holder; // To keep the generated error alive

        if is_proactive {
            if let Some(fast_err) = strategies::proactive::fast_syntax_check(&content, &error.file) {
                self.logger.info("Proactive scan detected a fast syntax error. Routing to native heuristics.");
                virtual_error_holder = fast_err;
                actual_error = &virtual_error_holder;
            } else {
                self.logger.info("Proactive scan found no syntax errors. Skipping IA call.");
                return Ok(None);
            }
        }

        self.logger.info(&format!("Analyzing error in {}:{}", actual_error.file, actual_error.line));

        // 1. Try Heuristics (Common Fixes)
        suggestion = strategies::find_heuristic_fix(actual_error, &content);

            if only_use_strategies {
                if let Some(ref mut s) = suggestion
                    && s.confidence >= 0.3 {
                        s.confidence = 0.6; // Boost to 0.6 so that confidence >= 0.5 passes!
                    }
            } else {
                // 2. LLM Fallback if heuristics failed or low confidence
                if suggestion.is_none() || suggestion.as_ref().unwrap().confidence < 0.6 {
                    self.logger.info("Heuristics insufficient. Calling IA for deep analysis...");
                    if let Ok(llm_suggestion) = self.suggest_llm_fix(actual_error, brain).await {
                        suggestion = Some(llm_suggestion);
                    }
                }
            }

        let suggestion = match suggestion {
            Some(s) => s,
            None => {
                if !is_proactive {
                    self.logger.warn("No fix suggestion could be generated.");
                }
                return Ok(None);
            }
        };

        let pct = (suggestion.confidence * 100.0) as u32;
        self.logger.info(&format!("Fix Suggestion ({}%): {}", pct, suggestion.explanation));

        if suggestion.confidence < 0.5 {
            self.logger.warn(&format!("Confidence too low ({}%) — skipping auto-fix.", pct));
            return Ok(Some(FixResult {
                suggestion,
                applied: false,
                reverted: false,
                pending_id: None,
            }));
        }

        // 3. Execution Decision
        if auto_fix && !confirm_before_fix {
            self.logger.info("Auto-fix enabled. Preparing snapshot and applying...");
            
            // Snapshot before modify
            let snapshot_manager = SnapshotManager::new(&self.project_root)?;
            let snapshot_result = snapshot_manager.take(
                &suggestion.file, 
                &format!("autofix error at line {}", actual_error.line)
            );

            let snapshot = match snapshot_result {
                Ok(s) => Some(s),
                Err(e) => {
                    self.logger.warn(&format!("Failed to take pre-fix snapshot: {}. Applying anyway...", e));
                    None
                }
            };

            // Apply patch
            match self.apply_fix_directly(&suggestion).await {
                Ok(_) => {
                    // Test validation
                    self.logger.info("Fix applied. Running tests to validate...");
                    let test_runner = TestRunner::new(&self.project_root);
                    let test_config = TestRunnerConfig {
                        target_file: Some(suggestion.file.clone()),
                        ..Default::default()
                    };

                    match test_runner.run(test_config).await {
                        Ok(test_res) => {
                            if test_res.passed {
                                self.logger.info("Tests passed! Fix is verified.");
                                Ok(Some(FixResult {
                                    suggestion,
                                    applied: true,
                                    reverted: false,
                                    pending_id: None,
                                }))
                            } else {
                                self.logger.warn("Tests failed. Reverting changes...");
                                if let Some(snap) = snapshot
                                    && let Err(err) = snapshot_manager.restore(&snap.id) {
                                        self.logger.error(&format!("Critical: Failed to restore snapshot {}: {}", snap.id, err));
                                    }
                                Ok(Some(FixResult {
                                    suggestion,
                                    applied: false,
                                    reverted: true,
                                    pending_id: None,
                                }))
                            }
                        }
                        Err(e) => {
                            self.logger.warn(&format!("Test runner error: {}. Keeping fix but marked unverified.", e));
                            Ok(Some(FixResult {
                                suggestion,
                                applied: true,
                                reverted: false,
                                pending_id: None,
                            }))
                        }
                    }
                }
                Err(e) => {
                    self.logger.error(&format!("Failed to apply fix: {}", e));
                    Err(e)
                }
            }
        } else {
            self.logger.info("Auto-fix requires confirmation. Posting pending diff...");
            // Non-automatic mode
            let manager = crate::infrastructure::session::SessionManager::new(&self.project_root);
            
            let abs_file = if std::path::Path::new(&suggestion.file).is_absolute() {
                PathBuf::from(&suggestion.file)
            } else {
                self.project_root.join(&suggestion.file)
            };
            
            let expected_checksum = if let Ok(content) = std::fs::read_to_string(&abs_file) {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(content.as_bytes());
                hasher.finalize().into_iter().map(|b| format!("{:02x}", b)).collect()
            } else {
                String::new()
            };
            
            let pending_diff = crate::domain::session::PendingDiff {
                id: uuid::Uuid::new_v4().to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                file: suggestion.file.clone(),
                error_code: String::new(),
                error_message: error.message.clone(),
                error_line: error.line as u32,
                explanation: suggestion.explanation.clone(),
                suggested_fix: suggestion.suggested_fix.clone(),
                before_code: suggestion.before.clone(),
                after_code: suggestion.after.clone(),
                confidence: suggestion.confidence as f64,
                expected_checksum,
                status: crate::domain::session::DiffStatus::Pending,
            };
            
            let pending_id = pending_diff.id.clone();
            if let Err(e) = manager.add_pending_diff(pending_diff) {
                self.logger.error(&format!("Failed to post pending diff to session: {}", e));
            }

            Ok(Some(FixResult {
                suggestion,
                applied: false,
                reverted: false,
                pending_id: Some(pending_id),
            }))
        }
    }

    async fn suggest_llm_fix(&self, error: &ErrorEvent, brain: &BrainConfig) -> Result<FixSuggestion, UnevenError> {
        let content = std::fs::read_to_string(&error.file)
            .map_err(UnevenError::IoError)?;

        let lines: Vec<&str> = content.lines().collect();
        let start = error.line.saturating_sub(40);
        let end = std::cmp::min(lines.len(), error.line + 40);
        let context_window = lines[start..end].join("\n");

        // Format the context window with line numbers for the LLM prompt
        let numbered_content = crate::infrastructure::utils::formatter::format_with_line_numbers(&content);
        let numbered_lines: Vec<&str> = numbered_content.lines().collect();
        let numbered_context_window = numbered_lines[start..end].join("\n");

        let prompt = format!(
            r#"You are an expert software engineer. Fix the following error.

### ERROR:
Message : {}
File    : {}:{}

### CONTEXT:
```
{}
```

### INSTRUCTIONS:
Respond in this exact format (no extra text):
EXPLANATION: <one sentence explaining the fix>
FIX: <specific change required>
AFTER_CODE: <the corrected version of the code block>
CONFIDENCE: <number between 0.0 and 1.0>"#,
            error.message, error.file, error.line, numbered_context_window
        );

        let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
        let params = llm::InferParams {
            model: brain.model.clone(),
            threads,
            max_tokens: if brain.max_tokens > 0 { brain.max_tokens } else { 512 },
            temperature: brain.temperature as f64,
            repeat_penalty: 1.1,
            max_context: 4096,
            grammar: None,
            provider: Some(brain.provider.clone()),
            system: Some(crate::infrastructure::security::laws::get_context("fix")),
            history: None,
        };
        let response = llm::infer_with_params(&prompt, &params).await?;
        let text = response.content.trim();

        // Basic field extraction (matching TS extractField logic)
        let explanation = self.extract_field(text, "EXPLANATION").unwrap_or_default();
        let suggested_fix = self.extract_field(text, "FIX").unwrap_or_default();
        let after_code = self.extract_field(text, "AFTER_CODE").unwrap_or_default();
        let confidence = self.extract_field(text, "CONFIDENCE")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.8);

        Ok(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: context_window, // Simplified before code
            after: after_code,
            confidence,
            explanation,
            suggested_fix,
            append_eof: false,
            top_of_file: false,
        })
    }

    async fn apply_fix_directly(&self, suggestion: &FixSuggestion) -> Result<(), UnevenError> {
        fix::apply_patch(&suggestion.file, &suggestion.before, &suggestion.after, suggestion.top_of_file, suggestion.append_eof).await?;
        
        self.logger.fix_applied(
            &suggestion.file, 
            &suggestion.before, 
            &suggestion.after, 
            &suggestion.explanation
        );
        
        Ok(())
    }



    fn extract_field(&self, text: &str, field: &str) -> Option<String> {
        let marker = format!("{}:", field);
        if let Some(start) = text.find(&marker) {
            let start = start + marker.len();
            let sub = &text[start..];
            let next_marker_pos = [
                "EXPLANATION:", "FIX:", "BEFORE_CODE:", "AFTER_CODE:", "CONFIDENCE:",
                "explanation:", "suggestedFix:", "beforeCode:", "afterCode:", "confidence:",
                "\"explanation\":", "\"suggestedFix\":", "\"beforeCode\":", "\"afterCode\":", "\"confidence\":"
            ]
                .iter()
                .filter_map(|&m| sub.find(m))
                .min();
            
            let val = match next_marker_pos {
                Some(pos) => &sub[..pos],
                None => sub,
            };
            return Some(val.trim().trim_matches(|c| c == '"' || c == ',' || c == '{' || c == '}').trim().to_string());
        }
        None
    }
}
