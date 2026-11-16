use crate::components::{
    FileEditor, MessageBubble, PatchViewer, PodFlow, PromptInput, SqlInspector, ThinkStepper,
};
use crate::state::AppState;
use crate::utils::icon_logo_data_uri;
use crate::utils::project::pick_and_open_project;
use dioxus::prelude::*;

#[component]
pub fn CockpitPage() -> Element {
    let state = use_context::<Signal<AppState>>();
    let current_state = state.read();
    let has_project = current_state.active_project.is_some();
    let has_open_tabs = !current_state.open_tabs.is_empty();
    let messages = current_state.active_messages();
    let show_pods = current_state.show_pods;
    let has_pending_patch = current_state.pending_patch.is_some();
    let has_pending_sql = current_state.pending_sql.is_some();
    let msg_count = messages.len();
    let is_reasoning = current_state.is_reasoning;

    use_effect(use_reactive(
        (&msg_count, &is_reasoning),
        move |(count, reasoning)| {
            if count > 0 || reasoning {
                let _ = document::eval(
                    r#"
                    setTimeout(() => {
                        const el = document.getElementById('chat-bottom-anchor');
                        if (el) {
                            el.scrollIntoView({ behavior: 'smooth' });
                        }
                    }, 60);
                    "#,
                );
            }
        },
    ));

    rsx! {
        div { class: "flex-1 flex flex-col bg-[#0a0a0a] overflow-hidden select-none px-4 md:px-8",
            if has_open_tabs {
                // When files are open in tabs, hide the chat column entirely!
                FileEditor {}
            } else {
                // Central Conversation & Execution Column
                div { class: "max-w-5xl mx-auto w-full flex-1 flex flex-col min-h-0 pt-4",
                if !has_project {
                    // Empty State: Prompt User to Point to a Project Folder
                    div { class: "flex-1 flex flex-col items-center justify-center text-center p-8 my-auto select-none",
                        div { class: "w-16 h-16 rounded-2xl bg-[#141414] border border-[#262626] flex items-center justify-center mb-4 p-2.5 shadow-md",
                            img {
                                src: icon_logo_data_uri(),
                                class: "w-full h-full object-contain drop-shadow-md",
                                alt: "Uneven IDE",
                            }
                        }
                        h2 { class: "text-base font-medium text-neutral-100 mb-1 tracking-tight font-sans",
                            "Uneven IDE"
                        }
                        p { class: "text-xs text-neutral-400 max-w-sm mb-6 leading-relaxed font-sans",
                            "No project connected. Point to an existing codebase or create a new directory on your machine to begin."
                        }
                        button {
                            class: "px-5 py-2.5 rounded-xl bg-neutral-100 hover:bg-white text-black font-semibold text-xs font-mono transition-all flex items-center gap-2 cursor-pointer shadow-lg active:scale-95",
                            onclick: move |_| {
                                pick_and_open_project(state);
                            },
                            span { "+" }
                            span { "Open Project Folder" }
                        }
                    }
                } else {
                    // Active Workspace: Messages, Pods, and Prompt Bar
                    // Scrollable Conversation Area (Hardware-accelerated scrolling with isolated boundary)
                    div { class: "flex-1 overflow-y-auto pr-1 flex flex-col min-h-0 mb-3 scrollbar-none [overscroll-behavior-y:contain] [will-change:scroll-position]",
                        if messages.is_empty() {
                            if let Some(pod) = current_state.active_pod() {
                                div { class: "flex-1 flex flex-col items-center justify-center text-center p-8 my-auto select-none opacity-60",
                                    div { class: "w-10 h-10 rounded-full border border-emerald-500/40 bg-emerald-950/20 flex items-center justify-center mb-3 text-emerald-400 font-mono text-base",
                                        "⚡"
                                    }
                                    h2 { class: "text-sm font-medium text-neutral-200 mb-1 tracking-tight font-sans",
                                        "{pod.title}"
                                    }
                                    p { class: "text-xs text-neutral-400 max-w-sm font-mono mb-1",
                                        "Prime Specialist: {pod.prime_name} ({pod.prime_role})"
                                    }
                                    p { class: "text-[11px] text-neutral-500 max-w-md italic font-sans",
                                        "\"{pod.instructions}\""
                                    }
                                }
                            } else {
                                div { class: "flex-1 flex flex-col items-center justify-center text-center p-8 my-auto select-none opacity-50",
                                    div { class: "w-12 h-12 rounded-xl border border-neutral-800 bg-[#121212] flex items-center justify-center mb-3 p-2",
                                        img {
                                            src: icon_logo_data_uri(),
                                            class: "w-full h-full object-contain",
                                            alt: "Uneven Studio",
                                        }
                                    }
                                    h2 { class: "text-sm font-medium text-neutral-300 mb-1 tracking-tight font-sans",
                                        "Uneven Studio"
                                    }
                                    p { class: "text-xs text-neutral-500 max-w-xs font-mono",
                                        "Autonomous engineering agents ready."
                                    }
                                }
                            }
                        } else {
                            for msg in messages {
                                MessageBubble { key: "{msg.id}", message: msg }
                            }
                        }
                        div { id: "chat-bottom-anchor" }
                    }

                    // Pending Patch Inspector (Surfaces when user opens the patch viewer)
                    if has_pending_patch && current_state.show_patch_viewer {
                        div { class: "mb-3 max-h-80 overflow-y-auto",
                            PatchViewer {}
                        }
                    }

                    // Pods Flow Row (Visible only when 'Pods' toggle in header is active AND no pod is currently active)
                    if show_pods && current_state.active_pod_id.is_none() {
                        div { class: "mb-2",
                            PodFlow {}
                        }
                    }

                    // Pending SQL Query Inspector (Surfaces when AI proposes queries for verification)
                    if has_pending_sql && current_state.show_sql_inspector {
                        div { class: "mb-2",
                            SqlInspector {}
                        }
                    }

                    // Centralized Prompt Cockpit Bar with aligned ThinkStepper right above
                    div { class: "w-full max-w-4xl mx-auto flex flex-col gap-1.5",
                        ThinkStepper {}
                        PromptInput {}
                    }
                }
            }
            }
        }
    }
}


