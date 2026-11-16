use crate::state::AppState;
use crate::utils::formatters::format_duration;
use dioxus::prelude::*;
use uneven_core::application::TokenQuotaService;

#[component]
pub fn StatusBar() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current_state = state.read();

    let telemetry = &current_state.telemetry;
    let quota = TokenQuotaService::get_quota_metrics();
    let provider = if current_state.active_provider == "default" {
        "Auto (Smart Routing)"
    } else {
        &current_state.active_provider
    };
    let show_terminal = current_state.show_terminal;
    let is_terminal_running = current_state.terminal.is_running;

    let phase_str = match current_state.thought_phase {
        Some(uneven_core::application::think_service::ThoughtPhase::ResolvingContext) => {
            "Analyzing Context…"
        }
        Some(uneven_core::application::think_service::ThoughtPhase::Drafting) => {
            "Drafting Solution [1/4]…"
        }
        Some(uneven_core::application::think_service::ThoughtPhase::RubberDuckingDraft) => {
            "Rubber Duck (Draft)…"
        }
        Some(uneven_core::application::think_service::ThoughtPhase::Auditing) => {
            "Auditing Architecture [2/4]…"
        }
        Some(uneven_core::application::think_service::ThoughtPhase::VerifyingIntegrity) => {
            "Verifying Integrity [3/4]…"
        }
        Some(uneven_core::application::think_service::ThoughtPhase::RubberDuckingSimulation) => {
            "Rubber Duck (Simulation)…"
        }
        Some(uneven_core::application::think_service::ThoughtPhase::Synthesizing) => {
            "Synthesizing [4/4]…"
        }
        Some(uneven_core::application::think_service::ThoughtPhase::Completed) => "Done",
        None => "Idle",
    };

    rsx! {
        footer { class: "h-7 bg-neutral-900 border-t border-neutral-800 px-3 flex items-center justify-between text-[11px] text-neutral-400 font-mono select-none z-10",
            // Left: Engine Phase Status
            div { class: "flex items-center gap-3",
                div { class: "flex items-center gap-1.5",
                    span { class: "w-2 h-2 rounded-full",
                        class: if current_state.thought_phase.is_some() {
                            "bg-cyan-400 animate-ping"
                        } else {
                            "bg-emerald-500"
                        }
                    }
                    span { class: "text-neutral-300 font-medium", "{phase_str}" }
                }
                span { class: "text-neutral-700", "|" }
                div { class: "flex items-center gap-1",
                    span { class: "text-neutral-500", "Brain:" }
                    span { class: "text-neutral-300", "{provider}" }
                }
                span { class: "text-neutral-700", "|" }
                button {
                    class: if show_terminal {
                        "flex items-center gap-1.5 px-2 py-0.5 rounded bg-[#1e1e1e] border border-[#333333] text-emerald-400 font-medium text-[10px] cursor-pointer"
                    } else {
                        "flex items-center gap-1.5 px-2 py-0.5 rounded hover:bg-[#1a1a1a] text-neutral-400 hover:text-neutral-200 text-[10px] cursor-pointer transition-colors"
                    },
                    title: "Toggle Integrated Terminal",
                    onclick: move |_| {
                        let cur = state.read().show_terminal;
                        state.write().show_terminal = !cur;
                    },
                    span { ">_" }
                    span { "Terminal" }
                    if is_terminal_running {
                        span { class: "w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse ml-0.5" }
                    }
                }
            }

            // Right: Telemetry & Quota metrics
            div { class: "flex items-center gap-4 text-[10px]",
                if quota.monthly_tokens > 0 {
                    div { class: "flex items-center gap-1",
                        span { class: "text-neutral-500", "Quota:" }
                        span { class: "text-emerald-400 font-medium", "{quota.monthly_percent:.1}%" }
                    }
                    div { class: "flex items-center gap-1",
                        span { class: "text-neutral-500", "Turn (5h):" }
                        span { class: "text-neutral-300", "{quota.turn_5h_percent:.1}%" }
                    }
                }
                if quota.optimization_savings_percent > 0.0 {
                    div { class: "flex items-center gap-1 text-cyan-400",
                        span { class: "text-neutral-500", "Saved:" }
                        span { "+{quota.optimization_savings_percent:.1}%" }
                    }
                }
                if telemetry.elapsed.as_millis() > 0 {
                    div { class: "flex items-center gap-1",
                        span { class: "text-neutral-500", "Time:" }
                        span { class: "text-neutral-300", "{format_duration(telemetry.elapsed)}" }
                    }
                }
                span { class: "text-neutral-600", "•" }
                span { class: "text-neutral-500", "Uneven IDE 0.2.2" }
            }
        }
    }
}
