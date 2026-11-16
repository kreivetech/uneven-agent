use crate::state::AppState;
use dioxus::prelude::*;
use uneven_core::application::think_service::ThoughtPhase;

#[component]
pub fn ThinkStepper() -> Element {
    let state = use_context::<Signal<AppState>>();
    let current_state = state.read();

    // Only render when thinking/reasoning is actively in progress
    if !current_state.is_reasoning {
        return rsx! {};
    }

    let phase = match current_state.thought_phase {
        Some(ThoughtPhase::Completed) | None => return rsx! {},
        Some(p) => p,
    };

    let step_text = match phase {
        ThoughtPhase::ResolvingContext => "Analyzing context…",
        ThoughtPhase::Drafting => "Drafting solution [1/4]…",
        ThoughtPhase::RubberDuckingDraft => "🦆 Rubber-ducking: Unpacking assumptions…",
        ThoughtPhase::Auditing => "Auditing architecture [2/4]…",
        ThoughtPhase::VerifyingIntegrity => "Verifying integrity [3/4]…",
        ThoughtPhase::RubberDuckingSimulation => "🦆 Rubber-ducking: Simulating execution…",
        ThoughtPhase::Synthesizing => "Synthesizing proposal [4/4]…",
        ThoughtPhase::Completed => return rsx! {},
    };

    let display_text = if let Some(ref sub) = current_state.active_subagent_info {
        sub.clone()
    } else {
        step_text.to_string()
    };

    rsx! {
        div { class: "flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-mono font-medium bg-[#1a1a1a] border border-[#2e2e2e] text-neutral-200 shadow-sm transition-all animate-pulse shrink-0",
            // Emerald spinner
            svg {
                class: "animate-spin w-3 h-3 text-emerald-400 shrink-0",
                xmlns: "http://www.w3.org/2000/svg",
                fill: "none",
                view_box: "0 0 24 24",
                circle {
                    class: "opacity-25",
                    cx: "12",
                    cy: "12",
                    r: "10",
                    stroke: "currentColor",
                    stroke_width: "4",
                }
                path {
                    class: "opacity-75",
                    fill: "currentColor",
                    d: "M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z",
                }
            }
            span { class: "text-neutral-300 font-sans text-xs tracking-tight", "{display_text}" }
        }
    }
}
