use super::context_assembler::ContextAssembler;
use crate::domain::config::BrainConfig;
use crate::domain::directives::{
    parse_directives, resolve_builtin_skill, strip_directives, with_directives_protocol,
    AgentDirective,
};
use crate::domain::files_map::FilesMapIndex;
use crate::infrastructure::llm::history_manager::{
    load_or_build_files_map, prepare_structured_prompt, BrainTurn,
};
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

/// Callback invoked when a stage or `ask` turn requests `[EXEC_CMD: <command>]`.
/// Returns `true` if the user approves immediate execution, `false` to decline.
pub type CmdApprovalFn = std::sync::Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// Parses all [NEED_FILE: path] tags from an agent response via `parse_directives`.
pub fn parse_need_file_tags(text: &str) -> HashSet<String> {
    parse_directives(text)
        .into_iter()
        .filter_map(|d| match d {
            AgentDirective::NeedFile(p) => Some(p),
            _ => None,
        })
        .collect()
}

/// Parses all [EXEC_CMD: command] tags from an agent response via `parse_directives`.
pub fn parse_exec_cmd_tags(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    for d in parse_directives(text) {
        if let AgentDirective::ExecCmd(cmd) = d
            && !commands.contains(&cmd)
        {
            commands.push(cmd);
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

/// Tracks resolved files, skills, web lookups, and commands across reasoning stages or `ask` rounds.
#[derive(Default)]
pub(crate) struct DirectiveState {
    pub loaded_files: HashSet<String>,
    pub loaded_skills: HashSet<String>,
    pub skill_guidelines: String,
    pub executed_cmds: HashSet<String>,
    pub fetched_urls: HashSet<String>,
    pub searched_queries: HashSet<String>,
    pub created_pods: bool,
    pub pending_commands: Vec<String>,
}

pub(crate) fn append_skills(base: &str, skills: &str) -> String {
    if skills.is_empty() {
        base.to_string()
    } else {
        format!("{}{}", base, skills)
    }
}

async fn run_approved_command(root: &Path, cmd: &str) -> String {
    let _heavy_guard = if super::command_guard::is_heavy_command(cmd) {
        Some(super::command_guard::acquire_heavy_command_guard().await)
    } else {
        None
    };

    #[cfg(windows)]
    let mut command = {
        let mut c = tokio::process::Command::new("cmd");
        c.args(["/C", cmd]);
        c
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };

    command.current_dir(root);
    match tokio::time::timeout(std::time::Duration::from_secs(60), command.output()).await {
        Ok(Ok(out)) => {
            let code = out.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = format!("{}{}", stdout, stderr);
            let truncated: String = combined.chars().take(4000).collect();
            format!("Exit code: {}\n```text\n{}\n```", code, truncated.trim_end())
        }
        Ok(Err(e)) => format!("Execution error: {}", e),
        Err(_) => "Execution timed out after 60s.".to_string(),
    }
}

/// Resolves all `DIRECTIVES_PROTOCOL` triggers emitted in a stage or `ask` turn and injects
/// results into `history` / `state` before the next stage runs. Returns `true` if new context was added.
pub(crate) async fn absorb_stage_directives(
    stage_output: &str,
    root: &Path,
    files_index: &FilesMapIndex,
    state: &mut DirectiveState,
    safety: &SafetyGuard,
    cmd_approval: Option<&CmdApprovalFn>,
    history: &mut Vec<BrainTurn>,
) -> bool {
    let mut added_blocks: Vec<String> = Vec::new();
    let mut added_skills = false;
    let web_loader = crate::infrastructure::knowledge::web_loader::WebScraper::new();

    for directive in parse_directives(stage_output) {
        match directive {
            AgentDirective::UseSkill(s) => {
                if state.loaded_skills.insert(s.clone())
                    && let Some(instructions) = resolve_builtin_skill(&s)
                {
                    state.skill_guidelines.push_str(&format!(
                        "\n\n---\n## OPERATIONAL GUIDELINES: {}\n{}\n---\n",
                        s.to_uppercase(),
                        instructions
                    ));
                    added_skills = true;
                }
            }
            AgentDirective::NeedDir { id, page } => {
                if let Some(listing) = files_index.render_group(id, page, 50) {
                    added_blocks.push(format!("### Expanded Directory Group {}\n{}", id, listing));
                }
            }
            AgentDirective::NeedFile(path) => {
                if state.loaded_files.insert(path.clone()) {
                    let block = read_workspace_file(root, &path)
                        .unwrap_or_else(|| format!("### File: {}\n(Not found in workspace)\n", path));
                    added_blocks.push(block);
                }
            }
            AgentDirective::WebSearch(query) => {
                if state.searched_queries.insert(query.clone()) {
                    let res = tokio::time::timeout(
                        std::time::Duration::from_secs(15),
                        web_loader.search_web(&query, root),
                    )
                    .await;
                    let summary = match res {
                        Ok(Ok(items)) if !items.is_empty() => items
                            .into_iter()
                            .take(5)
                            .map(|r| format!("- [{}]({}): {}", r.title, r.url, r.snippet))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        Ok(Ok(_)) => "No results found.".to_string(),
                        Ok(Err(e)) => format!("Web search error: {}", e),
                        Err(_) => "Web search timed out.".to_string(),
                    };
                    added_blocks.push(format!("### Web Search: {}\n{}", query, summary));
                }
            }
            AgentDirective::WebUrl(url) => {
                if state.fetched_urls.insert(url.clone()) {
                    let res = tokio::time::timeout(
                        std::time::Duration::from_secs(15),
                        web_loader.fetch_url(&url),
                    )
                    .await;
                    let body = match res {
                        Ok(Ok(doc)) => doc.content.chars().take(6000).collect::<String>(),
                        Ok(Err(e)) => format!("Failed to fetch URL: {}", e),
                        Err(_) => "URL fetch timed out.".to_string(),
                    };
                    added_blocks.push(format!("### Web Page: {}\n{}", url, body));
                }
            }
            AgentDirective::CreatePod { .. } => {
                if !state.created_pods {
                    let _ = crate::domain::pod::AgentPod::process_autonomous_pod_creations(
                        root,
                        stage_output,
                    );
                    state.created_pods = true;
                }
            }
            AgentDirective::ExecCmd(cmd) => {
                if !state.executed_cmds.insert(cmd.clone()) {
                    continue;
                }
                if let Err(err) = safety.validate_intent(&cmd) {
                    added_blocks.push(format!("### Command Blocked: `{}`\n{}", cmd, err));
                    continue;
                }
                let Some(cb) = cmd_approval.cloned() else {
                    if !state.pending_commands.contains(&cmd) {
                        state.pending_commands.push(cmd);
                    }
                    continue;
                };
                let cmd_for_cb = cmd.clone();
                let approved = tokio::task::spawn_blocking(move || cb(&cmd_for_cb))
                    .await
                    .unwrap_or(false);
                if approved {
                    let out = run_approved_command(root, &cmd).await;
                    added_blocks.push(format!("### Command Output: `{}`\n{}", cmd, out));
                } else {
                    added_blocks.push(format!(
                        "### Command Declined: `{}`\nUser declined executing this command.",
                        cmd
                    ));
                }
            }
        }
    }

    if !added_blocks.is_empty() {
        history.push(BrainTurn {
            role: "user".to_string(),
            content: added_blocks.join("\n\n"),
        });
        return true;
    }

    added_skills
}

/// Executes a stage of the `think` loop using `prepare_structured_prompt` so `laws` and
/// `files_map` are delivered via `system` while keeping `think_history` clean of delimiter tags.
async fn run_think_turn(
    think_history: &mut Vec<BrainTurn>,
    raw_prompt: &str,
    stage_law: &str,
    context_map: Option<&str>,
    brain: &BrainConfig,
    threads: u32,
    max_context: usize,
) -> Result<String> {
    let effective_law = with_directives_protocol(stage_law);
    let prepared = prepare_structured_prompt(think_history, raw_prompt, &effective_law, context_map);
    let system_text = prepared.system.unwrap_or(effective_law);

    let chat_template = detect_template(&brain.model);
    let prompt_to_use = if brain.provider == "local" {
        let local_turns: Vec<crate::infrastructure::llm::inference::chat_template::ChatTurn> =
            prepared
                .history
                .iter()
                .map(|(role, content)| {
                    crate::infrastructure::llm::inference::chat_template::ChatTurn {
                        role: role.clone(),
                        content: content.clone(),
                    }
                })
                .collect();
        apply_template(&chat_template, &system_text, &local_turns, &prepared.prompt)
    } else {
        prepared.prompt
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
        system: Some(system_text),
        history: if prepared.history.is_empty() {
            None
        } else {
            Some(prepared.history)
        },
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
    let prompt_clone = prompt_to_use;
    let inference_handle = tokio::spawn(async move {
        infer_with_params_streaming(&prompt_clone, &params, move |token| {
            let _ = tx.try_send(token);
        })
        .await
    });

    let mut turn_response = String::new();
    while let Some(token) = rx.recv().await {
        turn_response.push_str(&token);
    }

    let content = match inference_handle.await {
        Ok(Ok(r)) => r.content,
        Ok(Err(e)) => return Err(e.into()),
        Err(e) => return Err(anyhow::anyhow!("Inference panicked: {}", e)),
    };

    think_history.push(BrainTurn {
        role: "user".to_string(),
        content: raw_prompt.to_string(),
    });
    think_history.push(BrainTurn {
        role: "assistant".to_string(),
        content: content.clone(),
    });

    Ok(content)
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
    progress_callback: Option<F>,
    system_override: Option<&str>,
    history: Option<&[crate::infrastructure::llm::history_manager::BrainTurn]>,
) -> Result<ThoughtResult>
where
    F: FnMut(ThoughtPhase) + Send + Sync,
{
    execute_thought_loop_with_options(
        task,
        files,
        config,
        root,
        provider_opt,
        model_opt,
        progress_callback,
        system_override,
        history,
        None,
    )
    .await
}

/// Executes a full 4-turn reflective reasoning loop with optional custom system laws,
/// conversation history, and an interactive `[EXEC_CMD]` approval callback for intermediate stages.
#[allow(clippy::too_many_arguments)]
pub async fn execute_thought_loop_with_options<F>(
    task: &str,
    files: &[String],
    config: &UnevenConfig,
    root: &Path,
    provider_opt: Option<&str>,
    model_opt: Option<&str>,
    mut progress_callback: Option<F>,
    system_override: Option<&str>,
    history: Option<&[crate::infrastructure::llm::history_manager::BrainTurn]>,
    cmd_approval: Option<CmdApprovalFn>,
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

    let files_index = load_or_build_files_map(root);
    let compact_files_map = files_index.render_compact_index();
    let full_map_context = if compact_files_map.is_empty() {
        context_block.clone()
    } else {
        format!("### Directory Groups (use [NEED_DIR: id] to expand)\n{}\n\n{}", compact_files_map, context_block)
    };

    // Seed internal think history with the last 6 conversation turns (if provided)
    let mut think_history: Vec<BrainTurn> = if let Some(hist) = history {
        hist.iter()
            .rev()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    } else {
        Vec::new()
    };

    let mut directive_state = DirectiveState {
        loaded_files,
        loaded_skills,
        skill_guidelines,
        ..Default::default()
    };

    // ─────────────────────────────────────────────────────────────────────────────
    // TURN 1: Draft Proposal (Actor)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::Drafting);
    }

    let draft_base = append_skills(
        &system_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| laws::get_context_in_root("think_draft", Some(root))),
        &directive_state.skill_guidelines,
    );

    let draft_system = format!(
        "{}\n\n[PHASE: DRAFTING - Produce an initial concrete solution, architecture plan, and preliminary diffs.]",
        draft_base
    );
    let draft_prompt = format!(
        "User Request: {}\n\nFormulate the initial solution plan and preliminary changes based strictly on the provided codebase context.",
        task
    );

    let draft_response = run_think_turn(
        &mut think_history,
        &draft_prompt,
        &draft_system,
        Some(&full_map_context),
        &brain,
        threads,
        max_context,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Draft phase failed: {}", e))?;

    absorb_stage_directives(
        &draft_response,
        root,
        &files_index,
        &mut directive_state,
        &safety,
        cmd_approval.as_ref(),
        &mut think_history,
    )
    .await;

    // ─────────────────────────────────────────────────────────────────────────────
    // INTERMEDIATE 1: Rubber Duck Problem Decomposition (Unpacking Draft)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::RubberDuckingDraft);
    }

    let duck_draft_system = append_skills(
        &laws::get_context_in_root("think_duck_draft", Some(root)),
        &directive_state.skill_guidelines,
    );
    let duck_draft_prompt =
        "Explain your draft above to your Rubber Duck following your system guidelines.";

    let duck_draft_reflection = run_think_turn(
        &mut think_history,
        duck_draft_prompt,
        &duck_draft_system,
        Some(&full_map_context),
        &brain,
        threads,
        max_context,
    )
    .await
    .unwrap_or_else(|_| "Rubber Duck Draft reflection unavailable.".to_string());

    absorb_stage_directives(
        &duck_draft_reflection,
        root,
        &files_index,
        &mut directive_state,
        &safety,
        cmd_approval.as_ref(),
        &mut think_history,
    )
    .await;

    // ─────────────────────────────────────────────────────────────────────────────
    // TURN 2: Internal Critic & Contextual Audit (The Critic Loop)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::Auditing);
    }

    let critic_system = append_skills(
        &laws::get_context_in_root("think_audit", Some(root)),
        &directive_state.skill_guidelines,
    );
    let critic_prompt = "Audit the draft above against the user request and codebase context, listing any defects or required fixes.";

    let critic_findings = run_think_turn(
        &mut think_history,
        critic_prompt,
        &critic_system,
        Some(&full_map_context),
        &brain,
        threads,
        max_context,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Critic audit phase failed: {}", e))?;

    absorb_stage_directives(
        &critic_findings,
        root,
        &files_index,
        &mut directive_state,
        &safety,
        cmd_approval.as_ref(),
        &mut think_history,
    )
    .await;

    // ─────────────────────────────────────────────────────────────────────────────
    // TURN 3: Scope & Code Integrity Guard (Anti-Regression & Anti-Hallucination)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::VerifyingIntegrity);
    }

    let integrity_system = append_skills(
        &laws::get_context_in_root("think_integrity", Some(root)),
        &directive_state.skill_guidelines,
    );
    let integrity_prompt = "Verify scope and code integrity against the draft and critic findings above, and output your verdict.";

    let integrity_findings = run_think_turn(
        &mut think_history,
        integrity_prompt,
        &integrity_system,
        Some(&full_map_context),
        &brain,
        threads,
        max_context,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Integrity verification phase failed: {}", e))?;

    absorb_stage_directives(
        &integrity_findings,
        root,
        &files_index,
        &mut directive_state,
        &safety,
        cmd_approval.as_ref(),
        &mut think_history,
    )
    .await;

    // ─────────────────────────────────────────────────────────────────────────────
    // INTERMEDIATE 2: Rubber Duck Pre-Flight Execution Simulation (Dry-Run Walkthrough)
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::RubberDuckingSimulation);
    }

    let duck_sim_system = append_skills(
        &laws::get_context_in_root("think_duck_simulation", Some(root)),
        &directive_state.skill_guidelines,
    );
    let duck_sim_prompt =
        "Run your pre-flight dry-run simulation over the reconciled solution following your system guidelines.";

    let duck_simulation_trace = run_think_turn(
        &mut think_history,
        duck_sim_prompt,
        &duck_sim_system,
        Some(&full_map_context),
        &brain,
        threads,
        max_context,
    )
    .await
    .unwrap_or_else(|_| "Rubber Duck Simulation trace unavailable.".to_string());

    absorb_stage_directives(
        &duck_simulation_trace,
        root,
        &files_index,
        &mut directive_state,
        &safety,
        cmd_approval.as_ref(),
        &mut think_history,
    )
    .await;

    // ─────────────────────────────────────────────────────────────────────────────
    // TURN 4: Synthesis & Final Assured Patch
    // ─────────────────────────────────────────────────────────────────────────────
    if let Some(ref mut cb) = progress_callback {
        cb(ThoughtPhase::Synthesizing);
    }

    let synthesis_base = append_skills(
        &system_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| laws::get_context_in_root("think_synthesis", Some(root))),
        &directive_state.skill_guidelines,
    );

    let synthesis_system = format!(
        "{}\n\n[PHASE: FINAL SYNTHESIS - Eliminate all issues flagged by the Critic and enforce Scope & Code Integrity. Deliver the final proposal and structured Markdown patches.]",
        synthesis_base
    );

    let synthesis_prompt =
        "Deliver the final verified response and patches for the user, resolving all Critic and Integrity findings.";

    let final_answer = run_think_turn(
        &mut think_history,
        synthesis_prompt,
        &synthesis_system,
        Some(&full_map_context),
        &brain,
        threads,
        max_context,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Synthesis phase failed: {}", e))?;

    // 6. Sanitize response with Security Guard
    let sanitized_answer = safety.sanitize_response(&final_answer);

    // 7. Autonomously provision any specialized pods created by the AI
    let _ = crate::domain::pod::AgentPod::process_autonomous_pod_creations(root, &sanitized_answer);

    // 8. Parse patch objects and CLI command tags if present
    let session_id = uuid::Uuid::new_v4().to_string();
    let parsed_file_obj = file_object(&sanitized_answer, &session_id);
    let mut parsed_commands = directive_state.pending_commands;
    for cmd in parse_exec_cmd_tags(&sanitized_answer) {
        if !parsed_commands.contains(&cmd) {
            parsed_commands.push(cmd);
        }
    }

    // 9. Clean chat proposal for human readability (strips internal directives and replaces machine SEARCH/REPLACE blocks with clean diffs)
    let clean_output = strip_directives(&sanitized_answer);
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

    #[tokio::test]
    async fn test_absorb_stage_directives_exec_cmd_approved_and_declined() {
        let dir = tempfile::tempdir().unwrap();
        let files_index = FilesMapIndex::build(dir.path(), None);
        let safety = SafetyGuard::new();
        let mut state = DirectiveState::default();
        let mut history = Vec::new();

        let approve_cb: CmdApprovalFn = std::sync::Arc::new(|_| true);
        let added = absorb_stage_directives(
            "Check output: [EXEC_CMD: echo hello_directive]",
            dir.path(),
            &files_index,
            &mut state,
            &safety,
            Some(&approve_cb),
            &mut history,
        )
        .await;
        assert!(added);
        assert_eq!(history.len(), 1);
        assert!(history[0].content.contains("Command Output"));
        assert!(history[0].content.contains("hello_directive"));

        let decline_cb: CmdApprovalFn = std::sync::Arc::new(|_| false);
        let added_decline = absorb_stage_directives(
            "Declined: [EXEC_CMD: echo declined_cmd]",
            dir.path(),
            &files_index,
            &mut state,
            &safety,
            Some(&decline_cb),
            &mut history,
        )
        .await;
        assert!(added_decline);
        assert_eq!(history.len(), 2);
        assert!(history[1].content.contains("Command Declined"));
    }

    #[tokio::test]
    async fn test_absorb_stage_directives_need_file_and_dedup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sample.txt"), "sample content").unwrap();
        let files_index = FilesMapIndex::build(dir.path(), None);
        let safety = SafetyGuard::new();
        let mut state = DirectiveState::default();
        let mut history = Vec::new();

        let added = absorb_stage_directives(
            "Read [NEED_FILE: sample.txt]",
            dir.path(),
            &files_index,
            &mut state,
            &safety,
            None,
            &mut history,
        )
        .await;
        assert!(added);
        assert!(history[0].content.contains("sample content"));

        let added_again = absorb_stage_directives(
            "Read again [NEED_FILE: sample.txt]",
            dir.path(),
            &files_index,
            &mut state,
            &safety,
            None,
            &mut history,
        )
        .await;
        assert!(!added_again);
    }
}
