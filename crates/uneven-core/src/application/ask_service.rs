use super::context_assembler::ContextAssembler;
use super::think_service::run_single_turn;
use crate::infrastructure::llm::history_manager::{
    get_project_files_map, optimize_context, BrainTurn,
};
use crate::infrastructure::llm::init as init_llm;
use crate::infrastructure::security::laws;
use crate::infrastructure::security::SafetyGuard;
use crate::UnevenConfig;
use anyhow::Result;
use std::path::Path;

/// Result of an `ask` inquiry turn.
#[derive(Debug, Clone, PartialEq)]
pub struct AskResult {
    pub response: String,
    pub files_used: Vec<String>,
}

/// Executes a single-turn contextual question answering inquiry (`ask`).
/// 
/// Leverages the interactive shell's optimize_context engine for token-efficient
/// system laws injection, workspace file map tracking, and persistent session memory.
pub async fn execute_ask(
    question: &str,
    files: &[String],
    config: &UnevenConfig,
    root: &Path,
    provider_opt: Option<&str>,
    model_opt: Option<&str>,
    history: Option<&mut Vec<BrainTurn>>,
) -> Result<AskResult> {
    execute_ask_with_system(
        question,
        files,
        config,
        root,
        provider_opt,
        model_opt,
        history,
        None,
    )
    .await
}

/// Executes a single-turn contextual question answering inquiry (`ask`) with optional custom system laws.
#[allow(clippy::too_many_arguments)]
pub async fn execute_ask_with_system(
    question: &str,
    files: &[String],
    config: &UnevenConfig,
    root: &Path,
    provider_opt: Option<&str>,
    model_opt: Option<&str>,
    mut history: Option<&mut Vec<BrainTurn>>,
    system_override: Option<&str>,
) -> Result<AskResult> {
    // 1. Safety Guard Validation (Shield)
    let safety = SafetyGuard::new();
    if let Err(err) = safety.validate_intent(question) {
        return Ok(AskResult {
            response: format!("⚠️ [SECURITY POLICY]: {}", err),
            files_used: Vec::new(),
        });
    }

    // 2. Resolve brain configuration
    let mut brain = config.get_brain_for_command("ask");
    if let Some(p) = provider_opt {
        brain.provider = p.to_string();
    }
    if let Some(m) = model_opt {
        brain.model = m.to_string();
    }

    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    init_llm(&brain.provider, &brain.model, threads).await?;

    let max_context = if brain.provider == "local" {
        4096
    } else {
        1000000
    };

    // 3. Assemble Codebase Context
    let paths_override = if files.is_empty() {
        None
    } else {
        Some(files.to_vec())
    };
    let context = ContextAssembler::assemble(question, config, &brain, root, paths_override, None)
        .await
        .unwrap_or_else(|_| "(No relevant codebase context found)\n\n".to_string());

    // 4. Instructions & Prompt with Native Bilingual Protocol
    let base_law = system_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| laws::get_context("ask"));

    let system_instructions = format!(
        "{}\n\n[COMMUNICATION DISCIPLINE]:\n\
        1. Human-Facing Response: Deliver all explanations, reasoning, and conceptual walkthroughs naturally in the user's primary language (e.g. Portuguese if asked in Portuguese).\n\
        2. Technical Precision: All code keywords, variable names, functions, structs, and file paths MUST strictly remain in English.",
        base_law
    );

    let raw_prompt = format!(
        "## CODEBASE CONTEXT\n{}\n\n## USER QUESTION\n{}",
        context, question
    );

    // 5. Optimize Context (Auto-compact history if needed and eliminate redundant laws/maps)
    let prompt = if let Some(ref mut hist) = history {
        crate::application::ContextCompactorService::auto_compact(hist);
        let files_map = get_project_files_map(config, root);
        optimize_context(hist, &raw_prompt, &system_instructions, Some(&files_map))
    } else {
        raw_prompt
    };

    // 6. Execute inference turn
    let raw_response = run_single_turn(&prompt, &system_instructions, &brain, threads, max_context).await?;

    // 7. Sanitize response with Security Guard
    let sanitized_response = safety.sanitize_response(&raw_response);

    // 8. Update conversation history
    if let Some(ref mut hist) = history {
        hist.push(BrainTurn {
            role: "user".to_string(),
            content: question.to_string(),
        });
        hist.push(BrainTurn {
            role: "assistant".to_string(),
            content: sanitized_response.clone(),
        });
    }

    Ok(AskResult {
        response: sanitized_response,
        files_used: files.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ask_result_creation() {
        let res = AskResult {
            response: "Unit test response".to_string(),
            files_used: vec!["src/main.rs".to_string()],
        };
        assert_eq!(res.files_used.len(), 1);
        assert_eq!(res.response, "Unit test response");
    }
}
