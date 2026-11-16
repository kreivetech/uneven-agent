use crate::state::AppState;
use dioxus::prelude::*;
use uneven_core::domain::pod::{AgentPod, PodStatus};

#[component]
pub fn PodFlow() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current_state = state.read();
    let pods = current_state.pods.clone();
    let active_pod_id = current_state.active_pod_id.clone();

    rsx! {
        div { class: "w-full flex flex-col gap-2 py-2 select-none",
            // Horizontal Autonomous Pod Cards Row (Read-only view for user; managed autonomously by AI)
            div { class: "flex items-center gap-3 overflow-x-auto pb-1 scrollbar-none",
                for pod in pods {
                    {
                        let is_active = active_pod_id.as_ref() == Some(&pod.id);
                        rsx! {
                            PodCard {
                                key: "{pod.id}",
                                pod: pod,
                                is_active: is_active,
                                on_select: move |id: String| {
                                    if state.read().active_pod_id.as_ref() == Some(&id) {
                                        state.write().select_pod(None);
                                    } else {
                                        state.write().select_pod(Some(id));
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

#[component]
fn PodCard(
    pod: AgentPod,
    is_active: bool,
    on_select: EventHandler<String>,
) -> Element {
    let card_border = if is_active {
        "border-neutral-400 ring-1 ring-neutral-400/40 bg-[#1c1c1c]"
    } else {
        "border-[#262626] hover:border-[#383838] bg-[#141414] hover:bg-[#181818]"
    };

    let status_dot = match pod.status {
        PodStatus::Completed => "bg-emerald-400 shadow-sm shadow-emerald-500/60",
        PodStatus::Running => "bg-amber-400 animate-pulse shadow-sm shadow-amber-500/60",
        PodStatus::Failed => "bg-rose-500 shadow-sm shadow-rose-500/60",
        PodStatus::Pending => "bg-neutral-600",
    };

    let status_text = match pod.status {
        PodStatus::Completed => "completed",
        PodStatus::Running => "running",
        PodStatus::Failed => "failed",
        PodStatus::Pending => "pending",
    };

    let status_color = match pod.status {
        PodStatus::Completed => "text-emerald-400",
        PodStatus::Running => "text-amber-400",
        PodStatus::Failed => "text-rose-400",
        PodStatus::Pending => "text-neutral-500",
    };

    let pod_id = pod.id.clone();

    rsx! {
        div {
            class: "w-52 h-20 p-3 rounded-xl border {card_border} transition-all duration-150 flex flex-col justify-between shrink-0 cursor-pointer shadow-sm",
            onclick: move |_| on_select.call(pod_id.clone()),

            // Top Row: Title on Left, Status Dot on Right
            div { class: "flex items-center justify-between gap-2",
                span { class: "text-xs font-semibold text-neutral-100 truncate tracking-tight font-sans",
                    "{pod.title}"
                }
                span { class: "w-2.5 h-2.5 rounded-full shrink-0 {status_dot}" }
            }

            // Bottom Row: Task Count on Left, Status Text on Right
            div { class: "flex items-center justify-between text-[11px] font-mono",
                span { class: "text-neutral-400",
                    "{pod.completed_tasks:02}/{pod.total_tasks:02} tasks"
                }
                span { class: "text-[10px] font-medium {status_color}",
                    "{status_text}"
                }
            }
        }
    }
}
