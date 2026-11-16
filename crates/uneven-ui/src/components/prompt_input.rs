use crate::state::{AppState, MessageRole, TelemetryData};
use crate::utils::project::pick_and_attach_files;
use dioxus::prelude::*;
use std::time::Instant;
use uneven_core::application::ask_service::execute_ask_with_system;
use uneven_core::application::multi_agent_service::{
    execute_multi_agent_workflow, SupervisorEvent,
};
use uneven_core::application::shell_classifier::{
    classify_shell_intent, is_approval_intent, ShellAction,
};
use uneven_core::application::think_service::{
    execute_thought_loop_with_history, ThoughtPhase,
};
use uneven_core::infrastructure::llm::history_manager::BrainTurn;
use uneven_core::infrastructure::llm::{ensure_local_model_files, is_local_model_downloaded};
use uneven_core::infrastructure::patch::{apply_file_actions, PatchActionKind};
use uneven_core::infrastructure::security::execute_safe_command;

fn submit_prompt(
    mut state: Signal<AppState>,
    mut prompt_text: Signal<String>,
) {
    let task = prompt_text.read().trim().to_string();
    if task.is_empty()
        || state.read().is_reasoning
        || task.len() > uneven_core::infrastructure::translation::MAX_ALLOWED_PROMPT_CHARS
    {
        return;
    }

    let attached = state.read().attached_files.clone();

    let (task_to_run, files_to_run, is_multi_agent) = {
        let s = state.read();
        if let Some(pod) = s.active_pod() {
            let files = if attached.is_empty() {
                pod.target_files.clone()
            } else {
                attached.clone()
            };
            (task.clone(), files, pod.multi_agent_enabled || s.multi_agent_mode)
        } else {
            (task.clone(), attached.clone(), s.multi_agent_mode)
        }
    };

    // If an active pod is set, set its status to Running
    let active_pod_id_initial = state.read().active_pod_id.clone();
    if let Some(ref pid) = active_pod_id_initial {
        let mut s = state.write();
        if let Some(p) = s.pods.iter_mut().find(|p| &p.id == pid) {
            p.status = uneven_core::domain::pod::PodStatus::Running;
        }
    }

    // 1. Append user message to active context history
    state.write().push_message(MessageRole::User, task.clone(), attached.clone());
    state.write().is_reasoning = true;
    state.write().thought_phase = Some(ThoughtPhase::ResolvingContext);

    // Clear input state
    prompt_text.set(String::new());
    state.write().attached_files.clear();

    // 1.5 Direct confirmation of pending patch:
    // If a patch is already pending and user confirms/approves, immediately apply changes!
    if state.read().pending_patch.is_some() && is_approval_intent(&task) {
        let file_obj = state.read().pending_patch.clone().unwrap();
        let pending_commands = state.read().thought_result.as_ref().map(|t| t.commands.clone()).unwrap_or_default();
        let maybe_root = state.read().project_root.clone();
        let root = match maybe_root {
            Some(r) => r,
            None => {
                state.write().push_message(
                    MessageRole::Assistant,
                    "Please open a project directory before applying patches.",
                    Vec::new(),
                );
                state.write().is_reasoning = false;
                state.write().thought_phase = None;
                return;
            }
        };

        spawn(async move {
            let results = apply_file_actions(&root, &file_obj);
            let mut success_count = 0;
            let mut failed_count = 0;
            let mut report_lines = Vec::new();

            for res in results {
                match res.kind {
                    PatchActionKind::Created(p) => {
                        success_count += 1;
                        report_lines.push(format!("- Created: `{}`", p));
                    }
                    PatchActionKind::Edited(p) => {
                        success_count += 1;
                        report_lines.push(format!("- Edited: `{}`", p));
                    }
                    PatchActionKind::Renamed { from, to } => {
                        success_count += 1;
                        report_lines.push(format!("- Renamed: `{}` → `{}`", from, to));
                    }
                    PatchActionKind::Deleted(p) => {
                        success_count += 1;
                        report_lines.push(format!("- Deleted: `{}`", p));
                    }
                    PatchActionKind::Appended { path, .. } => {
                        success_count += 1;
                        report_lines.push(format!("- Appended: `{}`", path));
                    }
                    PatchActionKind::Failed { path, reason } => {
                        failed_count += 1;
                        report_lines.push(format!("- ❌ Failed `{}`: {}", path, reason));
                    }
                    PatchActionKind::SkippedUnsafe(p) => {
                        failed_count += 1;
                        report_lines.push(format!("- ⚠️ Skipped unsafe path: `{}`", p));
                    }
                }
            }

            if !pending_commands.is_empty() {
                report_lines.push("\n⚡ **Executed Commands:**".to_string());
                for cmd in &pending_commands {
                    match execute_safe_command(cmd, &root, None).await {
                        Ok(cmd_res) => {
                            let icon = if cmd_res.success { "✅" } else { "❌" };
                            report_lines.push(format!("- {} `{}`", icon, cmd));
                            if !cmd_res.output.trim().is_empty() {
                                report_lines.push(format!("  ```\n  {}\n  ```", cmd_res.output.trim()));
                            }
                        }
                        Err(e) => {
                            report_lines.push(format!("- ⚠️ Blocked `{}`: {}", cmd, e));
                        }
                    }
                }
            }

            let message_text = if failed_count == 0 {
                format!(
                    "✅ **Applied {} mutation(s) to disk successfully!**\n\n{}",
                    success_count,
                    report_lines.join("\n")
                )
            } else {
                format!(
                    "⚠️ **Applied with warnings** ({} successful, {} failed):\n\n{}",
                    success_count,
                    failed_count,
                    report_lines.join("\n")
                )
            };

            state.write().pending_patch = None;
            state.write().show_patch_viewer = false;
            state.write().is_reasoning = false;
            state.write().thought_phase = None;
            state.write().push_message(MessageRole::Assistant, message_text, Vec::new());

            let current_pod_id = state.read().active_pod_id.clone();
            if let Some(ref pid) = current_pod_id {
                let mut s = state.write();
                if let Some(p) = s.pods.iter_mut().find(|p| &p.id == pid) {
                    p.status = uneven_core::domain::pod::PodStatus::Completed;
                    p.completed_tasks = (p.completed_tasks + 1).min(p.total_tasks.max(1));
                }
            }
        });
        return;
    }

    // 2. Spawn async task to run either multi-agent supervisor or single think loop
    spawn(async move {
        let start_time = Instant::now();
        let maybe_root = state.read().project_root.clone();
        let root = match maybe_root {
            Some(r) => r,
            None => {
                state.write().push_message(
                    MessageRole::Assistant,
                    "Please open a project folder first before executing instructions.",
                    Vec::new(),
                );
                state.write().is_reasoning = false;
                state.write().thought_phase = None;
                return;
            }
        };

        let config_res = uneven_core::infrastructure::config::loader::load_config(&root);
        let mut config = match config_res {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Failed to load configuration: {}", e);
                state.write().push_message(MessageRole::Assistant, err_msg, Vec::new());
                state.write().is_reasoning = false;
                state.write().thought_phase = None;
                return;
            }
        };

        // Align brain configuration with currently selected model in UI state
        config.brain.provider = state.read().active_provider.clone();
        config.brain.model = state.read().active_model.clone();
        config.brain.max_tokens = uneven_core::infrastructure::llm::get_default_max_output_tokens(
            &config.brain.provider,
            &config.brain.model,
        );

        // If a local GGUF model is selected, verify/download files if missing
        if config.brain.provider == "local"
            && let Err(e) = ensure_local_model_files(&config.brain.model).await
        {
            state.write().push_message(
                MessageRole::Assistant,
                format!("⚠️ Local GGUF model download error: {}", e),
                Vec::new(),
            );
            state.write().is_reasoning = false;
            state.write().thought_phase = None;
            return;
        }

        // If Auto-Translate is enabled, translate user prompt to English using DuckDuckGo for token economy
        let effective_task_to_run = if state.read().auto_translate_to_en {
            uneven_core::infrastructure::translation::translate_prompt_to_english(&task_to_run).await
        } else {
            task_to_run.clone()
        };

        let active_pod_opt = state.read().active_pod().cloned();
        let sess_id = if let Some(ref p) = active_pod_opt {
            format!("{}_{}", state.read().active_session, p.id)
        } else {
            state.read().active_session.clone()
        };
        let pod_ask_override = active_pod_opt.as_ref().map(|p| uneven_core::infrastructure::security::laws::get_pod_context("ask", p));
        let pod_think_override = active_pod_opt.as_ref().map(|p| uneven_core::infrastructure::security::laws::get_pod_context("think", p));

        let mut history_turns = uneven_core::infrastructure::llm::history_manager::get_history_from_disk(&sess_id, &root).unwrap_or_default();
        uneven_core::application::ContextCompactorService::auto_compact(&mut history_turns);
        let files_map = uneven_core::infrastructure::llm::history_manager::get_project_files_map(&config, &root);

        // ─── Shell Intent Classifier (Using laws::get_context("shell") & files_map) ───
        state.write().thought_phase = Some(ThoughtPhase::ResolvingContext);
        let (intent, classifier_msg, _classifier_tokens) =
            classify_shell_intent(&effective_task_to_run, &history_turns, &files_map, &config, None).await;

        let mut run_think_pipeline = false;
        let mut is_executing_confirmed_plan = false;
        let mut effective_multi_agent = is_multi_agent;

        match intent.action {
            ShellAction::Askf => {
                let maybe_pending = state.read().pending_patch.clone();
                if let Some(file_obj) = maybe_pending {
                    let results = apply_file_actions(&root, &file_obj);
                    let pending_commands = state.read().thought_result.as_ref().map(|t| t.commands.clone()).unwrap_or_default();
                    let mut summary_lines = Vec::new();
                    summary_lines.push("✅ **Patches applied successfully:**\n".to_string());
                    for r in &results {
                        match &r.kind {
                            PatchActionKind::Created(p) => summary_lines.push(format!("- Created `{}`", p)),
                            PatchActionKind::Edited(p) => summary_lines.push(format!("- Edited `{}`", p)),
                            PatchActionKind::Renamed { from, to } => summary_lines.push(format!("- Renamed `{}` → `{}`", from, to)),
                            PatchActionKind::Deleted(p) => summary_lines.push(format!("- Deleted `{}`", p)),
                            PatchActionKind::Appended { path, anchor } => summary_lines.push(format!("- Appended to `{}` (anchor: {})", path, anchor)),
                            PatchActionKind::Failed { path, reason } => summary_lines.push(format!("- ❌ Failed `{}`: {}", path, reason)),
                            PatchActionKind::SkippedUnsafe(p) => summary_lines.push(format!("- ⚠️ Skipped unsafe `{}`", p)),
                        }
                    }
                    if !pending_commands.is_empty() {
                        summary_lines.push("\n⚡ **Commands executed:**\n".to_string());
                        for cmd in &pending_commands {
                            match execute_safe_command(cmd, &root, None).await {
                                Ok(cmd_res) => {
                                    let icon = if cmd_res.success { "✅" } else { "❌" };
                                    summary_lines.push(format!("- {} `{}`", icon, cmd));
                                    if !cmd_res.output.trim().is_empty() {
                                        summary_lines.push(format!("  ```\n  {}\n  ```", cmd_res.output.trim()));
                                    }
                                }
                                Err(e) => {
                                    summary_lines.push(format!("- ⚠️ Blocked `{}`: {}", cmd, e));
                                }
                            }
                        }
                    }
                    let final_msg = summary_lines.join("\n");
                    state.write().pending_patch = None;
                    state.write().thought_phase = None;
                    state.write().push_message(MessageRole::Assistant, final_msg.clone(), Vec::new());
                    history_turns.push(BrainTurn {
                        role: "user".to_string(),
                        content: task_to_run.clone(),
                    });
                    history_turns.push(BrainTurn {
                        role: "assistant".to_string(),
                        content: final_msg,
                    });
                    let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);

                    let current_pod_id = state.read().active_pod_id.clone();
                    if let Some(ref pid) = current_pod_id {
                        let mut s = state.write();
                        if let Some(p) = s.pods.iter_mut().find(|p| &p.id == pid) {
                            p.status = uneven_core::domain::pod::PodStatus::Completed;
                            p.completed_tasks = (p.completed_tasks + 1).min(p.total_tasks.max(1));
                        }
                    }
                } else {
                    is_executing_confirmed_plan = true;
                    run_think_pipeline = true;
                }
            }
            ShellAction::Think | ShellAction::Ask | ShellAction::Explain | ShellAction::Docs => {
                run_think_pipeline = true;
            }
            ShellAction::MultiAgents => {
                run_think_pipeline = true;
                effective_multi_agent = true;
            }
            ShellAction::RunCmd => {
                let cmd_str = intent.question.clone().unwrap_or_else(|| effective_task_to_run.clone());
                state.write().thought_phase = None;
                let reply = match execute_safe_command(&cmd_str, &root, None).await {
                    Ok(res) => {
                        let status_badge = if res.success { "✅ **Command executed successfully:**" } else { "❌ **Command failed:**" };
                        let output_block = if res.output.trim().is_empty() {
                            "*(No output)*".to_string()
                        } else {
                            format!("```\n{}\n```", res.output.trim())
                        };
                        format!("{}\n```bash\n{}\n```\n\n{}", status_badge, cmd_str, output_block)
                    }
                    Err(e) => {
                        format!("⚠️ **Command execution blocked:** `{}`\n\n> {}", cmd_str, e)
                    }
                };
                state.write().push_message(MessageRole::Assistant, reply.clone(), Vec::new());
                history_turns.push(BrainTurn {
                    role: "user".to_string(),
                    content: task_to_run.clone(),
                });
                history_turns.push(BrainTurn {
                    role: "assistant".to_string(),
                    content: reply,
                });
                let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
            }
            ShellAction::Scan => {
                state.write().thought_phase = Some(ThoughtPhase::ResolvingContext);
                let target_dir = intent.clean_target_dir().unwrap_or_else(|| ".".to_string());
                let config_clone = config.clone();
                let root_clone = root.clone();

                let (phase_tx, mut phase_rx) = tokio::sync::mpsc::channel::<String>(16);
                let mut state_for_phase = state;
                spawn(async move {
                    while let Some(_phase) = phase_rx.recv().await {
                        state_for_phase.write().thought_phase = Some(ThoughtPhase::Auditing);
                    }
                });

                let on_progress = move |msg: &str| {
                    let _ = phase_tx.try_send(msg.to_string());
                };

                let scan_res = uneven_core::application::ScanService::run_scan(
                    &root_clone,
                    &target_dir,
                    &config_clone,
                    Some(on_progress),
                )
                .await;

                state.write().thought_phase = None;

                match scan_res {
                    Ok(res) => {
                        let chat_summary = res.to_chat_summary();
                        state.write().push_message(MessageRole::Assistant, chat_summary.clone(), Vec::new());
                        history_turns.push(BrainTurn {
                            role: "user".to_string(),
                            content: task_to_run.clone(),
                        });
                        history_turns.push(BrainTurn {
                            role: "assistant".to_string(),
                            content: chat_summary,
                        });
                        let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
                    }
                    Err(e) => {
                        let err_msg = format!("❌ **Security scan execution failed:** {}", e);
                        state.write().push_message(MessageRole::Assistant, err_msg, Vec::new());
                    }
                }
            }
            ShellAction::Pentest => {
                state.write().thought_phase = Some(ThoughtPhase::ResolvingContext);
                let pentest_mode = intent.pentest_mode();

                let config_clone = config.clone();
                let root_clone = root.clone();

                let (phase_tx, mut phase_rx) = tokio::sync::mpsc::channel::<String>(16);
                let mut state_for_phase = state;
                spawn(async move {
                    while let Some(_phase) = phase_rx.recv().await {
                        state_for_phase.write().thought_phase = Some(ThoughtPhase::Auditing);
                    }
                });

                let on_progress = move |msg: &str| {
                    let _ = phase_tx.try_send(msg.to_string());
                };

                let pentest_res = uneven_core::application::PentestService::run_pentest(
                    &root_clone,
                    pentest_mode,
                    &config_clone,
                    true,
                    true,
                    Some(on_progress),
                )
                .await;

                state.write().thought_phase = None;

                match pentest_res {
                    Ok(res) => {
                        let chat_summary = res.to_chat_summary();
                        state.write().push_message(MessageRole::Assistant, chat_summary.clone(), Vec::new());
                        history_turns.push(BrainTurn {
                            role: "user".to_string(),
                            content: task_to_run.clone(),
                        });
                        history_turns.push(BrainTurn {
                            role: "assistant".to_string(),
                            content: chat_summary,
                        });
                        let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
                    }
                    Err(e) => {
                        let err_msg = format!("❌ **Pentest execution failed:** {}", e);
                        state.write().push_message(MessageRole::Assistant, err_msg, Vec::new());
                    }
                }
            }
            ShellAction::Zeroday => {
                state.write().thought_phase = Some(ThoughtPhase::ResolvingContext);
                let is_deep = intent.is_deep();
                let dir_override = intent.clean_target_dir().map(|d| vec![d]);

                let config_clone = config.clone();
                let root_clone = root.clone();

                let (phase_tx, mut phase_rx) = tokio::sync::mpsc::channel::<String>(16);
                let mut state_for_phase = state;
                spawn(async move {
                    while let Some(_phase) = phase_rx.recv().await {
                        state_for_phase.write().thought_phase = Some(ThoughtPhase::Auditing);
                    }
                });

                let on_progress = move |msg: &str| {
                    let _ = phase_tx.try_send(msg.to_string());
                };

                let zd_res = uneven_core::application::ZerodayService::run_zeroday_scan(
                    &root_clone,
                    dir_override,
                    &config_clone,
                    true,
                    is_deep,
                    Some(on_progress),
                )
                .await;

                state.write().thought_phase = None;

                match zd_res {
                    Ok(res) => {
                        let chat_summary = res.to_chat_summary();
                        state.write().push_message(MessageRole::Assistant, chat_summary.clone(), Vec::new());
                        history_turns.push(BrainTurn {
                            role: "user".to_string(),
                            content: task_to_run.clone(),
                        });
                        history_turns.push(BrainTurn {
                            role: "assistant".to_string(),
                            content: chat_summary,
                        });
                        let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
                    }
                    Err(e) => {
                        let err_msg = format!("❌ **Zero-day scan execution failed:** {}", e);
                        state.write().push_message(MessageRole::Assistant, err_msg, Vec::new());
                    }
                }
            }
            ShellAction::Analyze => {
                state.write().thought_phase = Some(ThoughtPhase::ResolvingContext);

                let db_url = std::env::var("DATABASE_URL")
                    .or_else(|_| std::env::var("UNEVEN_DB_URL"))
                    .unwrap_or_default();

                if db_url.trim().is_empty() {
                    state.write().thought_phase = None;
                    let msg = "⚠️ **Database URL is not configured.**\n\n\
                    To perform automated database analysis and live BI reporting, please configure your `DATABASE_URL` in **Settings ➔ Database & Security Analysis**.";
                    state.write().push_message(MessageRole::Assistant, msg, Vec::new());
                    return;
                }

                let analysis_goal = intent.task.clone().unwrap_or_else(|| effective_task_to_run.clone());
                let config_clone = config.clone();

                let security = std::sync::Arc::new(uneven_core::infrastructure::security::SecurityContext::new(
                    config_clone.security.blocked_columns.clone(),
                    config_clone.security.blocked_tables.clone(),
                    config_clone.security.masked_columns.clone(),
                ));

                state.write().thought_phase = Some(ThoughtPhase::Auditing);

                let mut analyst = uneven_core::infrastructure::knowledge::DatabaseAnalyst::new(
                    security.clone(),
                    config_clone.knowledge.as_ref()
                        .and_then(|k| k.db.as_ref())
                        .and_then(|d| d.allowed_tables.clone())
                        .unwrap_or_default(),
                );

                if let Err(e) = analyst.connect(&db_url).await {
                    state.write().thought_phase = None;
                    let err_msg = format!("❌ **Database connection error:** {}\n\nPlease verify that your `DATABASE_URL` is reachable.", e);
                    state.write().push_message(MessageRole::Assistant, err_msg, Vec::new());
                    return;
                }

                // Introspect schemas with blind masking applied
                let schemas = match analyst.introspect(None).await {
                    Ok(s) => s,
                    Err(e) => {
                        state.write().thought_phase = None;
                        let err_msg = format!("❌ **Schema introspection failed:** {}", e);
                        state.write().push_message(MessageRole::Assistant, err_msg, Vec::new());
                        return;
                    }
                };

                if schemas.is_empty() {
                    state.write().thought_phase = None;
                    let msg = "⚠️ **No accessible tables found in the database.**\n\nEither no tables exist or all detected tables are restricted by security rules.";
                    state.write().push_message(MessageRole::Assistant, msg, Vec::new());
                    return;
                }

                state.write().thought_phase = Some(ThoughtPhase::Synthesizing);

                let system_instructions = uneven_core::infrastructure::security::laws::get_context("analyze");
                let service = uneven_core::infrastructure::knowledge::AnalysisService::new(
                    analyst,
                    security.clone(),
                    system_instructions,
                );

                let infer_params = uneven_core::infrastructure::llm::InferParams {
                    model: config_clone.brain.model.clone(),
                    threads: 4,
                    max_tokens: 1024,
                    temperature: 0.2,
                    repeat_penalty: 1.1,
                    max_context: 4096,
                    grammar: None,
                    provider: Some(config_clone.brain.provider.clone()),
                    system: None,
                    history: None,
                };

                let db_type = if db_url.starts_with("sqlite") {
                    Some("sqlite")
                } else if db_url.starts_with("postgres") || db_url.starts_with("pg") {
                    Some("postgres")
                } else if db_url.starts_with("mysql") {
                    Some("mysql")
                } else {
                    None
                };

                match service.generate_query(db_type, &schemas, &analysis_goal, &infer_params).await {
                    Ok(query_res) => {
                        state.write().thought_phase = None;

                        if query_res.cannot_generate {
                            let msg = format!(
                                "⚠️ **Could not synthesize query for this request:**\n\n> {}\n\n*Available tables: {}*",
                                query_res.explanation,
                                schemas.iter().map(|s| format!("`{}`", s.name)).collect::<Vec<_>>().join(", ")
                            );
                            state.write().push_message(MessageRole::Assistant, msg, Vec::new());
                        } else {
                            let sql = query_res.sql.clone();
                            let explanation = query_res.explanation.clone();
                            let visual_sug = query_res.visual_suggestions.clone();

                            state.write().pending_sql = Some(crate::state::PendingSqlQuery {
                                id: uuid::Uuid::new_v4().to_string(),
                                original_sql: sql.clone(),
                                current_sql: sql.clone(),
                                explanation: explanation.clone(),
                                goal: analysis_goal.clone(),
                                visual_suggestions: visual_sug,
                            });
                            state.write().show_sql_inspector = true;

                            let notice_msg = format!(
                                "🗄 **SQL Query Synthesized & Awaiting Your Approval**\n\n\
                                **Goal:** {}\n\
                                **Logic:** {}\n\n\
                                ```sql\n{}\n```\n\n\
                                ✏️ *You can edit the SQL query directly in the inspector card above before executing.*",
                                analysis_goal,
                                explanation,
                                sql
                            );
                            state.write().push_message(MessageRole::Assistant, notice_msg.clone(), Vec::new());

                            history_turns.push(BrainTurn {
                                role: "user".to_string(),
                                content: task_to_run.clone(),
                            });
                            history_turns.push(BrainTurn {
                                role: "assistant".to_string(),
                                content: notice_msg,
                            });
                            let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
                        }
                    }
                    Err(e) => {
                        state.write().thought_phase = None;
                        let err_msg = format!("❌ **Failed to generate analytical query:** {}", e);
                        state.write().push_message(MessageRole::Assistant, err_msg, Vec::new());
                    }
                }
            }
            ShellAction::CreatePod => {
                let existing_pods = state.read().pods.clone();
                let mut pod_list_md = String::new();
                for p in &existing_pods {
                    pod_list_md.push_str(&format!("- **{}** (`{}`): {}\n", p.title, p.prime_name, p.instructions));
                }

                let reply = format!(
                    "⚠️ **Agent Pods are autonomously orchestrated and instantiated by AI.**\n\n\
                    Manual creation by users has been disabled. The AI reasoning engine automatically provisions and manages new Pods according to project architectural complexity.\n\n\
                    **Registered Project Pods:**\n{}",
                    if pod_list_md.is_empty() { "No pods registered at the moment.".to_string() } else { pod_list_md }
                );

                state.write().show_pods = true;
                state.write().thought_phase = None;
                state.write().push_message(MessageRole::Assistant, reply.clone(), Vec::new());

                history_turns.push(BrainTurn {
                    role: "user".to_string(),
                    content: task_to_run.clone(),
                });
                history_turns.push(BrainTurn {
                    role: "assistant".to_string(),
                    content: reply,
                });
                let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
            }
            ShellAction::DispatchPod => {
                let (target, mission) = intent.pod_dispatch_spec(&effective_task_to_run);

                // Update pod status to Running in UI state
                let target_lower = target.to_lowercase();
                {
                    let mut s = state.write();
                    if let Some(p) = s.pods.iter_mut().find(|p| {
                        p.id.to_lowercase() == target_lower
                            || p.title.to_lowercase().contains(&target_lower)
                    }) {
                        p.status = uneven_core::domain::pod::PodStatus::Running;
                    }
                }

                let (phase_tx, mut phase_rx) = tokio::sync::mpsc::channel::<ThoughtPhase>(16);
                let mut state_for_phase = state;
                spawn(async move {
                    while let Some(phase) = phase_rx.recv().await {
                        state_for_phase.write().thought_phase = Some(phase);
                    }
                });

                let on_phase = move |phase: ThoughtPhase| {
                    let _ = phase_tx.try_send(phase);
                };

                let active_session = state.read().active_session.clone();
                let outcome_res = uneven_core::application::dispatch_pod_mission(
                    &target,
                    &mission,
                    &config,
                    &root,
                    &active_session,
                    Some(on_phase),
                )
                .await;

                let elapsed = start_time.elapsed();
                match outcome_res {
                    Ok(outcome) => {
                        let final_msg = outcome.summary_message.clone();
                        let file_obj = outcome.file_object.clone();

                        // Reload pods in state so the card displays Completed, updated tasks, and last_handoff
                        state.write().pods = uneven_core::domain::pod::AgentPod::load_project_pods(&root);
                        state.write().pending_patch = file_obj;
                        state.write().thought_phase = None;
                        state.write().telemetry = TelemetryData {
                            tokens_used: 0,
                            prompt_tokens: 0,
                            completion_tokens: 0,
                            cost: 0.0,
                            saved: 0.0,
                            elapsed,
                        };

                        state.write().push_message(MessageRole::Assistant, final_msg.clone(), Vec::new());

                        history_turns.push(BrainTurn {
                            role: "user".to_string(),
                            content: task_to_run.clone(),
                        });
                        history_turns.push(BrainTurn {
                            role: "assistant".to_string(),
                            content: final_msg,
                        });
                        let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
                    }
                    Err(err) => {
                        state.write().thought_phase = None;
                        state.write().push_message(
                            MessageRole::Assistant,
                            format!("⚠️ Pod mission execution failed: {}", err),
                            Vec::new(),
                        );
                    }
                }
            }
            _ => {
                if !classifier_msg.is_empty() {
                    state.write().thought_phase = None;
                    state.write().push_message(MessageRole::Assistant, classifier_msg.clone(), Vec::new());
                    history_turns.push(BrainTurn {
                        role: "user".to_string(),
                        content: task_to_run.clone(),
                    });
                    history_turns.push(BrainTurn {
                        role: "assistant".to_string(),
                        content: classifier_msg,
                    });
                    let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
                } else {
                    let ask_res = execute_ask_with_system(
                        &effective_task_to_run,
                        &files_to_run,
                        &config,
                        &root,
                        None,
                        None,
                        Some(&mut history_turns),
                        pod_ask_override.as_deref(),
                    )
                    .await;

                    if let Ok(result) = ask_res {
                        let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
                        state.write().thought_phase = None;
                        state.write().push_message(MessageRole::Assistant, result.response, Vec::new());
                    }
                }
            }
        }

        if run_think_pipeline {
            let is_multi_agent = effective_multi_agent;
                if is_multi_agent {
                    // ─── Multi-Agent Supervisor Pipeline ───────────────────────
                    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<SupervisorEvent>(32);
                    let mut state_for_events = state;
                    let sess_id_events = sess_id.clone();
                    let root_events = root.clone();
                    let task_events = task_to_run.clone();

                    spawn(async move {
                        while let Some(evt) = event_rx.recv().await {
                            match evt {
                                SupervisorEvent::PlanningStarted => {
                                    state_for_events.write().active_subagent_info =
                                        Some("👑 Prime: Devising Strategy".to_string());
                                    state_for_events.write().thought_phase =
                                        Some(ThoughtPhase::ResolvingContext);
                                }
                                SupervisorEvent::PlanCreated {
                                    total_subtasks,
                                    strategy,
                                    subtasks,
                                } => {
                                    let mut plan_desc = format!(
                                        "👑 **Prime Supervisor Architecture Plan**\n\n**Strategy:** {}\n\n**Delegated Pipeline ({} subtasks)**:\n",
                                        strategy, total_subtasks
                                    );
                                    for sub in subtasks {
                                        plan_desc.push_str(&format!(
                                            "- **Worker #{}**: {} (target: `{}`)\n",
                                            sub.index,
                                            sub.title,
                                            sub.target_files.join(", ")
                                        ));
                                    }
                                    state_for_events.write().push_message(MessageRole::Assistant, plan_desc, Vec::new());
                                }
                                SupervisorEvent::SubagentStarted {
                                    subagent_idx,
                                    total,
                                    title,
                                    ..
                                } => {
                                    state_for_events.write().active_subagent_info =
                                        Some(format!("Worker #{}/{} [{}]", subagent_idx, total, title));
                                    state_for_events.write().thought_phase =
                                        Some(ThoughtPhase::ResolvingContext);
                                }
                                SupervisorEvent::SubagentPhase { phase, .. } => {
                                    state_for_events.write().thought_phase = Some(phase);
                                }
                                SupervisorEvent::EvaluatingSubagent { subagent_idx } => {
                                    state_for_events.write().active_subagent_info =
                                        Some(format!("👑 Prime: Evaluating Worker #{}", subagent_idx));
                                    state_for_events.write().thought_phase = Some(ThoughtPhase::Auditing);
                                }
                                SupervisorEvent::SubagentCompleted {
                                    subagent_idx,
                                    patches_generated,
                                    summary,
                                } => {
                                    let report = format!(
                                        "⚙️ **Worker #{} Completed** ({} patch mutation(s)):\n\n{}",
                                        subagent_idx, patches_generated, summary
                                    );
                                    state_for_events.write().push_message(MessageRole::Assistant, report, Vec::new());
                                }
                                SupervisorEvent::AllCompleted {
                                    final_summary,
                                    file_object,
                                } => {
                                    state_for_events.write().pending_patch = file_object;
                                    state_for_events.write().active_subagent_info = None;
                                    state_for_events.write().thought_phase = None;
                                    state_for_events.write().push_message(
                                        MessageRole::Assistant,
                                        format!("🏁 **Pipeline Complete:** {}", final_summary),
                                        Vec::new(),
                                    );

                                    let current_pod_id = state_for_events.read().active_pod_id.clone();
                                    if let Some(ref pid) = current_pod_id {
                                        let mut s = state_for_events.write();
                                        if let Some(p) = s.pods.iter_mut().find(|p| &p.id == pid) {
                                            p.status = uneven_core::domain::pod::PodStatus::Completed;
                                            p.completed_tasks = (p.completed_tasks + 1).min(p.total_tasks.max(1));
                                        }
                                    }

                                    let mut hist = uneven_core::infrastructure::llm::history_manager::get_history_from_disk(&sess_id_events, &root_events).unwrap_or_default();
                                    hist.push(BrainTurn {
                                        role: "user".to_string(),
                                        content: task_events.clone(),
                                    });
                                    hist.push(BrainTurn {
                                        role: "assistant".to_string(),
                                        content: format!("🏁 **Pipeline Complete:** {}", final_summary),
                                    });
                                    let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&hist, &sess_id_events, &root_events);
                                }
                                SupervisorEvent::Failed {
                                    subagent_idx,
                                    reason,
                                } => {
                                    let err_text = match subagent_idx {
                                        Some(idx) => {
                                            format!("⚠️ Worker #{} encountered an issue: {}", idx, reason)
                                        }
                                        None => format!("⚠️ Supervisor planning error: {}", reason),
                                    };
                                    state_for_events.write().push_message(MessageRole::Assistant, err_text, Vec::new());

                                    let current_pod_id = state_for_events.read().active_pod_id.clone();
                                    if let Some(ref pid) = current_pod_id {
                                        let mut s = state_for_events.write();
                                        if let Some(p) = s.pods.iter_mut().find(|p| &p.id == pid) {
                                            p.status = uneven_core::domain::pod::PodStatus::Failed;
                                        }
                                    }
                                }
                            }
                        }
                    });

                    let on_event = move |evt: SupervisorEvent| {
                        let _ = event_tx.try_send(evt);
                    };

                    let multi_res = execute_multi_agent_workflow(
                        &effective_task_to_run,
                        &files_to_run,
                        &config,
                        &root,
                        None,
                        None,
                        Some(on_event),
                    )
                    .await;

                    let elapsed = start_time.elapsed();
                    if multi_res.is_ok() {
                        state.write().telemetry = TelemetryData {
                            tokens_used: 0,
                            prompt_tokens: 0,
                            completion_tokens: 0,
                            cost: 0.0,
                            saved: 0.0,
                            elapsed,
                        };
                    }
                } else {
                    // ─── Standard Single-Agent Think Pipeline ──────────────────
                    let (phase_tx, mut phase_rx) = tokio::sync::mpsc::channel::<ThoughtPhase>(16);
                    let mut state_for_phase = state;
                    let phase_collector = spawn(async move {
                        while let Some(phase) = phase_rx.recv().await {
                            if phase == ThoughtPhase::Completed {
                                state_for_phase.write().thought_phase = None;
                            } else {
                                state_for_phase.write().thought_phase = Some(phase);
                            }
                        }
                        state_for_phase.write().thought_phase = None;
                    });

                    let on_phase = move |phase: ThoughtPhase| {
                        let _ = phase_tx.try_send(phase);
                    };

                    let think_res = execute_thought_loop_with_history(
                        &effective_task_to_run,
                        &files_to_run,
                        &config,
                        &root,
                        None,
                        None,
                        Some(on_phase),
                        pod_think_override.as_deref(),
                        Some(&history_turns),
                    )
                    .await;

                    phase_collector.cancel();
                    state.write().thought_phase = None;

                    let elapsed = start_time.elapsed();

                    match think_res {
                        Ok(result) => {
                            let final_ans = result.final_response.clone();
                            let pending_patch = result.file_object.clone();
                            let commands = result.commands.clone();

                            if is_executing_confirmed_plan && (pending_patch.is_some() || !commands.is_empty()) {
                                let mut summary_lines = Vec::new();
                                if let Some(ref file_obj) = pending_patch {
                                    let results = apply_file_actions(&root, file_obj);
                                    summary_lines.push("✅ **Patches applied successfully:**\n".to_string());
                                    for r in &results {
                                        match &r.kind {
                                            PatchActionKind::Created(p) => summary_lines.push(format!("- Created `{}`", p)),
                                            PatchActionKind::Edited(p) => summary_lines.push(format!("- Edited `{}`", p)),
                                            PatchActionKind::Renamed { from, to } => summary_lines.push(format!("- Renamed `{}` → `{}`", from, to)),
                                            PatchActionKind::Deleted(p) => summary_lines.push(format!("- Deleted `{}`", p)),
                                            PatchActionKind::Appended { path, anchor } => summary_lines.push(format!("- Appended to `{}` (anchor: {})", path, anchor)),
                                            PatchActionKind::Failed { path, reason } => summary_lines.push(format!("- ❌ Failed `{}`: {}", path, reason)),
                                            PatchActionKind::SkippedUnsafe(p) => summary_lines.push(format!("- ⚠️ Skipped unsafe `{}`", p)),
                                        }
                                    }
                                }

                                if !commands.is_empty() {
                                    if !summary_lines.is_empty() {
                                        summary_lines.push(String::new());
                                    }
                                    summary_lines.push("⚡ **Commands executed:**\n".to_string());
                                    for cmd in &commands {
                                        match execute_safe_command(cmd, &root, None).await {
                                            Ok(cmd_res) => {
                                                let icon = if cmd_res.success { "✅" } else { "❌" };
                                                summary_lines.push(format!("- {} `{}`", icon, cmd));
                                                if !cmd_res.output.trim().is_empty() {
                                                    summary_lines.push(format!("  ```\n  {}\n  ```", cmd_res.output.trim()));
                                                }
                                            }
                                            Err(e) => {
                                                summary_lines.push(format!("- ⚠️ Blocked `{}`: {}", cmd, e));
                                            }
                                        }
                                    }
                                }

                                let exec_summary = summary_lines.join("\n");
                                let combined_msg = format!("{}\n\n{}", final_ans, exec_summary);

                                state.write().thought_result = Some(result);
                                state.write().pending_patch = None;
                                state.write().thought_phase = None;
                                state.write().push_message(MessageRole::Assistant, combined_msg.clone(), Vec::new());

                                history_turns.push(BrainTurn {
                                    role: "user".to_string(),
                                    content: task_to_run.clone(),
                                });
                                history_turns.push(BrainTurn {
                                    role: "assistant".to_string(),
                                    content: combined_msg,
                                });
                                let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
                            } else {
                                state.write().thought_result = Some(result);
                                state.write().pending_patch = pending_patch;
                                state.write().thought_phase = None;
                                state.write().is_reasoning = false;
                                state.write().telemetry = TelemetryData {
                                    tokens_used: 0,
                                    prompt_tokens: 0,
                                    completion_tokens: 0,
                                    cost: 0.0,
                                    saved: 0.0,
                                    elapsed,
                                };

                                state.write().push_message(MessageRole::Assistant, final_ans.clone(), Vec::new());

                                history_turns.push(BrainTurn {
                                    role: "user".to_string(),
                                    content: task_to_run.clone(),
                                });
                                history_turns.push(BrainTurn {
                                    role: "assistant".to_string(),
                                    content: final_ans,
                                });
                                let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&history_turns, &sess_id, &root);
                            }

                            // Reload pods in state so any autonomously created pods or updated statuses immediately appear
                            state.write().pods = uneven_core::domain::pod::AgentPod::load_project_pods(&root);

                            let current_pod_id = state.read().active_pod_id.clone();
                            if let Some(ref pid) = current_pod_id {
                                let mut s = state.write();
                                if let Some(p) = s.pods.iter_mut().find(|p| &p.id == pid) {
                                    p.status = uneven_core::domain::pod::PodStatus::Completed;
                                    p.completed_tasks = (p.completed_tasks + 1).min(p.total_tasks.max(1));
                                    let _ = uneven_core::domain::pod::AgentPod::save_project_pods(&root, &s.pods);
                                }
                            }
                        }
                        Err(err) => {
                            state.write().push_message(
                                MessageRole::Assistant,
                                format!("⚠️ Thought loop encountered an error: {}", err),
                                Vec::new(),
                            );

                            let current_pod_id = state.read().active_pod_id.clone();
                            if let Some(ref pid) = current_pod_id {
                                let mut s = state.write();
                                if let Some(p) = s.pods.iter_mut().find(|p| &p.id == pid) {
                                    p.status = uneven_core::domain::pod::PodStatus::Failed;
                                }
                            }
                        }
                    }
                }
        }

        state.write().is_reasoning = false;
        state.write().thought_phase = None;
        state.write().active_subagent_info = None;
    });
}

#[component]
pub fn PromptInput() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut prompt_text = use_signal(String::new);
    let mut downloading_model = use_signal(|| None::<String>);
    let mut show_agent_dropdown = use_signal(|| false);
    let mut show_model_dropdown = use_signal(|| false);

    let is_busy = state.read().is_reasoning;
    let is_multi_agent = state.read().multi_agent_mode;
    let has_project = state.read().active_project.is_some();
    let active_pod = state.read().active_pod().cloned();
    let pods = state.read().pods.clone();
    let current_provider = state.read().active_provider.clone();
    let current_model = state.read().active_model.clone();

    let max_chars = uneven_core::infrastructure::translation::MAX_ALLOWED_PROMPT_CHARS;
    let char_count = prompt_text.read().len();
    let is_overflow = char_count > max_chars;
    let is_approaching_limit = char_count > (max_chars * 85 / 100);

    let placeholder_text = if !has_project {
        "Open a project folder to start coding with Uneven...".to_string()
    } else if is_busy {
        if is_multi_agent {
            "Prime Supervisor orchestrating multi-agent pipeline...".to_string()
        } else {
            "Thinking and auditing in progress...".to_string()
        }
    } else if downloading_model.read().is_some() {
        "Downloading local GGUF model files...".to_string()
    } else if let Some(ref pod) = active_pod {
        format!("Instruct {} for this Pod...", pod.prime_name)
    } else {
        "Type a prompt or task...".to_string()
    };


    let model_catalog: &[(&str, &str, &str, &str)] = &[
        ("gemini", "gemini-2.5-flash", "Gemini 2.5 Flash", "Medium"),
        ("gemini", "gemini-3.5-flash", "Gemini 3.5 Flash", "Fast"),
        ("gemini", "gemini-2.5-pro", "Gemini 2.5 Pro", "Large"),
        ("gemini", "gemini-2.5-flash-lite", "Gemini 2.5 Flash Lite", "Lite"),
        ("claude", "claude-3-7-sonnet", "Claude 3.7 Sonnet", "Large"),
        ("claude", "claude-sonnet-4-6", "Claude Sonnet 4.6", "Large"),
        ("claude", "claude-opus-4-8", "Claude Opus 4.8", "Max"),
        ("claude", "claude-3-5-haiku", "Claude 3.5 Haiku", "Fast"),
        ("openai", "gpt-4o", "GPT-4o", "Large"),
        ("openai", "gpt-4o-mini", "GPT-4o Mini", "Fast"),
        ("openai", "gpt-5.5", "GPT-5.5", "Max"),
        ("openai", "o3-mini", "o3-mini", "Reasoning"),
        ("deepseek", "deepseek-chat", "DeepSeek Chat (V3)", "Fast"),
        ("deepseek", "deepseek-reasoner", "DeepSeek Reasoner (R1)", "Reasoning"),
        ("deepseek", "deepseek-v4-flash", "DeepSeek V4 Flash", "Fast"),
        ("deepseek", "deepseek-v4-pro", "DeepSeek V4 Pro", "Large"),
        ("groq", "llama-3.3-70b-versatile", "Groq Llama 3.3 70B", "Fast"),
        ("mistral", "mistral-large-latest", "Mistral Large", "Large"),
        ("local", "llama-3.2-1b-q4", "Llama 3.2 1B Q4 (Local)", "Offline"),
        ("local", "qwen-2.5-1.5b-q8", "Qwen 2.5 1.5B Q8 (Local)", "Offline"),
        ("local", "deepseek-r1-1.5b-q8", "DeepSeek-R1 1.5B Q8 (Local)", "Offline"),
        ("ollama", "llama3", "Ollama (llama3)", "Local"),
    ];

    let (active_model_name, active_model_tier) = if current_provider == "default" || current_model == "default" {
        ("Auto (Smart Routing)", "Fast")
    } else {
        model_catalog
            .iter()
            .find(|(p, m, _, _)| *p == current_provider && *m == current_model)
            .map(|(_, _, name, tier)| (*name, *tier))
            .unwrap_or((current_model.as_str(), "Custom"))
    };

    let agent_label = if let Some(ref pod) = active_pod {
        pod.title.clone()
    } else {
        "Master Agent".to_string()
    };

    let line_count = prompt_text.read().split('\n').count().max(1);
    let textarea_rows = line_count.clamp(1, 8);

    rsx! {
        div { class: "w-full max-w-4xl mx-auto flex flex-col bg-[#121212] border border-[#222222] rounded-2xl px-3.5 py-2.5 shadow-2xl relative select-none mb-2",
            // Top Row: Agent Selector Pill (Left) & Subtle "+" File Attachment (Right)
            div { class: "flex items-center justify-between pb-1.5 mb-0.5 relative",
                // Agent Context Switcher Pill
                div { class: "relative flex items-center gap-1.5",
                    div {
                        class: "flex items-center gap-2 pl-3 pr-2.5 py-1 rounded-full text-xs font-mono font-medium bg-[#1c1c1c] hover:bg-[#262626] border border-[#2e2e2e] text-neutral-200 transition-colors cursor-pointer",
                        onclick: move |_| {
                            let curr = show_agent_dropdown();
                            show_agent_dropdown.set(!curr);
                            show_model_dropdown.set(false);
                        },
                        span { "{agent_label}" }
                        span { class: "text-neutral-500 text-[10px]", "▾" }

                        if active_pod.is_some() {
                            span {
                                class: "ml-0.5 w-4 h-4 rounded-full bg-neutral-800 hover:bg-neutral-600 hover:text-white text-neutral-400 flex items-center justify-center text-[9px] transition-all cursor-pointer",
                                title: "Return to main chat",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    state.write().select_pod(None);
                                },
                                "✕"
                            }
                        }
                    }

                    // Agent Switcher Dropdown Popover
                    if show_agent_dropdown() {
                        div { class: "absolute top-9 left-0 z-50 min-w-[220px] bg-[#161616] border border-[#2a2a2a] rounded-xl shadow-2xl py-1 text-xs font-mono",
                            // Option: Master Agent
                            div {
                                class: "px-3 py-2 hover:bg-[#222222] text-neutral-300 hover:text-white cursor-pointer flex items-center justify-between border-b border-[#222222]",
                                onclick: move |_| {
                                    state.write().select_pod(None);
                                    show_agent_dropdown.set(false);
                                },
                                span { "Master Agent" }
                                if active_pod.is_none() {
                                    span { class: "text-emerald-400 text-xs", "✓" }
                                }
                            }

                            // Options: Connected Pods
                            for pod in pods {
                                {
                                    let pid = pod.id.clone();
                                    let ptitle = pod.title.clone();
                                    let is_curr = active_pod.as_ref().map(|p| p.id == pid).unwrap_or(false);
                                    rsx! {
                                        div {
                                            key: "{pid}",
                                            class: "px-3 py-2 hover:bg-[#222222] text-neutral-300 hover:text-white cursor-pointer flex items-center justify-between",
                                            onclick: move |_| {
                                                state.write().select_pod(Some(pid.clone()));
                                                show_agent_dropdown.set(false);
                                            },
                                            span { "{ptitle}" }
                                            if is_curr {
                                                span { class: "text-emerald-400 text-xs", "✓" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Optional Patch Toggle Pill (Surfaces beside Master Agent when a patch is proposed)
                    if let Some(ref file_obj) = state.read().pending_patch {
                        {
                            let count = file_obj.patch.len();
                            let is_open = state.read().show_patch_viewer;
                            let count_str = if count == 1 { "1 file".to_string() } else { format!("{} files", count) };
                            let pill_class = if is_open {
                                "flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-mono font-medium bg-amber-950/40 hover:bg-amber-900/50 border border-amber-600/50 text-amber-300 transition-colors cursor-pointer"
                            } else {
                                "flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-mono font-medium bg-[#1c1c1c] hover:bg-[#262626] border border-amber-500/40 text-amber-400 transition-colors cursor-pointer"
                            };
                            rsx! {
                                button {
                                    class: "{pill_class}",
                                    title: "Show or hide proposal diffs",
                                    onclick: move |_| {
                                        let curr = state.read().show_patch_viewer;
                                        state.write().show_patch_viewer = !curr;
                                    },
                                    span { "📝" }
                                    span { "Patch ({count_str})" }
                                    span { class: "text-[10px] text-amber-400/70", if is_open { "▴" } else { "▾" } }
                                }
                            }
                        }
                    }

                    // Optional SQL Toggle Pill (Surfaces beside Master Agent when a SQL query is pending)
                    if state.read().pending_sql.is_some() {
                        {
                            let is_sql_open = state.read().show_sql_inspector;
                            let pill_class = if is_sql_open {
                                "flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-mono font-medium bg-emerald-950/50 hover:bg-emerald-900/60 border border-emerald-600/50 text-emerald-300 transition-colors cursor-pointer shadow-sm"
                            } else {
                                "flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-mono font-medium bg-[#1c1c1c] hover:bg-[#262626] border border-emerald-500/40 text-emerald-400 transition-colors cursor-pointer"
                            };
                            rsx! {
                                button {
                                    class: "{pill_class}",
                                    title: "Show or hide SQL approval card",
                                    onclick: move |_| {
                                        let curr = state.read().show_sql_inspector;
                                        state.write().show_sql_inspector = !curr;
                                    },
                                    span { "🗄" }
                                    span { "SQL Approval" }
                                    span { class: "text-[10px] text-emerald-400/70", if is_sql_open { "▴" } else { "▾" } }
                                }
                            }
                        }
                    }
                }

                // Subtle "+" Button to Attach Files
                div { class: "flex items-center gap-1.5",
                    if !state.read().attached_files.is_empty() {
                        span { class: "text-[11px] font-mono text-neutral-400",
                            "{state.read().attached_files.len()} attached"
                        }
                    }
                    button {
                        class: "w-7 h-7 rounded-lg border border-[#262626] bg-[#181818] hover:bg-[#222222] text-neutral-400 hover:text-white flex items-center justify-center text-sm font-mono transition-colors cursor-pointer active:scale-95",
                        title: "Browse and attach files (+)",
                        onclick: move |_| {
                            pick_and_attach_files(state);
                        },
                        "+"
                    }
                }
            }

            // Attached Files Chips
            if !state.read().attached_files.is_empty() {
                div { class: "flex flex-wrap gap-1.5 mb-2",
                    for (idx, f) in state.read().attached_files.iter().enumerate() {
                        span {
                            key: "{f}_{idx}",
                            class: "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-xs font-mono bg-[#1c1c1c] border border-[#2a2a2a] text-neutral-200",
                            span { "{f}" }
                            button {
                                class: "hover:text-rose-400 text-neutral-500 font-bold ml-1 transition-colors text-[10px] cursor-pointer",
                                onclick: move |_| {
                                    state.write().remove_attached_file(idx);
                                },
                                "✕"
                            }
                        }
                    }
                }
            }

            // Warning Banner if prompt exceeds max length
            if is_overflow {
                div { class: "flex items-center justify-between gap-2 px-3 py-1.5 mb-2 text-xs font-mono rounded-lg bg-rose-950/60 border border-rose-800 text-rose-300",
                    span { "Prompt length ({char_count}/{max_chars}) exceeds limit. Please use '+' to attach files." }
                    span { class: "text-rose-400 font-bold shrink-0", "+{char_count - max_chars} chars" }
                }
            }

            // Main Input Textarea
            textarea {
                class: "w-full bg-transparent border-0 resize-none text-sm text-neutral-100 placeholder-neutral-500 focus:outline-none font-sans py-0.5 leading-snug scrollbar-none transition-all",
                rows: "{textarea_rows}",
                placeholder: "{placeholder_text}",
                disabled: !has_project || is_busy || downloading_model.read().is_some(),
                value: "{prompt_text}",
                oninput: move |evt| prompt_text.set(evt.value()),
                onkeydown: move |evt| {
                    if evt.key() == Key::Enter && !evt.modifiers().contains(Modifiers::SHIFT) {
                        evt.prevent_default();
                        submit_prompt(state, prompt_text);
                    }
                }
            }

            // Bottom Controls Row: Model Selector Pill, Multi-Agent Toggle, Char Count, Send Button
            div { class: "flex items-center justify-between pt-2 border-t border-[#1c1c1c] mt-1 relative flex-wrap gap-2",
                // Left Controls: Model Trigger Pill & Multi-Agent Toggle
                div { class: "flex items-center gap-2 relative",
                    // Model Selection Pill: > Gemini 2.5 Flash • Medium ▾
                    div { class: "relative",
                        button {
                            class: "px-3 py-1 text-xs font-mono rounded-lg bg-[#181818] hover:bg-[#202020] border border-[#2a2a2a] text-neutral-300 hover:text-white flex items-center gap-1.5 cursor-pointer transition-all",
                            disabled: is_busy || downloading_model.read().is_some(),
                            onclick: move |_| {
                                let curr = show_model_dropdown();
                                show_model_dropdown.set(!curr);
                                show_agent_dropdown.set(false);
                            },
                            span { class: "text-neutral-500", ">" }
                            span { class: "font-medium", "{active_model_name}" }
                            span { class: "text-neutral-500", "•" }
                            span { class: "text-neutral-400 text-[11px]", "{active_model_tier}" }
                            span { class: "text-neutral-500 text-[10px] ml-0.5", "▾" }
                        }

                        // Model Selection Dropdown Menu
                        if show_model_dropdown() {
                            div { class: "absolute bottom-10 left-0 z-50 w-72 max-h-72 overflow-y-auto bg-[#161616] border border-[#2a2a2a] rounded-xl shadow-2xl py-1 text-xs font-mono scrollbar-none",
                                for (p, m, name, tier) in model_catalog.iter().copied() {
                                    {
                                        let is_sel = current_provider == p && current_model == m;
                                        let p_str = p.to_string();
                                        let m_str = m.to_string();
                                        rsx! {
                                            div {
                                                key: "{p}:{m}",
                                                class: "px-3 py-2 hover:bg-[#222222] text-neutral-300 hover:text-white cursor-pointer flex items-center justify-between",
                                                onclick: move |_| {
                                                    state.write().active_provider = p_str.clone();
                                                    state.write().active_model = m_str.clone();

                                                    if let Some(ref root) = state.read().project_root
                                                        && let Ok(mut cfg) = uneven_core::infrastructure::config::loader::load_config(root) {
                                                            cfg.brain.provider = p_str.clone();
                                                            cfg.brain.model = m_str.clone();
                                                            let _ = uneven_core::infrastructure::config::loader::save_config(root, &cfg);
                                                        }

                                                    if p_str == "local" && !is_local_model_downloaded(&m_str) {
                                                        downloading_model.set(Some(m_str.clone()));
                                                        let target = m_str.clone();
                                                        spawn(async move {
                                                            let _ = ensure_local_model_files(&target).await;
                                                            downloading_model.set(None);
                                                        });
                                                    }

                                                    show_model_dropdown.set(false);
                                                },
                                                div { class: "flex items-center gap-2",
                                                    span { "{name}" }
                                                    span { class: "text-[10px] text-neutral-500", "({tier})" }
                                                }
                                                if is_sel {
                                                    span { class: "text-emerald-400 text-xs", "✓" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Multi-Agent Toggle Pill
                    button {
                        class: if is_multi_agent {
                            "px-3 py-1 text-xs font-mono font-medium rounded-lg bg-emerald-950/40 border border-emerald-700/60 text-emerald-300 shadow-sm flex items-center gap-1.5 cursor-pointer transition-all"
                        } else {
                            "px-3 py-1 text-xs font-mono rounded-lg bg-[#181818] border border-[#2a2a2a] text-neutral-400 hover:text-neutral-200 hover:bg-[#222222] flex items-center gap-1.5 cursor-pointer transition-all"
                        },
                        disabled: is_busy,
                        title: "Toggle Multi-Agent Mode",
                        onclick: move |_| {
                            let curr = state.read().multi_agent_mode;
                            state.write().multi_agent_mode = !curr;
                        },
                        if is_multi_agent {
                            span { class: "w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" }
                            span { "Multi-Agent: ON" }
                        } else {
                            span { class: "w-1.5 h-1.5 rounded-full bg-neutral-600" }
                            span { "Multi-Agent: OFF" }
                        }
                    }
                }

                // Right Controls: Char Counter & Send Button
                div { class: "flex items-center gap-3",
                    span {
                        class: if is_overflow {
                            "text-[11px] font-mono text-rose-400 font-bold"
                        } else if is_approaching_limit {
                            "text-[11px] font-mono text-amber-400"
                        } else {
                            "text-[11px] font-mono text-neutral-500"
                        },
                        "{char_count}/{max_chars}"
                    }

                    button {
                        class: "px-4 py-1.5 bg-neutral-100 hover:bg-white text-black font-semibold rounded-lg text-xs font-mono transition-all flex items-center gap-1.5 cursor-pointer disabled:opacity-20 disabled:cursor-not-allowed shadow-sm active:scale-95",
                        disabled: !has_project || is_busy || downloading_model.read().is_some() || prompt_text.read().trim().is_empty() || is_overflow,
                        onclick: move |_| submit_prompt(state, prompt_text),
                        if is_busy {
                            span { class: "w-3 h-3 border-2 border-black/30 border-t-black rounded-full animate-spin" }
                            span { "Processing" }
                        } else {
                            span { "Send ↑" }
                        }
                    }
                }
            }
        }
    }
}

