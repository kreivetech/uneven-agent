use super::context_assembler::ContextAssembler;
use crate::domain::config::BrainConfig;
use crate::infrastructure::llm::inference::chat_template::{apply_template, detect_template};
use crate::infrastructure::llm::{
    infer_with_params_streaming, init as init_llm, InferParams,
};
use crate::infrastructure::patch::{clean_chat_proposal, file_object, FileObject};
use crate::infrastructure::security::laws;
use crate::infrastructure::security::SafetyGuard;
use crate::UnevenConfig;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Progress phases of the multi-turn reflective reasoning loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThoughtPhase {
    ResolvingContext,
    Drafting,
    RubberDuckingDraft,
    Auditing,
    VerifyingIntegrity,
    RubberDuckingSimulation,
    Synthesizing,
    Completed,
}

/// The structured outcome of a full `think` reasoning cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct ThoughtResult {
    pub final_response: String,
    pub draft_response: String,
    pub duck_draft_reflection: String,
    pub critic_findings: String,
    pub integrity_findings: String,
    pub duck_simulation_trace: String,
    pub file_object: Option<FileObject>,
    pub commands: Vec<String>,
}

/// Parses all [NEED_FILE: path] tags from an agent response.
pub fn parse_need_file_tags(text: &str) -> HashSet<String> {
    let mut files = HashSet::new();
    let mut start_idx = 0;
    while let Some(open_pos) = text[start_idx..].find("[NEED_FILE:") {
        let abs_open = start_idx + open_pos;
        if let Some(close_pos) = text[abs_open..].find(']') {
            let tag_content = &text[abs_open + 11..abs_open + close_pos];
            let clean_path = tag_content.trim().trim_matches('`').trim();
            if !clean_path.is_empty() {
                files.insert(clean_path.to_string());
            }
            start_idx = abs_open + close_pos + 1;
        } else {
            break;
        }
    }
    files
}

/// Parses all [EXEC_CMD: command] tags from an agent response.
pub fn parse_exec_cmd_tags(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut start_idx = 0;
    while let Some(open_pos) = text[start_idx..].find("[EXEC_CMD:") {
        let abs_open = start_idx + open_pos;
        if let Some(close_pos) = text[abs_open..].find(']') {
            let tag_content = &text[abs_open + 10..abs_open + close_pos];
            let clean_cmd = tag_content.trim().trim_matches('`').trim();
            if !clean_cmd.is_empty() && !commands.contains(&clean_cmd.to_string()) {
                commands.push(clean_cmd.to_string());
            }
            start_idx = abs_open + close_pos + 1;
        } else {
            break;
        }
    }
    commands
}

/// Strips [EXEC_CMD: ...] tags from displayed markdown text.
pub fn strip_exec_cmd_tags(text: &str) -> String {
    let mut result = text.to_string();
    while let Some(start) = result.find("[EXEC_CMD:") {
        if let Some(end) = result[start..].find(']') {
            let full_tag = &result[start..=start + end];
            result = result.replace(full_tag, "").trim().to_string();
        } else {
            break;
        }
    }
    result
}

/// Reads a workspace file and formats it as a markdown code block.
fn read_workspace_file(root: &Path, rel_path: &str) -> Option<String> {
    let target = root.join(rel_path);
    if target.exists() && target.is_file()
        && let Ok(content) = std::fs::read_to_string(&target) {
            let ext = target.extension().and_then(|e| e.to_str()).unwrap_or("text");
            return Some(format!(
                "### File: {}\n```{}\n{}\n```\n\n",
                rel_path, ext, content
            ));
        }
    None
}

/// Executes a single inference turn against the LLM provider.
pub async fn run_single_turn(
    prompt: &str,
    system_instructions: &str,
    brain: &BrainConfig,
    threads: u32,
    max_context: usize,
) -> Result<String> {
    let chat_template = detect_template(&brain.model);
    let formatted_prompt = apply_template(&chat_template, system_instructions, &[], prompt);

    let prompt_to_use = if brain.provider == "local" {
        formatted_prompt
    } else {
        prompt.to_string()
    };

    let params = InferParams {
        model: brain.model.clone(),
        threads,
        max_tokens: brain.max_tokens,
        temperature: brain.temperature as f64,
        repeat_penalty: 1.1,
        max_context,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: Some(system_instructions.to_string()),
        history: None,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
    let prompt_clone = prompt_to_use.clone();
    let params_clone = InferParams {
        model: params.model.clone(),
        threads: params.threads,
        max_tokens: params.max_tokens,
        temperature: params.temperature,
        repeat_penalty: params.repeat_penalty,
        max_context: params.max_context,
        grammar: params.grammar.clone(),
        provider: params.provider.clone(),
        system: params.system.clone(),
        history: params.history.clone(),
    };

    let inference_handle = tokio::spawn(async move {
        infer_with_params_streaming(&prompt_clone, &params_clone, move |token| {
            let _ = tx.try_send(token);
        })
        .await
    });

    let mut turn_response = String::new();
    while let Some(token) = rx.recv().await {
        turn_response.push_str(&token);
    }

    match inference_handle.await {
        Ok(Ok(r)) => Ok(r.content),
        Ok(Err(e)) => Err(e.into()),
        Err(e) => Err(anyhow::anyhow!("Inference panicked: {}", e)),
    }
}

/// Executes a full 4-turn reflective reasoning loop:
/// Context Resolution ➔ Turn 1: Draft ➔ Turn 2: Critic (18-point audit) ➔ Turn 3: Scope & Code Integrity ➔ Turn 4: Synthesis
pub async fn execute_thought_loop<F>(
    task: &str,
    files: &[String],
    config: &UnevenConfig,
    root: &Path,
    provider_opt: Option<&str>,
    model_opt: Option<&str>,
    progress_callback: Option<F>,
) -> Result<ThoughtResult>
where
    F: FnMut(ThoughtPhase) + Send + Sync,
{
    execute_thought_loop_with_history(
        task,
        files,
        config,
        root,
        provider_opt,
        model_opt,
        progress_callback,
        None,
        None,
    )
    .await
}

/// Executes a full 4-turn reflective reasoning loop with optional custom system laws:
/// Context Resolution ➔ Turn 1: Draft ➔ Turn 2: Critic (18-point audit) ➔ Turn 3: Scope & Code Integrity ➔ Turn 4: Synthesis
#[allow(clippy::too_many_arguments)]
pub async fn execute_thought_loop_with_system<F>(
    task: &str,
    files: &[String],
    config: &UnevenConfig,
    root: &Path,
    provider_opt: Option<&str>,
    model_opt: Option<&str>,
    progress_callback: Option<F>,
    system_override: Option<&str>,
) -> Result<ThoughtResult>
where
    F: FnMut(ThoughtPhase) + Send + Sync,
{
    execute_thought_loop_with_history(
        task,
        files,
        config,
        root,
        provider_opt,
        model_opt,
        progress_callback,
        system_override,
        None,
    )
    .await
}

/// Executes a full 4-turn reflective reasoning loop with optional custom system laws and conversation history:
/// Context Resolution ➔ Turn 1: Draft ➔ Turn 2: Critic (18-point audit) ➔ Turn 3: Scope & Code Integrity ➔ Turn 4: Synthesis
#[allow(clippy::too_many_arguments)]
pub async fn execute_thought_loop_with_history<F>(
    task: &str,
    files: &[String],
    config: &UnevenConfig,
    root: &Path,
    provider_opt: Option<&str>,
    model_opt: Option<&str>,
    mut progress_callback: Option<F>,
    system_override: Option<&str>,
    history: Option<&[crate::infrastructure::llm::history_manager::BrainTurn]>,
) -> Result<ThoughtResult>
where
    F: FnMut(ThoughtPhase) + Send + Sync,
{
    // 1. Safety Guard Validation (Shield)
    let safety = SafetyGuard::new();
    safety.validate_intent(task)?;

    // 2. Select and configure brain
    let mut brain = config.get_brain_for_command("think");
    if brain.provider.is_empty() || brain.provider == "default" {
        brain = config.get_brain_for_command("ask");
    }
    if let Some(p) = provider_opt {
        brain.provider = p.to_string();
    }
    if let Some(m) = model_opt {
        brain.model = m.to_string();
    }

    // 3. Assemble Initial Context
    let files_vec = files.to_vec();
    let initial_context = ContextAssembler::assemble(task, config, &brain, root, Some(files_vec.clone()), None)
        .await
        .unwrap_or_else(|_| "(No relevant context found)\n\n".to_string());

    // 4. Initialize LLM
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    init_llm(&brain.provider, &brain.model, threads).await?;

    let max_context = if brain.provider == "local" {
        4096
    } else {
        1000000
    };

    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::ResolvingContext);
    }

    // 5. Context Resolution Loop (Iteratively read workspace files requested via [NEED_FILE])
    let mut loaded_files = HashSet::new();
    let mut file_content_cache = HashMap::new();
    let mut loaded_skills: HashSet<String> = HashSet::new();

    for f in files {
        if let Some(formatted) = read_workspace_file(root, f) {
            loaded_files.insert(f.clone());
            file_content_cache.insert(f.clone(), formatted);
        }
    }

    let mut context_resolution_attempts = 2;
    while context_resolution_attempts > 0 {
        context_resolution_attempts -= 1;

        let mut current_map = String::new();
        current_map.push_str("## CODEBASE CONTEXT\n\n");
        if file_content_cache.is_empty() {
            current_map.push_str(&initial_context);
            current_map.push('\n');
        } else {
            for content in file_content_cache.values() {
                current_map.push_str(content);
            }
        }

        let available_skills_summary = crate::domain::skills::list_builtin_skills()
            .iter()
            .map(|(name, desc)| format!("- `{}`: {}", name, desc))
            .collect::<Vec<_>>()
            .join("\n");

        let scout_prompt = format!(
            "{}\nUser Request: {}\n\nAvailable Specialized Skills:\n{}\n\nIf you need additional workspace files, emit [NEED_FILE: path]. If you need specialized domain guidelines, emit [USE_SKILL: name]. Otherwise output READY.",
            current_map, task, available_skills_summary
        );

        let scout_system = "You are a senior engineer scouting workspace files and specialized skills. Output only [NEED_FILE: path] or [USE_SKILL: name] tags or READY.";
        if let Ok(scout_resp) = run_single_turn(&scout_prompt, scout_system, &brain, threads, max_context).await {
            let needed_skills = crate::domain::skills::parse_use_skill_tags(&scout_resp);
            loaded_skills.extend(needed_skills);

            let needed = parse_need_file_tags(&scout_resp);
            let new_files: HashSet<String> = needed.difference(&loaded_files).cloned().collect();
            if new_files.is_empty() {
                break;
            }
            for path in new_files {
                if let Some(formatted) = read_workspace_file(root, &path) {
                    file_content_cache.insert(path.clone(), formatted);
                }
                loaded_files.insert(path);
            }
        } else {
            break;
        }
    }

    // Build the consolidated context block for reasoning
    let mut context_block = String::new();
    context_block.push_str("## RELEVANT CONTEXT & CODEBASE MAP\n\n");
    if file_content_cache.is_empty() {
        context_block.push_str(&initial_context);
        context_block.push('\n');
    } else {
        for content in file_content_cache.values() {
            context_block.push_str(content);
        }
    }

    // Assemble operational skill guidelines if any were activated
    let mut skill_guidelines = String::new();
    for skill_name in &loaded_skills {
        if let Some(instructions) = crate::domain::skills::get_builtin_skill(skill_name) {
            skill_guidelines.push_str(&format!(
                "\n\n---\n## OPERATIONAL GUIDELINES: {}\n{}\n---\n",
                skill_name.to_uppercase(),
                instructions
            ));
        }
    }

    // Assemble recent conversation history if provided
    let mut conversation_context = String::new();
    if let Some(hist) = history {
        let recent_turns: Vec<_> = hist.iter().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
        if !recent_turns.is_empty() {
            conversation_context.push_str("### Recent Conversation History:\n");
            for turn in recent_turns {
                let role = if turn.role == "user" { "User" } else { "Assistant" };
                conversation_context.push_str(&format!("{}: {}\n\n", role, turn.content.trim()));
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // TURN 1: Draft Proposal (Actor)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::Drafting);
    }

    let mut draft_base = system_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| laws::get_context("think_draft"));
    if !skill_guidelines.is_empty() {
        draft_base.push_str(&skill_guidelines);
    }

    let draft_system = format!(
        "{}\n\n[PHASE: DRAFTING - Produce an initial concrete solution, architecture plan, and preliminary diffs.]",
        draft_base
    );
    let draft_prompt = if !conversation_context.is_empty() {
        format!(
            "{}\n{}\nUser Request: {}\n\nFormulate a preliminary solution plan and the necessary code changes.",
            context_block, conversation_context, task
        )
    } else {
        format!(
            "{}\nUser Request: {}\n\nFormulate a preliminary solution plan and the necessary code changes.",
            context_block, task
        )
    };

    let draft_response = run_single_turn(&draft_prompt, &draft_system, &brain, threads, max_context)
        .await
        .map_err(|e| anyhow::anyhow!("Draft phase failed: {}", e))?;

    let draft_skills = crate::domain::skills::parse_use_skill_tags(&draft_response);
    for s in draft_skills {
        if loaded_skills.insert(s.clone())
            && let Some(instructions) = crate::domain::skills::get_builtin_skill(&s) {
                skill_guidelines.push_str(&format!(
                    "\n\n---\n## OPERATIONAL GUIDELINES: {}\n{}\n---\n",
                    s.to_uppercase(),
                    instructions
                ));
            }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // INTERMEDIATE 1: Rubber Duck Problem Decomposition (Unpacking Draft)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::RubberDuckingDraft);
    }

    let duck_draft_system = laws::get_context("think_duck_draft");
    let duck_draft_prompt = format!(
        "### User Original Request:\n{}\n\n### Proposed Solution Draft:\n{}\n\n\
        In 3 to 4 concise technical bullet points, explain your draft to your Rubber Duck:\n\
        1. Core Mechanism: What approach was chosen and why over alternatives?\n\
        2. Implicit Assumptions: What types, system states, or caller contracts are assumed?\n\
        3. Fragility & Edge Cases: Where is this most vulnerable to failure under unexpected conditions?",
        task, draft_response
    );

    let duck_draft_reflection = run_single_turn(&duck_draft_prompt, &duck_draft_system, &brain, threads, max_context)
        .await
        .unwrap_or_else(|_| "Rubber Duck Draft reflection unavailable.".to_string());

    // ─────────────────────────────────────────────────────────────────────────────
    // TURN 2: Internal Critic & Contextual Audit (The Critic Loop)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::Auditing);
    }

    let critic_system = laws::get_context("think_audit");

    let critic_prompt = if !conversation_context.is_empty() {
        format!(
            "{}\n### User Request:\n{}\n\n### Codebase Context:\n{}\n\n### Initial Solution Draft:\n{}\n\n### Rubber Duck Self-Reflection:\n{}\n\nPerform the contextual self-critique and audit, enumerating any required corrections.",
            conversation_context, task, context_block, draft_response, duck_draft_reflection
        )
    } else {
        format!(
            "### User Original Request:\n{}\n\n### Codebase Context:\n{}\n\n### Initial Solution Draft:\n{}\n\n### Rubber Duck Self-Reflection:\n{}\n\nPerform the contextual self-critique and audit, enumerating any required corrections.",
            task, context_block, draft_response, duck_draft_reflection
        )
    };

    let critic_findings = run_single_turn(&critic_prompt, &critic_system, &brain, threads, max_context)
        .await
        .map_err(|e| anyhow::anyhow!("Critic audit phase failed: {}", e))?;

    // ─────────────────────────────────────────────────────────────────────────────
    // TURN 3: Scope & Code Integrity Guard (Anti-Regression & Anti-Hallucination)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::VerifyingIntegrity);
    }

    let integrity_system = laws::get_context("think_integrity");

    let integrity_prompt = if !conversation_context.is_empty() {
        format!(
            "{}\n### User Request:\n{}\n\n### Codebase Context:\n{}\n\n### Proposed Solution Draft:\n{}\n\n### Critic Audit Findings:\n{}\n\nPerform the Scope & Code Integrity Check and provide your verdict.",
            conversation_context, task, context_block, draft_response, critic_findings
        )
    } else {
        format!(
            "### User Original Request:\n{}\n\n### Codebase Context:\n{}\n\n### Proposed Solution Draft:\n{}\n\n### Critic Audit Findings:\n{}\n\nPerform the Scope & Code Integrity Check and provide your verdict.",
            task, context_block, draft_response, critic_findings
        )
    };

    let integrity_findings = run_single_turn(&integrity_prompt, &integrity_system, &brain, threads, max_context)
        .await
        .map_err(|e| anyhow::anyhow!("Integrity verification phase failed: {}", e))?;

    // ─────────────────────────────────────────────────────────────────────────────
    // INTERMEDIATE 2: Rubber Duck Pre-Flight Execution Simulation (Dry-Run Walkthrough)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::RubberDuckingSimulation);
    }

    let duck_sim_system = laws::get_context("think_duck_simulation");
    let duck_sim_prompt = format!(
        "### User Original Request:\n{}\n\n### Proposed Solution Draft:\n{}\n\n### Critic Audit Findings:\n{}\n\n### Scope & Integrity Directives:\n{}\n\n\
        Walk through the execution flow with your Rubber Duck in 3 to 4 concise bullet points:\n\
        1. Step-by-Step Runtime Trace: Trace execution from entry to exit with expected types/returns.\n\
        2. Reconciliation Check: Confirm all Critic & Integrity points are accounted for.\n\
        3. Import & Contract Defense: Ensure no missing imports or broken external callers.",
        task, draft_response, critic_findings, integrity_findings
    );

    let duck_simulation_trace = run_single_turn(&duck_sim_prompt, &duck_sim_system, &brain, threads, max_context)
        .await
        .unwrap_or_else(|_| "Rubber Duck Simulation trace unavailable.".to_string());

    // ─────────────────────────────────────────────────────────────────────────────
    // TURN 4: Synthesis & Final Assured Patch
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::Synthesizing);
    }

    let mut synthesis_base = system_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| laws::get_context("think_synthesis"));
    if !skill_guidelines.is_empty() {
        synthesis_base.push_str(&skill_guidelines);
    }

    let synthesis_system = format!(
        "{}\n\n[PHASE: FINAL SYNTHESIS - Eliminate all issues flagged by the Critic and enforce Scope & Code Integrity. Deliver the final proposal and structured Markdown patches.]",
        synthesis_base
    );

    let synthesis_prompt = if !conversation_context.is_empty() {
        format!(
            "{}\n### User Request:\n{}\n\n### Context:\n{}\n\n### Initial Draft:\n{}\n\n### Internal Critic Audit Findings:\n{}\n\n### Scope & Code Integrity Directives:\n{}\n\n### Pre-Flight Rubber Duck Simulation:\n{}\n\n\
            Produce the FINAL, flawless response for the user in clean Markdown:\n\
            1. Human-Facing Overview & Explanations: Formulate naturally in the user's primary language without robotic, bureaucratic, or unnatural phrasing.\n\
            2. Technical & Code Discipline: All source code, diff hunks, functions, variable names, and comments MUST strictly remain in English.\n\
            3. Adaptive Code Mutation & Presentation Strategy:\n\
               - If executing an approved plan or follow-up confirmation from the conversation: output the exact, surgical Markdown file actions ('Action: create' with complete content or 'Action: edit' with SEARCH/REPLACE blocks) ready for immediate disk application. Do NOT ask for confirmation again.\n\
               - For micro-mutations: output exact surgical Markdown file actions.\n\
               - For medium/large multi-file architecture tasks proposed for the first time without prior approval: present the Architectural Blueprint first and ask naturally if the user would like to proceed.\n\
               - For pure conceptual Q&A: do not output any file actions.\n\
            4. Terminal Commands Execution: When a task benefits from executing terminal commands natively (such as project bootstrapping, dependency management, compilation checks, or running test suites with the project's native toolchain), emit `[EXEC_CMD: <command>]`. Prefer using the standard ecosystem CLI toolchain of the target technology for scaffolding rather than manually synthesizing boilerplate files.",
            conversation_context, task, context_block, draft_response, critic_findings, integrity_findings, duck_simulation_trace
        )
    } else {
        format!(
            "### User Request:\n{}\n\n### Context:\n{}\n\n### Initial Draft:\n{}\n\n### Internal Critic Audit Findings:\n{}\n\n### Scope & Code Integrity Directives:\n{}\n\n### Pre-Flight Rubber Duck Simulation:\n{}\n\n\
            Produce the FINAL, flawless response for the user in clean Markdown:\n\
            1. Human-Facing Overview & Explanations: Formulate naturally in the user's primary language without robotic, bureaucratic, or unnatural phrasing.\n\
            2. Technical & Code Discipline: All source code, diff hunks, functions, variable names, and comments MUST strictly remain in English.\n\
            3. Adaptive Code Mutation & Presentation Strategy:\n\
               - If executing an approved plan or follow-up confirmation: output exact, surgical Markdown file actions ('Action: create' with complete content or 'Action: edit' with SEARCH/REPLACE blocks) ready for disk application. Do NOT ask for confirmation again.\n\
               - For micro-mutations: output exact surgical Markdown file actions.\n\
               - For medium/large multi-file architecture tasks proposed for the first time without prior approval: present the Architectural Blueprint first and ask naturally if the user would like to proceed.\n\
               - For pure conceptual Q&A: do not output any file actions.\n\
            4. Terminal Commands Execution: When a task benefits from executing terminal commands natively (such as project bootstrapping, dependency management, compilation checks, or running test suites with the project's native toolchain), emit `[EXEC_CMD: <command>]`. Prefer using the standard ecosystem CLI toolchain of the target technology for scaffolding rather than manually synthesizing boilerplate files.",
            task, context_block, draft_response, critic_findings, integrity_findings, duck_simulation_trace
        )
    };

    let final_answer = run_single_turn(&synthesis_prompt, &synthesis_system, &brain, threads, max_context)
        .await
        .map_err(|e| anyhow::anyhow!("Synthesis phase failed: {}", e))?;

    // 6. Sanitize response with Security Guard
    let sanitized_answer = safety.sanitize_response(&final_answer);

    // 7. Autonomously provision any specialized pods created by the AI
    let _ = crate::domain::pod::AgentPod::process_autonomous_pod_creations(root, &sanitized_answer);

    // 8. Parse patch objects and CLI command tags if present
    let session_id = uuid::Uuid::new_v4().to_string();
    let parsed_file_obj = file_object(&sanitized_answer, &session_id);
    let parsed_commands = parse_exec_cmd_tags(&sanitized_answer);

    // 9. Clean chat proposal for human readability (strips internal skill tags, exec tags, and replaces machine SEARCH/REPLACE blocks with clean diffs)
    let clean_output = crate::domain::skills::strip_use_skill_tags(&sanitized_answer);
    let clean_output = strip_exec_cmd_tags(&clean_output);
    let display_proposal = if parsed_file_obj.is_some() {
        clean_chat_proposal(&clean_output)
    } else {
        clean_output
    };

    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::Completed);
    }

    Ok(ThoughtResult {
        final_response: display_proposal,
        draft_response,
        duck_draft_reflection,
        critic_findings,
        integrity_findings,
        duck_simulation_trace,
        file_object: parsed_file_obj,
        commands: parsed_commands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_need_file_tags() {
        let text = "I need [NEED_FILE: src/main.rs] and [NEED_FILE: Cargo.toml] to inspect.";
        let tags = parse_need_file_tags(text);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains("src/main.rs"));
        assert!(tags.contains("Cargo.toml"));
    }

    #[test]
    fn test_parse_need_file_tags_empty() {
        let text = "I am ready with no files needed.";
        let tags = parse_need_file_tags(text);
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_exec_cmd_tags() {
        let text = "First initialize: [EXEC_CMD: cargo init] then run tests: [EXEC_CMD: cargo test].";
        let cmds = parse_exec_cmd_tags(text);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0], "cargo init");
        assert_eq!(cmds[1], "cargo test");
    }

    #[test]
    fn test_strip_exec_cmd_tags() {
        let text = "Executing setup [EXEC_CMD: cargo init] now.";
        let stripped = strip_exec_cmd_tags(text);
        assert_eq!(stripped, "Executing setup  now.");
    }

    #[test]
    fn test_thought_phases_and_result() {
        let phases = [
            ThoughtPhase::ResolvingContext,
            ThoughtPhase::Drafting,
            ThoughtPhase::RubberDuckingDraft,
            ThoughtPhase::Auditing,
            ThoughtPhase::VerifyingIntegrity,
            ThoughtPhase::RubberDuckingSimulation,
            ThoughtPhase::Synthesizing,
            ThoughtPhase::Completed,
        ];
        assert_eq!(phases.len(), 8);

        let res = ThoughtResult {
            final_response: "Final".to_string(),
            draft_response: "Draft".to_string(),
            duck_draft_reflection: "Duck 1".to_string(),
            critic_findings: "Critic".to_string(),
            integrity_findings: "Integrity".to_string(),
            duck_simulation_trace: "Duck 2".to_string(),
            file_object: None,
            commands: vec![],
        };
        assert_eq!(res.duck_draft_reflection, "Duck 1");
        assert_eq!(res.duck_simulation_trace, "Duck 2");
    }
}
