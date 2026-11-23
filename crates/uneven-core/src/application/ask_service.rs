use super::context_assembler::ContextAssembler;
use super::think_service::{
    absorb_stage_directives, append_skills, CmdApprovalFn, DirectiveState,
};
use crate::domain::directives::strip_directives;
use crate::infrastructure::llm::history_manager::{
    get_project_files_map, load_or_build_files_map, prepare_structured_prompt, BrainTurn,
};
use crate::infrastructure::llm::{infer_with_params, init as init_llm, InferParams};
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
    history: Option<&mut Vec<BrainTurn>>,
    system_override: Option<&str>,
) -> Result<AskResult> {
    execute_ask_with_options(
        question,
        files,
        config,
        root,
        provider_opt,
        model_opt,
        history,
        system_override,
        None,
    )
    .await
}

/// Executes a contextual `ask` inquiry with optional custom system laws and `[EXEC_CMD]` approval callback.
#[allow(clippy::too_many_arguments)]
pub async fn execute_ask_with_options(
    question: &str,
    files: &[String],
    config: &UnevenConfig,
    root: &Path,
    provider_opt: Option<&str>,
    model_opt: Option<&str>,
    mut history: Option<&mut Vec<BrainTurn>>,
    system_override: Option<&str>,
    cmd_approval: Option<CmdApprovalFn>,
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
        .unwrap_or_else(|| laws::get_context_in_root("ask", Some(root)));
    let base_law_with_directives = crate::domain::directives::with_directives_protocol(&base_law);

    let system_instructions = format!(
        "{}\n\n[COMMUNICATION DISCIPLINE]:\n\
        1. Human-Facing Response: Deliver all explanations, reasoning, and conceptual walkthroughs naturally in the user's primary language (e.g. Portuguese if asked in Portuguese).\n\
        2. Technical Precision: All code keywords, variable names, functions, structs, and file paths MUST strictly remain in English.",
        base_law_with_directives
    );

    let raw_prompt = format!(
        "## CODEBASE CONTEXT\n{}\n\n## USER QUESTION\n{}",
        context, question
    );

    // 5. Optimize Context and run unbounded directive resolution loop until model finishes
    let files_index = load_or_build_files_map(root);
    let files_map = get_project_files_map(config, root);
    let mut tool_history: Vec<BrainTurn> = if let Some(ref mut hist) = history {
        crate::application::ContextCompactorService::auto_compact(hist);
        hist.clone()
    } else {
        Vec::new()
    };

    let mut directive_state = DirectiveState {
        loaded_files: files.iter().cloned().collect(),
        ..Default::default()
    };
    let mut current_prompt = raw_prompt;
    let mut raw_response;

    loop {
        let sys_with_skills = append_skills(&system_instructions, &directive_state.skill_guidelines);
        let prepared = prepare_structured_prompt(
            &tool_history,
            &current_prompt,
            &sys_with_skills,
            Some(&files_map),
        );
        let params = InferParams {
            model: brain.model.clone(),
            threads,
            max_tokens: brain.max_tokens,
            temperature: brain.temperature as f64,
            repeat_penalty: 1.1,
            max_context,
            grammar: None,
            provider: Some(brain.provider.clone()),
            system: prepared.system.or(Some(sys_with_skills)),
            history: if prepared.history.is_empty() {
                None
            } else {
                Some(prepared.history)
            },
        };

        raw_response = infer_with_params(&prepared.prompt, &params).await?.content;

        let mut next_turns = Vec::new();
        let added = absorb_stage_directives(
            &raw_response,
            root,
            &files_index,
            &mut directive_state,
            &safety,
            cmd_approval.as_ref(),
            &mut next_turns,
        )
        .await;

        if !added {
            break;
        }

        tool_history.push(BrainTurn {
            role: "user".to_string(),
            content: current_prompt,
        });
        tool_history.push(BrainTurn {
            role: "assistant".to_string(),
            content: raw_response,
        });
        current_prompt = next_turns
            .into_iter()
            .map(|t| t.content)
            .collect::<Vec<_>>()
            .join("\n\n");
        if current_prompt.trim().is_empty() {
            current_prompt = "Continue answering the user question with the loaded guidelines.".to_string();
        }
    }

    // 7. Sanitize response with Security Guard and strip directive tags
    let sanitized_response = strip_directives(&safety.sanitize_response(&raw_response));

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
