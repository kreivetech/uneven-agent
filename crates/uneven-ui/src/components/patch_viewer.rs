use crate::state::{AppState, ChatMessage, MessageRole};
use crate::utils::diff::{compute_patch_diff, DiffLineType};
use dioxus::prelude::*;
use uneven_core::infrastructure::patch::{
    apply_file_actions, PatchActionKind, PatchActionResult,
};

#[component]
pub fn PatchViewer() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut selected_index = use_signal(|| 0usize);
    let mut is_applying = use_signal(|| false);

    let current_state = state.read();
    let pending = current_state.pending_patch.clone();

    match pending {
        None => rsx! {
            div { class: "h-full flex flex-col bg-neutral-900 border border-neutral-800 rounded-xl overflow-hidden select-none",
                // Header Bar
                div { class: "px-4 py-3 border-b border-neutral-800 bg-neutral-900/80 flex items-center justify-between",
                    div { class: "flex items-center gap-2",
                        span { class: "text-xs font-semibold text-neutral-300 uppercase tracking-wider",
                            "Patch & Audit Inspector"
                        }
                    }
                    span { class: "text-[10px] text-neutral-500 font-mono", "No pending diffs" }
                }

                // Empty State Body
                div { class: "flex-1 p-6 flex flex-col items-center justify-center text-center text-neutral-500 text-xs min-h-0",
                    div { class: "w-12 h-12 rounded-xl bg-neutral-950 border border-neutral-800 flex items-center justify-center mb-3 text-neutral-600 text-lg",
                        "◈"
                    }
                    p { class: "text-neutral-300 font-medium mb-1", "No Pending Mutations" }
                    p { class: "text-neutral-500 max-w-xs text-[11px]",
                        "Patches synthesized by the Think engine will appear here with surgical diffs for verification before being applied to disk."
                    }
                }
            }
        },
        Some(file_obj) => {
            let patches_len = file_obj.patch.len();
            let safe_idx = if *selected_index.read() >= patches_len {
                0
            } else {
                *selected_index.read()
            };
            let current_patch = file_obj.patch.get(safe_idx).cloned();

            let (diff_lines, stats) = match current_patch.as_ref() {
                Some(p) => compute_patch_diff(p),
                None => (Vec::new(), crate::utils::diff::DiffStats::default()),
            };

            let file_label = if patches_len == 1 { "file" } else { "files" };
            let applying = *is_applying.read();

            rsx! {
                div { class: "h-full flex flex-col bg-neutral-900 border border-neutral-800 rounded-xl overflow-hidden select-none min-h-0",
                    // Header with Status and Action Buttons
                    div { class: "px-4 py-2.5 border-b border-neutral-800 bg-neutral-900 flex items-center justify-between gap-2 flex-wrap",
                        div { class: "flex items-center gap-2",
                            span { class: "text-xs font-semibold text-neutral-200 uppercase tracking-wider",
                                "Patch Inspector"
                            }
                            span { class: "px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/20 text-amber-300 border border-amber-600/30",
                                "{patches_len} {file_label}"
                            }
                        }

                        div { class: "flex items-center gap-2",
                            // Hide / Minimize Button
                            button {
                                class: "px-2.5 py-1 text-xs font-medium rounded-lg text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800 border border-neutral-700/60 transition-all cursor-pointer",
                                title: "Hide patch panel (it can be reopened via the Patch button)",
                                onclick: move |_| {
                                    state.write().show_patch_viewer = false;
                                },
                                "✕ Hide"
                            }

                            // Discard Button
                            button {
                                class: "px-2.5 py-1 text-xs font-medium rounded-lg text-neutral-400 hover:text-rose-300 hover:bg-rose-950/40 border border-neutral-700/60 transition-all cursor-pointer disabled:opacity-40",
                                disabled: applying,
                                onclick: move |_| {
                                    state.write().pending_patch = None;
                                    state.write().show_patch_viewer = false;
                                    state.write().messages.push(ChatMessage::new(
                                        MessageRole::Assistant,
                                        "🚫 Pending patches were discarded by user.",
                                        Vec::new(),
                                    ));
                                },
                                "Discard"
                            }

                            // Apply All Changes Button
                            button {
                                class: "px-3 py-1 text-xs font-semibold rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white shadow-md shadow-emerald-950/40 flex items-center gap-1.5 transition-all cursor-pointer active:scale-95 disabled:opacity-50",
                                disabled: applying,
                                onclick: {
                                    let file_obj_clone = file_obj.clone();
                                    move |_| {
                                        is_applying.set(true);
                                        let file_obj_to_apply = file_obj_clone.clone();
                                        spawn(async move {
                                            let root = state.read().project_root.clone().unwrap_or_else(|| std::path::PathBuf::from("."));
                                            let results: Vec<PatchActionResult> =
                                                apply_file_actions(&root, &file_obj_to_apply);

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
                                                        report_lines.push(format!("- Renamed: `{}` -> `{}`", from, to));
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
                                            state.write().messages.push(ChatMessage::new(
                                                MessageRole::Assistant,
                                                message_text,
                                                Vec::new(),
                                            ));

                                            is_applying.set(false);
                                        });
                                    }
                                },
                                if applying {
                                    span { class: "w-3 h-3 border-2 border-white/30 border-t-white rounded-full animate-spin inline-block" }
                                    "Applying..."
                                } else {
                                    span { "✓" }
                                    "Apply All Changes"
                                }
                            }
                        }
                    }

                    // Optional Summary Banner
                    if let Some(ref summary) = file_obj.summary {
                        div { class: "px-4 py-2 bg-neutral-950/60 border-b border-neutral-800 text-xs text-neutral-300 italic flex items-center gap-2",
                            span { class: "text-cyan-400 not-italic font-bold font-mono text-[10px]", "SUMMARY:" }
                            span { class: "truncate", "{summary}" }
                        }
                    }

                    // File Tabs Bar
                    if patches_len > 1 {
                        div { class: "flex items-center gap-1.5 px-3 py-2 border-b border-neutral-800 bg-neutral-950/40 overflow-x-auto",
                            for (idx, patch) in file_obj.patch.iter().enumerate() {
                                button {
                                    key: "{patch.path}_{idx}",
                                    class: if idx == safe_idx {
                                        "px-2.5 py-1 text-xs font-mono font-medium rounded-md bg-neutral-800 text-cyan-300 border border-neutral-700 shadow-sm flex items-center gap-1.5 shrink-0 cursor-pointer"
                                    } else {
                                        "px-2.5 py-1 text-xs font-mono rounded-md text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800/50 border border-transparent flex items-center gap-1.5 shrink-0 cursor-pointer"
                                    },
                                    onclick: move |_| selected_index.set(idx),
                                    span { class: "text-[10px] px-1 py-0.2 rounded bg-neutral-900 border border-neutral-800 uppercase text-neutral-400",
                                        "{patch.mode}"
                                    }
                                    span { class: "truncate max-w-[140px]", "{patch.path}" }
                                }
                            }
                        }
                    }

                    // Selected File Diff View
                    if let Some(ref patch) = current_patch {
                        div { class: "flex-1 flex flex-col min-h-0 overflow-hidden",
                            // Subheader: Path & Mode
                            div { class: "px-4 py-2 bg-neutral-950/80 border-b border-neutral-800 flex items-center justify-between text-xs font-mono",
                                div { class: "flex items-center gap-2 truncate",
                                    span { class: "text-neutral-200 font-semibold truncate", "{patch.path}" }
                                    span {
                                        class: match patch.mode.to_lowercase().as_str() {
                                            "create" => "px-1.5 py-0.5 rounded text-[10px] bg-emerald-950 border border-emerald-800 text-emerald-300 font-bold uppercase",
                                            "delete" => "px-1.5 py-0.5 rounded text-[10px] bg-rose-950 border border-rose-800 text-rose-300 font-bold uppercase",
                                            "rename" => "px-1.5 py-0.5 rounded text-[10px] bg-amber-950 border border-amber-800 text-amber-300 font-bold uppercase",
                                            _ => "px-1.5 py-0.5 rounded text-[10px] bg-cyan-950 border border-cyan-800 text-cyan-300 font-bold uppercase",
                                        },
                                        "{patch.mode}"
                                    }
                                }

                                // Additions / Deletions Stats
                                div { class: "flex items-center gap-2 shrink-0 font-medium text-[11px]",
                                    if stats.additions > 0 {
                                        span { class: "text-emerald-400", "+{stats.additions}" }
                                    }
                                    if stats.deletions > 0 {
                                        span { class: "text-rose-400", "-{stats.deletions}" }
                                    }
                                }
                            }

                            // Diff Lines Stream
                            div { class: "flex-1 overflow-auto bg-neutral-950 p-2 font-mono text-[11px] leading-relaxed",
                                if diff_lines.is_empty() {
                                    div { class: "p-4 text-center text-neutral-500 italic",
                                        "No modifications detected in this patch chunk."
                                    }
                                } else {
                                    div { class: "flex flex-col min-w-full",
                                        for (i, line) in diff_lines.into_iter().enumerate() {
                                            match line.line_type {
                                                DiffLineType::Add => rsx! {
                                                    div {
                                                        key: "{i}",
                                                        class: "flex items-stretch bg-emerald-950/40 text-emerald-300 border-l-2 border-emerald-500 hover:bg-emerald-950/60",
                                                        span { class: "w-8 text-right pr-2 text-neutral-600 select-none shrink-0",
                                                            ""
                                                        }
                                                        span { class: "w-8 text-right pr-2 text-emerald-500/70 select-none shrink-0",
                                                            "{line.new_num.map(|n| n.to_string()).unwrap_or_default()}"
                                                        }
                                                        span { class: "w-4 text-center text-emerald-400 select-none shrink-0 font-bold",
                                                            "+"
                                                        }
                                                        span { class: "whitespace-pre flex-1 pl-1", "{line.text}" }
                                                    }
                                                },
                                                DiffLineType::Delete => rsx! {
                                                    div {
                                                        key: "{i}",
                                                        class: "flex items-stretch bg-rose-950/40 text-rose-300 border-l-2 border-rose-500 hover:bg-rose-950/60",
                                                        span { class: "w-8 text-right pr-2 text-rose-500/70 select-none shrink-0",
                                                            "{line.old_num.map(|n| n.to_string()).unwrap_or_default()}"
                                                        }
                                                        span { class: "w-8 text-right pr-2 text-neutral-600 select-none shrink-0",
                                                            ""
                                                        }
                                                        span { class: "w-4 text-center text-rose-400 select-none shrink-0 font-bold",
                                                            "-"
                                                        }
                                                        span { class: "whitespace-pre flex-1 pl-1 line-through decoration-rose-500/40",
                                                            "{line.text}"
                                                        }
                                                    }
                                                },
                                                DiffLineType::Context => rsx! {
                                                    div {
                                                        key: "{i}",
                                                        class: "flex items-stretch text-neutral-400 hover:bg-neutral-900/50 border-l-2 border-transparent",
                                                        span { class: "w-8 text-right pr-2 text-neutral-600 select-none shrink-0",
                                                            "{line.old_num.map(|n| n.to_string()).unwrap_or_default()}"
                                                        }
                                                        span { class: "w-8 text-right pr-2 text-neutral-600 select-none shrink-0",
                                                            "{line.new_num.map(|n| n.to_string()).unwrap_or_default()}"
                                                        }
                                                        span { class: "w-4 text-center text-neutral-600 select-none shrink-0",
                                                            " "
                                                        }
                                                        span { class: "whitespace-pre flex-1 pl-1", "{line.text}" }
                                                    }
                                                },
                                                DiffLineType::Header => rsx! {
                                                    div {
                                                        key: "{i}",
                                                        class: "px-3 py-1.5 my-1 bg-neutral-900 border border-neutral-800 rounded text-neutral-200 font-semibold text-xs",
                                                        "{line.text}"
                                                    }
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
