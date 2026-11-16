use crate::state::{AppState, MessageRole};
use dioxus::prelude::*;
use std::sync::Arc;
use uneven_core::infrastructure::knowledge::analysis_service::is_safe_query;
use uneven_core::infrastructure::knowledge::DatabaseAnalyst;
use uneven_core::infrastructure::reporting::generate_dashboard;
use uneven_core::infrastructure::security::SecurityContext;

#[component]
pub fn SqlInspector() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut is_executing = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);

    let current_state = state.read();
    let pending = current_state.pending_sql.clone();

    let q = match pending {
        Some(q) => q,
        None => return rsx! {},
    };

    let sql_text = q.current_sql.clone();
    let original_sql = q.original_sql.clone();
    let is_modified = sql_text.trim() != original_sql.trim();
    let is_safe = is_safe_query(&sql_text);
    let executing = *is_executing.read();
    let current_err = error_msg.read().clone();

    let sql_for_exec = sql_text.clone();
    let visual_sug = q.visual_suggestions.clone();

    rsx! {
        div { class: "w-full flex flex-col bg-[#111111] border border-[#2a2a2a] rounded-xl overflow-hidden shadow-2xl select-none mb-3 transition-all",
            // Header Bar
            div { class: "px-4 py-2.5 bg-[#161616] border-b border-[#222222] flex items-center justify-between gap-3 flex-wrap",
                div { class: "flex items-center gap-2",
                    span { class: "text-emerald-400 text-sm", "🗄" }
                    span { class: "text-xs font-semibold text-neutral-200 tracking-wide",
                        "SQL Query Inspector & Verification"
                    }
                    span { class: "px-2 py-0.5 rounded text-[10px] font-mono font-medium bg-amber-500/20 text-amber-300 border border-amber-500/30",
                        "Approval Required"
                    }
                    if is_modified {
                        span { class: "px-2 py-0.5 rounded text-[10px] font-mono font-medium bg-cyan-950/60 text-cyan-300 border border-cyan-700/40",
                            "Edited by User"
                        }
                    }
                }

                // Actions: Reset, Cancel, Execute
                div { class: "flex items-center gap-2",
                    if is_modified {
                        button {
                            class: "px-2.5 py-1 text-xs font-mono rounded-lg bg-[#202020] hover:bg-[#282828] border border-[#333333] text-neutral-300 hover:text-white transition-colors cursor-pointer flex items-center gap-1",
                            title: "Revert edits to original query suggested by AI",
                            onclick: move |_| {
                                if let Some(ref mut cur_q) = state.write().pending_sql {
                                    cur_q.current_sql = cur_q.original_sql.clone();
                                }
                            },
                            span { "↺" }
                            span { "Reset" }
                        }
                    }

                    button {
                        class: "px-2.5 py-1 text-xs font-mono rounded-lg bg-[#202020] hover:bg-[#282828] border border-[#333333] text-neutral-400 hover:text-white transition-colors cursor-pointer flex items-center gap-1",
                        title: "Dismiss query without touching the database",
                        onclick: move |_| {
                            state.write().pending_sql = None;
                            state.write().show_sql_inspector = false;
                        },
                        span { "✕" }
                        span { "Dismiss" }
                    }

                    button {
                        class: if !is_safe || executing {
                            "px-3 py-1 text-xs font-mono font-medium rounded-lg bg-neutral-800 text-neutral-500 border border-neutral-700 cursor-not-allowed flex items-center gap-1.5 opacity-60"
                        } else {
                            "px-3 py-1 text-xs font-mono font-semibold rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white shadow-md cursor-pointer transition-all active:scale-95 flex items-center gap-1.5"
                        },
                        disabled: !is_safe || executing,
                        title: if !is_safe { "Query contains restricted keywords" } else { "Execute query and compile interactive HTML dashboard" },
                        onclick: move |_| {
                            if !is_safe || executing {
                                return;
                            }
                            is_executing.set(true);
                            error_msg.set(None);

                            let sql_run = sql_for_exec.clone();
                            let visual_run = visual_sug.clone();
                            let root_opt = state.read().project_root.clone();

                            spawn(async move {
                                let root = match root_opt {
                                    Some(r) => r,
                                    None => {
                                        error_msg.set(Some("Please open a project directory first.".to_string()));
                                        is_executing.set(false);
                                        return;
                                    }
                                };

                                let db_url = std::env::var("DATABASE_URL")
                                    .or_else(|_| std::env::var("UNEVEN_DB_URL"))
                                    .unwrap_or_default();

                                if db_url.trim().is_empty() {
                                    error_msg.set(Some("Database URL is not configured. Please set DATABASE_URL in Settings.".to_string()));
                                    is_executing.set(false);
                                    return;
                                }

                                let config = uneven_core::infrastructure::config::loader::load_config(&root).unwrap_or_default();
                                let security = Arc::new(SecurityContext::new(
                                    config.security.blocked_columns.clone(),
                                    config.security.blocked_tables.clone(),
                                    config.security.masked_columns.clone(),
                                ));

                                let mut analyst = DatabaseAnalyst::new(security.clone(), Vec::new());
                                if let Err(e) = analyst.connect(&db_url).await {
                                    error_msg.set(Some(format!("Database connection failed: {}", e)));
                                    is_executing.set(false);
                                    return;
                                }

                                let rows = match analyst.execute_query(&sql_run).await {
                                    Ok(r) => r,
                                    Err(e) => {
                                        error_msg.set(Some(format!("SQL query execution failed: {}", e)));
                                        is_executing.set(false);
                                        return;
                                    }
                                };

                                let dashboard_html = generate_dashboard(
                                    &rows,
                                    "Uneven Analytics Dashboard",
                                    "dark",
                                    visual_run.as_ref(),
                                );

                                let reports_dir = root.join(".uneven").join("reports");
                                let _ = std::fs::create_dir_all(&reports_dir);
                                let timestamp = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                let file_name = format!("dashboard_{}.html", timestamp);
                                let full_path = reports_dir.join(&file_name);
                                let rel_path = format!(".uneven/reports/{}", file_name);

                                if let Err(e) = std::fs::write(&full_path, &dashboard_html) {
                                    error_msg.set(Some(format!("Failed to save report to disk: {}", e)));
                                    is_executing.set(false);
                                    return;
                                }

                                let summary_msg = format!(
                                    "📊 **Interactive Analytics Dashboard Ready!**\n\n\
                                    - **Report Path:** `{}`\n\
                                    - **Rows Fetched:** {}\n\
                                    - **Executed Query:**\n```sql\n{}\n```\n\n\
                                    *The dashboard has been automatically opened in your editor tab viewer.*",
                                    rel_path,
                                    rows.len(),
                                    sql_run
                                );

                                state.write().push_message(MessageRole::Assistant, summary_msg, Vec::new());
                                state.write().open_file_tab(rel_path);
                                state.write().pending_sql = None;
                                state.write().show_sql_inspector = false;
                                is_executing.set(false);
                            });
                        },
                        if executing {
                            span { "⚙ Executing…" }
                        } else {
                            span { "▶ Run & Build Dashboard" }
                        }
                    }
                }
            }

            // AI Objective & Explanation Section
            div { class: "px-4 py-2.5 bg-[#141414] border-b border-[#1f1f1f] flex flex-col gap-1 text-xs font-mono",
                if !q.goal.is_empty() {
                    div { class: "flex items-baseline gap-2",
                        span { class: "text-neutral-500 uppercase text-[10px] font-bold tracking-wider", "Goal:" }
                        span { class: "text-neutral-200", "{q.goal}" }
                    }
                }
                if !q.explanation.is_empty() {
                    div { class: "flex items-baseline gap-2",
                        span { class: "text-neutral-500 uppercase text-[10px] font-bold tracking-wider", "AI Logic:" }
                        span { class: "text-neutral-400 text-[11px]", "{q.explanation}" }
                    }
                }
            }

            // Interactive Monospace SQL Editor
            div { class: "relative bg-[#0b0b0b] p-1",
                textarea {
                    class: "w-full min-h-[120px] max-h-[260px] bg-transparent text-emerald-300 font-mono text-xs p-3 focus:outline-none resize-y selection:bg-emerald-950/80 leading-relaxed border-0",
                    spellcheck: "false",
                    autocapitalize: "off",
                    autocorrect: "off",
                    value: "{sql_text}",
                    oninput: move |evt| {
                        let new_val = evt.value();
                        if let Some(ref mut cur_q) = state.write().pending_sql {
                            cur_q.current_sql = new_val;
                        }
                    }
                }
            }

            // Footer Safety Status & Error Message
            div { class: "px-4 py-2 bg-[#141414] border-t border-[#1f1f1f] flex items-center justify-between text-xs font-mono flex-wrap gap-2",
                div { class: "flex items-center gap-2",
                    if is_safe {
                        span { class: "text-emerald-400 flex items-center gap-1.5 text-[11px]",
                            span { "✓" }
                            span { "Safe Read-Only Query (SELECT/WITH/PRAGMA)" }
                        }
                    } else {
                        span { class: "text-rose-400 flex items-center gap-1.5 text-[11px] font-semibold",
                            span { "⚠️" }
                            span { "Restricted: Only read queries (SELECT, WITH, PRAGMA) are permitted." }
                        }
                    }
                }

                if let Some(ref err) = current_err {
                    span { class: "text-rose-400 text-[11px] truncate max-w-md", "Error: {err}" }
                }
            }
        }
    }
}
