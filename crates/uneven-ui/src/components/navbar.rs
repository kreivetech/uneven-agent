use crate::routes::Route;
use crate::state::AppState;
use crate::utils::icon_logo_data_uri;
use crate::utils::project::pick_and_open_project;
use dioxus::prelude::*;

#[component]
pub fn Navbar() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current_state = state.read();
    let active_project = current_state.active_project.clone();
    let show_pods = current_state.show_pods;
    let show_terminal = current_state.show_terminal;

    rsx! {
        header { class: "h-12 bg-[#0c0c0c] border-b border-[#1c1c1c] px-4 flex items-center justify-between text-neutral-200 select-none z-10 shrink-0",
            // Left: Minimal clean logo with Crystal Icon
            div { class: "flex items-center gap-2.5",
                img {
                    src: icon_logo_data_uri(),
                    class: "w-5 h-5 rounded-md object-contain shrink-0",
                    alt: "Uneven Icon",
                }
                span { class: "text-white font-bold text-sm tracking-tight font-sans", "Uneven" }
            }

            // Center: Active Project Pill / Open Project trigger
            div { class: "flex items-center",
                if let Some(ref proj) = active_project {
                    div {
                        class: "px-3 py-1 rounded-lg bg-[#161616] border border-[#262626] text-xs font-mono text-neutral-300 flex items-center gap-1.5 shadow-sm cursor-pointer hover:border-neutral-700 transition-colors",
                        title: "{proj.path.display()}",
                        onclick: move |_| {
                            pick_and_open_project(state);
                        },
                        span { "{proj.name}" }
                        span { class: "text-neutral-500 text-[10px]", "▾" }
                    }
                } else {
                    button {
                        class: "px-3 py-1 rounded-lg bg-[#161616] hover:bg-[#202020] border border-[#262626] hover:border-neutral-600 text-xs font-mono text-neutral-400 hover:text-white flex items-center gap-1.5 shadow-sm cursor-pointer transition-all active:scale-95",
                        onclick: move |_| {
                            pick_and_open_project(state);
                        },
                        span { "+ Open Project" }
                        span { class: "text-neutral-500 text-[10px]", "▾" }
                    }
                }
            }


            // Right: Update button (when available), Config link & Terminal/Pods toggles
            div { class: "flex items-center gap-2 text-xs font-sans",
                if current_state.available_ide_update.is_some() {
                    {
                        let is_updating = current_state.is_updating_ide;
                        rsx! {
                            button {
                                class: "px-2.5 py-1 rounded-lg bg-emerald-950/60 hover:bg-emerald-900/60 border border-emerald-700/50 text-emerald-300 font-medium cursor-pointer transition-colors disabled:opacity-50",
                                disabled: is_updating,
                                onclick: move |_| {
                                    state.write().is_updating_ide = true;
                                    spawn(async move {
                                        let res = tokio::spawn(async move {
                                            uneven_core::application::UpdateService::apply_ide_update().await
                                        })
                                        .await
                                        .unwrap_or_else(|_| {
                                            Err(uneven_core::UnevenError::ConfigError(
                                                "[UE-01999999] Update task failed.".to_string(),
                                            ))
                                        });

                                        match res {
                                            Ok(msg) => {
                                                state.write().available_ide_update = None;
                                                state.write().push_message(
                                                    crate::state::MessageRole::Assistant,
                                                    msg,
                                                    Vec::new(),
                                                );
                                            }
                                            Err(e) => {
                                                state.write().push_message(
                                                    crate::state::MessageRole::Assistant,
                                                    e.to_string(),
                                                    Vec::new(),
                                                );
                                            }
                                        }
                                        state.write().is_updating_ide = false;
                                    });
                                },
                                if is_updating { "update now" } else { "update available" }
                            }
                        }
                    }
                }
                {
                    let route = use_route::<Route>();
                    let is_settings = route == Route::SettingsPage {};
                    rsx! {
                        Link {
                            to: if is_settings { Route::CockpitPage {} } else { Route::SettingsPage {} },
                            class: if is_settings {
                                "px-2.5 py-1 text-white bg-[#1f1f1f] border border-[#303030] rounded-lg transition-colors font-medium cursor-pointer"
                            } else {
                                "px-2.5 py-1 text-neutral-400 hover:text-white transition-colors font-medium cursor-pointer"
                            },
                            "Config"
                        }
                    }
                }

                button {
                    class: if show_terminal {
                        "px-2.5 py-1 rounded-lg bg-[#222222] border border-[#383838] text-emerald-400 font-medium transition-all shadow-sm flex items-center gap-1.5 cursor-pointer font-mono"
                    } else {
                        "px-2.5 py-1 rounded-lg bg-transparent border border-transparent hover:border-[#262626] text-neutral-400 hover:text-white transition-all flex items-center gap-1.5 cursor-pointer font-mono"
                    },
                    title: "Toggle Integrated Terminal",
                    onclick: move |_| {
                        let cur = state.read().show_terminal;
                        state.write().show_terminal = !cur;
                    },
                    span { class: "font-bold", ">_" }
                    span { "Terminal" }
                }

                button {
                    class: if show_pods {
                        "px-3 py-1 rounded-lg bg-[#222222] border border-[#383838] text-white font-medium transition-all shadow-sm flex items-center gap-1.5 cursor-pointer"
                    } else {
                        "px-3 py-1 rounded-lg bg-transparent border border-transparent hover:border-[#262626] text-neutral-400 hover:text-white transition-all flex items-center gap-1.5 cursor-pointer"
                    },
                    onclick: move |_| {
                        let cur = state.read().show_pods;
                        state.write().show_pods = !cur;
                    },
                    span { "Pods" }
                }
            }
        }
    }
}
