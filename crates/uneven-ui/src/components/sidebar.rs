use super::file_tree::FileTree;
use crate::state::AppState;
use crate::utils::icon_logo_data_uri;
use crate::utils::project::pick_and_open_project;
use dioxus::prelude::*;

#[component]
pub fn Sidebar() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut active_tab = use_signal(|| "chats"); // "chats" | "files"
    let current_state = state.read();
    let active_proj = current_state.active_project.clone();
    let recent_projects = current_state.recent_projects.clone();
    let sessions = current_state.sessions.clone();
    let active_session = current_state.active_session.clone();

    rsx! {
        aside { class: "w-60 bg-[#0e0e0e] border-r border-[#1c1c1c] flex flex-col justify-between select-none overflow-hidden shrink-0",
            div { class: "p-3 flex flex-col gap-4 overflow-y-auto flex-1 min-h-0",
                // + Open Project button (Triggers native OS directory picker)
                button {
                    class: "w-full py-2 px-3 rounded-xl border border-[#262626] bg-[#141414] hover:bg-[#1a1a1a] hover:border-[#383838] text-neutral-200 text-xs font-medium transition-all flex items-center justify-center gap-1.5 cursor-pointer shadow-sm active:scale-95",
                    title: "Open a folder on your computer",
                    onclick: move |_| {
                        pick_and_open_project(state);
                    },
                    span { class: "text-neutral-400 text-sm", "+" }
                    span { "Open Project" }
                }

                // Projects section
                div { class: "flex flex-col gap-1.5 flex-1 min-h-0",
                    div { class: "px-2 py-1 text-[11px] font-semibold tracking-wider text-neutral-500 uppercase font-sans",
                        "Projects"
                    }

                    if recent_projects.is_empty() {
                        div { class: "px-2.5 py-3 rounded-lg border border-dashed border-[#202020] text-center flex flex-col items-center gap-1",
                            span { class: "text-[11px] text-neutral-500 font-sans", "No projects connected." }
                            span { class: "text-[10px] text-neutral-600 font-sans", "Click '+ Open Project' to select a folder on your machine." }
                        }
                    } else {
                        div { class: "flex flex-col gap-1",
                            for proj in recent_projects {
                                {
                                    let is_active = active_proj.as_ref().map(|p| p.path == proj.path).unwrap_or(false);
                                    let proj_path_open = proj.path.clone();
                                    let proj_path_remove = proj.path.clone();
                                    let proj_path_display = proj.path.display().to_string();
                                    let proj_name = proj.name.clone();

                                    rsx! {
                                        div { key: "{proj_path_display}", class: "flex flex-col group",
                                            // Project item
                                            div {
                                                class: if is_active {
                                                    "w-full px-2.5 py-1.5 rounded-lg bg-[#1a1a1a] border border-[#2c2c2c] text-white text-xs font-mono font-medium transition-colors flex items-center justify-between cursor-pointer"
                                                } else {
                                                    "w-full px-2.5 py-1.5 rounded-lg text-neutral-400 hover:text-white hover:bg-[#141414] text-xs font-mono transition-colors flex items-center justify-between cursor-pointer"
                                                },
                                                title: "{proj_path_display}",
                                                onclick: move |_| {
                                                    state.write().open_project(proj_path_open.clone());
                                                },
                                                span { class: "truncate flex-1 font-sans font-medium text-xs", "{proj_name}" }
                                                button {
                                                    class: "text-neutral-600 hover:text-rose-400 text-[10px] ml-1 opacity-0 group-hover:opacity-100 transition-opacity p-0.5",
                                                    title: "Remove from recent list",
                                                    onclick: move |e| {
                                                        e.stop_propagation();
                                                        state.write().remove_recent_project(&proj_path_remove);
                                                    },
                                                    "✕"
                                                }
                                            }

                                            // Sub-items (Tabs: Chats vs Files) shown for active project
                                            if is_active {
                                                div { class: "flex flex-col pl-2.5 pt-1.5 pb-1 gap-1 border-l border-[#202020] ml-2.5 mt-1",
                                                    // Tab Switcher: Chats vs Files
                                                    div { class: "flex items-center gap-1 p-0.5 bg-[#141414] rounded-lg border border-[#222222] mb-1.5",
                                                        button {
                                                            class: if *active_tab.read() == "chats" {
                                                                "flex-1 py-1 px-1 rounded-md bg-[#222222] text-neutral-200 text-[10px] font-mono font-medium transition-colors cursor-pointer text-center"
                                                            } else {
                                                                "flex-1 py-1 px-1 rounded-md text-neutral-500 hover:text-neutral-300 text-[10px] font-mono transition-colors cursor-pointer text-center"
                                                            },
                                                            onclick: move |_| active_tab.set("chats"),
                                                            "Chats"
                                                        }
                                                        button {
                                                            class: if *active_tab.read() == "files" {
                                                                "flex-1 py-1 px-1 rounded-md bg-[#222222] text-neutral-200 text-[10px] font-mono font-medium transition-colors cursor-pointer text-center"
                                                            } else {
                                                                "flex-1 py-1 px-1 rounded-md text-neutral-500 hover:text-neutral-300 text-[10px] font-mono transition-colors cursor-pointer text-center"
                                                            },
                                                            onclick: move |_| active_tab.set("files"),
                                                            "Files"
                                                        }
                                                    }

                                                    if *active_tab.read() == "chats" {
                                                        // Header row for conversations with "+" button
                                                        div { class: "flex items-center justify-between px-1 text-[10px] font-mono text-neutral-500 mb-0.5",
                                                            span { "Conversations" }
                                                            button {
                                                                class: "hover:text-white text-neutral-400 p-0.5 rounded cursor-pointer transition-colors text-xs font-mono",
                                                                title: "Start new conversation (+)",
                                                                onclick: move |_| {
                                                                    state.write().create_new_session();
                                                                },
                                                                "+"
                                                            }
                                                        }

                                                        for sess in &sessions {
                                                            {
                                                                let is_sess_active = sess == &active_session;
                                                                let sess_id = sess.clone();
                                                                let sess_click = sess.clone();
                                                                let sess_del = sess.clone();
                                                                let display_title = state.read().session_title(&sess_id);

                                                                rsx! {
                                                                    div {
                                                                        key: "{sess_id}",
                                                                        class: if is_sess_active {
                                                                            "group/sess w-full px-2 py-1 rounded text-[11px] font-mono text-neutral-200 bg-[#1e1e1e] border border-[#2e2e2e] transition-colors flex items-center justify-between cursor-pointer"
                                                                        } else {
                                                                            "group/sess w-full px-2 py-1 rounded text-[11px] font-mono text-neutral-500 hover:text-neutral-300 hover:bg-[#141414] transition-colors flex items-center justify-between cursor-pointer"
                                                                        },
                                                                        onclick: move |_| {
                                                                            state.write().switch_session(sess_click.clone());
                                                                        },
                                                                        div { class: "flex items-center gap-1.5 truncate flex-1",
                                                                            span { class: if is_sess_active { "text-emerald-400 text-[8px]" } else { "text-neutral-600 text-[8px]" }, "•" }
                                                                            span { class: "truncate text-[11px]", title: "{display_title}", "{display_title}" }
                                                                        }
                                                                        button {
                                                                            class: "text-neutral-600 hover:text-rose-400 text-[10px] ml-1 opacity-0 group-hover/sess:opacity-100 transition-opacity p-0.5 cursor-pointer",
                                                                            title: "Delete conversation",
                                                                            onclick: move |e| {
                                                                                e.stop_propagation();
                                                                                state.write().delete_session(&sess_del);
                                                                            },
                                                                            "✕"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        // Render Integrated FileTree
                                                        FileTree {}
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

            // Bottom subtle branding / engine state
            div { class: "p-3 border-t border-[#1c1c1c] text-[10px] text-neutral-500 font-mono flex items-center justify-between shrink-0",
                div { class: "flex items-center gap-1.5",
                    img {
                        src: icon_logo_data_uri(),
                        class: "w-3.5 h-3.5 rounded object-contain shrink-0",
                        alt: "Uneven Core",
                    }
                    span { "Uneven Core" }
                }
                span { class: "w-1.5 h-1.5 rounded-full bg-emerald-500" }
            }
        }
    }
}
