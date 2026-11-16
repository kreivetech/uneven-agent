use anyhow::Result;
use std::fs;
use std::path::Path;
use uneven_core::infrastructure::knowledge::incremental_index::IncrementalIndex;
use uneven_core::infrastructure::llm::{infer_with_params, InferParams};
use uneven_core::UnevenConfig;

pub struct ContextAssembler;

impl ContextAssembler {
    pub async fn assemble(
        question: &str,
        _config: &UnevenConfig,
        brain: &uneven_core::domain::config::BrainConfig,
        root: &Path,
        relevant_paths_override: Option<Vec<String>>,
        _history: &[crate::commands::interactive::BrainTurn],
    ) -> Result<String> {
        let mut context = String::new();
        context.push_str("## RELEVANT CONTEXT (KNOWLEDGE MAP)\n\n");

        let mut paths_to_load = Vec::new();

        if let Some(ref paths) = relevant_paths_override
            && !paths.is_empty() {
                paths_to_load = paths.clone();
            }

        if paths_to_load.is_empty() {
            // Load file list cache from the global cache directory
            let cache_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root)
                .join("file-list-cache.json");
            let mut files = Vec::new();
            if cache_path.exists()
                && let Ok(content) = fs::read_to_string(&cache_path)
                    && let Ok(parsed) = serde_json::from_str::<Vec<String>>(&content) {
                        files = parsed;
                    }

            // Fallback to index state / files list
            if files.is_empty() {
                let inc_idx = IncrementalIndex::new(root);
                files = inc_idx
                    .state
                    .files
                    .keys()
                    .filter_map(|abs_path| {
                        Path::new(abs_path)
                            .strip_prefix(root)
                            .ok()
                            .map(|p| p.to_string_lossy().to_string())
                    })
                    .collect();
            }

            // If empty, fallback to a directory scan on the fly
            if files.is_empty() {
                let scanner =
                    uneven_core::infrastructure::knowledge::scanner::ProjectScanner::new(root);
                if let Ok(scanned) = scanner.scan(vec![".".to_string()]) {
                    files = scanned;
                }
            }

            if !files.is_empty() {
                // Classify files using the brain LLM
                let files_str = files.join("\n");
                let system_prompt = "You are a software developer assistant. Your task is to analyze a list of files in the project and identify which files are relevant to answer the user's question or request.\n\n\
                Instructions:\n\
                - Return ONLY a JSON array of strings containing the relative file paths of the relevant files.\n\
                - Example: [\"src/main.rs\", \"Cargo.toml\"]\n\
                - Do NOT include markdown styling, code blocks (```json), or any explanation. Output only raw JSON.\n\
                - Select at most 5-10 files. Be precise and select only files that are directly related to the user's query.\n\
                - If no files are relevant, return [].";

                // Use the max_context defined in the brain configuration, defaulting to a provider-aware fallback
                let max_context = if brain.provider == "local" {
                    4096 // Safe default VRAM budget for local inference
                } else {
                    1000000 // Modern default for cloud APIs (OpenAI, Gemini, Anthropic, etc.)
                };

                let params = InferParams {
                    model: brain.model.clone(),
                    threads: brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4),
                    max_tokens: 20000,
                    temperature: 0.1,
                    repeat_penalty: 1.1,
                    max_context,
                    grammar: None,
                    provider: Some(brain.provider.clone()),
                    system: Some(system_prompt.to_string()),
                    history: None,
                };

                let user_msg = format!("Files:\n{}\n\nQuestion: {}", files_str, question);
                if let Ok(resp) = infer_with_params(&user_msg, &params).await {
                    paths_to_load = parse_classifier_response(&resp.content, &files);
                }
            }
        }

        if paths_to_load.is_empty() {
            context.push_str("(No relevant context found or identified)\n\n");
        } else {
            for rel_path in &paths_to_load {
                let abs_path = root.join(rel_path);
                if abs_path.exists() && abs_path.is_file() {
                    if is_binary_file(&abs_path) {
                        continue;
                    }
                    if let Ok(content) = fs::read_to_string(&abs_path) {
                        // Limit individual file character count to protect context window size
                        let char_limit = 100_000;
                        let (display_content, truncated) = if content.len() > char_limit {
                            (format!("{}\n", &content[..char_limit]), true)
                        } else {
                            (content, false)
                        };

                        context.push_str(&format!("**File: {}**\n```\n", rel_path));
                        let numbered =
                            uneven_core::infrastructure::utils::formatter::format_with_line_numbers(
                                &display_content,
                            );
                        context.push_str(&numbered);
                        if truncated {
                            context
                                .push_str("... (rest of file truncated to save context window)\n");
                        }
                        context.push_str("```\n\n");
                    }
                }
            }
        }

        Ok(context)
    }
}

fn parse_classifier_response(response: &str, all_paths: &[String]) -> Vec<String> {
    let cleaned = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(paths) = serde_json::from_str::<Vec<String>>(cleaned) {
        return paths
            .into_iter()
            .filter(|p| all_paths.contains(p))
            .collect();
    }

    // Fallback: search for any of the known file paths in the raw response
    let mut detected = Vec::new();
    for path in all_paths {
        if response.contains(path) {
            detected.push(path.clone());
        }
    }
    detected
}

fn is_binary_file(path: &Path) -> bool {
    use std::io::Read;
    if let Ok(mut file) = fs::File::open(path) {
        let mut buffer = [0; 1024];
        if let Ok(n) = file.read(&mut buffer) {
            for &b in buffer.iter().take(n) {
                if b == 0 {
                    return true;
                }
            }
        }
    }
    false
}
