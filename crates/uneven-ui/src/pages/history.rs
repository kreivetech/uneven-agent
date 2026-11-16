use crate::routes::Route;
use crate::state::{AppState, MessageRole};
use dioxus::prelude::*;

#[component]
pub fn HistoryPage() -> Element {
    let state = use_context::<Signal<AppState>>();
    let current_state = state.read();
    let messages = current_state.messages.clone();

    rsx! {
        div { class: "flex-1 flex flex-col bg-neutral-950 p-6 overflow-y-auto select-none",
            // Header
            div { class: "border-b border-neutral-800 pb-4 mb-6 flex items-center justify-between",
                div {
                    h1 { class: "text-xl font-bold tracking-tight text-neutral-100 flex items-center gap-2",
                        span { class: "text-amber-400", "⏱" }
                        "Thought & Audit History"
                    }
                    p { class: "text-xs text-neutral-400 mt-1",
                        "Review current session reasoning runs, critic findings, and disk mutation audits."
                    }
                }

                Link {
                    to: Route::CockpitPage {},
                    class: "px-3 py-1.5 text-xs font-mono font-medium rounded-lg bg-neutral-900 hover:bg-neutral-800 text-cyan-300 border border-neutral-800 hover:border-neutral-700 transition-all flex items-center gap-1.5",
                    span { "←" }
                    "Back to Cockpit"
                }
            }

            // Body
            if messages.is_empty() {
                div { class: "flex-1 flex flex-col items-center justify-center text-center p-8 text-neutral-500 my-auto",
                    div { class: "w-12 h-12 rounded-xl bg-neutral-900 border border-neutral-800 flex items-center justify-center mb-3 text-neutral-600 text-lg",
                        "⏱"
                    }
                    h2 { class: "text-sm font-semibold text-neutral-300 mb-1", "No History Recorded Yet" }
                    p { class: "text-xs text-neutral-500 max-w-sm",
                        "Start a reasoning or refactoring task in the Cockpit. Multi-turn reasoning loops and critic audits will be logged here."
                    }
                }
            } else {
                div { class: "max-w-3xl flex flex-col gap-4",
                    div { class: "text-xs text-neutral-400 font-mono flex items-center justify-between pb-2 border-b border-neutral-900",
                        span { "Current Session Events ({messages.len()})" }
                    }

                    for (idx, msg) in messages.iter().enumerate() {
                        div {
                            key: "{msg.id}_{idx}",
                            class: "p-4 bg-neutral-900 border border-neutral-800 rounded-xl flex flex-col gap-2 font-mono text-xs",
                            div { class: "flex items-center justify-between",
                                div { class: "flex items-center gap-2",
                                    span {
                                        class: match msg.role {
                                            MessageRole::User => "px-2 py-0.5 rounded text-[10px] font-bold bg-cyan-950 border border-cyan-800 text-cyan-300 uppercase",
                                            MessageRole::Assistant => "px-2 py-0.5 rounded text-[10px] font-bold bg-neutral-800 border border-neutral-700 text-neutral-300 uppercase",
                                        },
                                        match msg.role {
                                            MessageRole::User => "User Query",
                                            MessageRole::Assistant => "Engine Synthesis",
                                        }
                                    }
                                    span { class: "text-[10px] text-neutral-500", "Turn #{idx + 1}" }
                                }

                                if !msg.files.is_empty() {
                                    span { class: "text-[10px] text-neutral-400 flex items-center gap-1",
                                        span { "📎" }
                                        "{msg.files.len()} context files"
                                    }
                                }
                            }

                            div { class: "font-sans text-xs text-neutral-300 bg-neutral-950/60 p-3 rounded-lg border border-neutral-800/80 max-h-32 overflow-y-auto whitespace-pre-wrap",
                                "{msg.text}"
                            }
                        }
                    }
                }
            }
        }
    }
}
