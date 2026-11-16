use crate::routes::Route;
use crate::state::AppState;
use crate::utils::env_manager::{
    load_all_env_keys, save_all_env_keys, DATABASE_KEY_VARS, PROVIDER_KEY_VARS,
};
use crate::utils::icon_logo_data_uri;
use catfive::{string, Validator};
use dioxus::prelude::*;
use std::collections::HashMap;
use uneven_core::application::TokenQuotaService;

#[component]
pub fn SettingsPage() -> Element {
    let state = use_context::<Signal<AppState>>();
    let current_state = state.read();
    let root = current_state
        .project_root
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // Real-time quota metrics
    let quota = TokenQuotaService::get_quota_metrics();

    // Provider & DB credentials state
    let mut env_keys = use_signal(|| load_all_env_keys(&root));
    let mut visible_keys = use_signal(HashMap::<String, bool>::new);
    let mut expanded_provider = use_signal(|| None::<String>);
    let mut expanded_db = use_signal(|| None::<String>);
    let mut save_status = use_signal(|| None::<String>);

    rsx! {
        div { class: "flex-1 flex flex-col bg-[#0a0a0a] p-6 overflow-y-auto select-none space-y-6 max-w-5xl mx-auto w-full",
            // Header Bar
            div { class: "border-b border-[#1c1c1c] pb-4 flex items-center justify-between flex-wrap gap-4",
                div {
                    h1 { class: "text-lg font-bold tracking-tight text-neutral-100 flex items-center gap-2.5",
                        img {
                            src: icon_logo_data_uri(),
                            class: "w-5 h-5 rounded-md object-contain shrink-0",
                            alt: "Uneven Settings",
                        }
                        "Settings & Telemetry"
                    }
                    p { class: "text-xs text-neutral-400 mt-0.5",
                        "Manage AI provider credentials, database connectivity, and monitor real-time token quota limits."
                    }
                }

                div { class: "flex items-center gap-3",
                    button {
                        class: "px-3 py-1.5 text-xs font-mono font-medium rounded-lg bg-[#141414] hover:bg-[#1f1f1f] text-neutral-300 border border-[#262626] hover:border-neutral-700 transition-all cursor-pointer flex items-center gap-1.5 shadow-sm active:scale-95",
                        onclick: move |_| {
                            let cur_root = state.read().project_root.clone().unwrap_or_else(|| std::path::PathBuf::from("."));
                            env_keys.set(load_all_env_keys(&cur_root));
                            save_status.set(Some("Credentials reloaded successfully.".to_string()));
                        },
                        span { "↻" }
                        "Reload"
                    }

                    Link {
                        to: Route::CockpitPage {},
                        class: "w-8 h-8 rounded-lg bg-[#161616] hover:bg-[#242424] border border-[#2a2a2a] hover:border-neutral-600 text-neutral-400 hover:text-white flex items-center justify-center text-sm font-semibold transition-all cursor-pointer shadow-sm active:scale-95",
                        title: "Back to Cockpit",
                        "✕"
                    }
                }
            }

            // Save Confirmation Banner
            if let Some(ref status) = *save_status.read() {
                div { class: "p-3 rounded-lg bg-emerald-950/60 border border-emerald-800 text-emerald-300 text-xs font-mono flex items-center justify-between shadow-lg",
                    span { "✓ {status}" }
                    button {
                        class: "text-neutral-400 hover:text-white font-bold ml-2 cursor-pointer",
                        onclick: move |_| save_status.set(None),
                        "✕"
                    }
                }
            }

            // ========================================================
            // 1. TOKEN USAGE & PLAN QUOTAS DASHBOARD
            // ========================================================
            div { class: "bg-[#111111] border border-[#202020] rounded-xl p-5 flex flex-col gap-4 shadow-xl",
                div { class: "flex items-center justify-between border-b border-[#1c1c1c] pb-3 flex-wrap gap-2",
                    div {
                        h2 { class: "text-sm font-semibold text-neutral-100 flex items-center gap-2",
                            "Token Quota & Consumption"
                        }
                        p { class: "text-xs text-neutral-500 mt-0.5",
                            "Real-time sliding window quota tracking and compaction optimization"
                        }
                    }
                }

                // 4-Card Usage Grid
                div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3.5 pt-1",
                    // Card 1: 5-Hour Work Turn
                    div { class: "p-3.5 bg-[#141414] border border-[#242424] rounded-xl flex flex-col justify-between gap-3",
                        div { class: "flex items-center justify-between text-xs",
                            span { class: "text-neutral-400 font-medium", "Work Turn (5h)" }
                            span {
                                class: if quota.turn_5h_percent >= 90.0 {
                                    "text-rose-400 font-mono font-bold"
                                } else if quota.turn_5h_percent >= 70.0 {
                                    "text-amber-400 font-mono font-bold"
                                } else {
                                    "text-cyan-400 font-mono font-bold"
                                },
                                "{quota.turn_5h_percent:.1}%"
                            }
                        }
                        // Progress bar
                        div { class: "w-full h-2 rounded-full bg-[#202020] overflow-hidden",
                            div {
                                class: if quota.turn_5h_percent >= 90.0 {
                                    "h-full rounded-full bg-rose-500 transition-all duration-500"
                                } else if quota.turn_5h_percent >= 70.0 {
                                    "h-full rounded-full bg-amber-500 transition-all duration-500"
                                } else {
                                    "h-full rounded-full bg-cyan-400 transition-all duration-500"
                                },
                                style: "width: {quota.turn_5h_percent.clamp(0.0, 100.0)}%;",
                            }
                        }
                        div { class: "flex items-center justify-between text-[10px] text-neutral-500 font-mono",
                            span { "Sliding window" }
                            span { "Resets in {quota.turn_remaining_minutes}m" }
                        }
                    }

                    // Card 2: Weekly Window (7 Days)
                    div { class: "p-3.5 bg-[#141414] border border-[#242424] rounded-xl flex flex-col justify-between gap-3",
                        div { class: "flex items-center justify-between text-xs",
                            span { class: "text-neutral-400 font-medium", "Weekly Window" }
                            span {
                                class: if quota.weekly_percent >= 90.0 {
                                    "text-rose-400 font-mono font-bold"
                                } else {
                                    "text-emerald-400 font-mono font-bold"
                                },
                                "{quota.weekly_percent:.1}%"
                            }
                        }
                        div { class: "w-full h-2 rounded-full bg-[#202020] overflow-hidden",
                            div {
                                class: "h-full rounded-full bg-emerald-400 transition-all duration-500",
                                style: "width: {quota.weekly_percent.clamp(0.0, 100.0)}%;",
                            }
                        }
                        div { class: "flex items-center justify-between text-[10px] text-neutral-500 font-mono",
                            span { "Rolling 7 days" }
                            span {
                                if quota.weekly_percent >= 90.0 {
                                    "High activity"
                                } else {
                                    "Healthy"
                                }
                            }
                        }
                    }

                    // Card 3: Monthly Calendar Quota
                    div { class: "p-3.5 bg-[#141414] border border-[#242424] rounded-xl flex flex-col justify-between gap-3",
                        div { class: "flex items-center justify-between text-xs",
                            span { class: "text-neutral-400 font-medium", "Monthly Quota" }
                            span {
                                class: if quota.monthly_percent >= 90.0 {
                                    "text-rose-400 font-mono font-bold"
                                } else {
                                    "text-cyan-300 font-mono font-bold"
                                },
                                "{quota.monthly_percent:.1}%"
                            }
                        }
                        div { class: "w-full h-2 rounded-full bg-[#202020] overflow-hidden",
                            div {
                                class: "h-full rounded-full bg-cyan-400 transition-all duration-500",
                                style: "width: {quota.monthly_percent.clamp(0.0, 100.0)}%;",
                            }
                        }
                        div { class: "flex items-center justify-between text-[10px] text-neutral-500 font-mono",
                            span { "Billing cycle" }
                            span { "{quota.month_days_remaining}d remaining" }
                        }
                    }

                    // Card 4: Optimization Savings
                    div { class: "p-3.5 bg-[#141414] border border-[#242424] rounded-xl flex flex-col justify-between gap-3",
                        div { class: "flex items-center justify-between text-xs",
                            span { class: "text-neutral-400 font-medium", "Efficiency Boost" }
                            span { class: "text-cyan-400 font-mono font-bold", "+{quota.optimization_savings_percent:.1}%" }
                        }
                        div { class: "w-full h-2 rounded-full bg-[#202020] overflow-hidden",
                            div {
                                class: "h-full rounded-full bg-cyan-500 transition-all duration-500",
                                style: "width: {quota.optimization_savings_percent.clamp(0.0, 100.0)}%;",
                            }
                        }
                        div { class: "flex items-center justify-between text-[10px] text-neutral-500 font-mono",
                            span { "Context Compaction" }
                            span { "Tokens saved" }
                        }
                    }
                }
            }

            // ========================================================
            // 2. AI BRAIN PROVIDERS & API KEYS (Click to configure)
            // ========================================================
            div { class: "bg-[#111111] border border-[#202020] rounded-xl p-5 flex flex-col gap-4 shadow-xl",
                div { class: "flex items-center justify-between border-b border-[#1c1c1c] pb-3 flex-wrap gap-2",
                    div {
                        h2 { class: "text-sm font-semibold text-neutral-100 flex items-center gap-2",
                            "AI Brain Providers"
                        }
                        p { class: "text-xs text-neutral-500 mt-0.5",
                            "Select a provider to view or update credentials. Changes persist securely across sessions."
                        }
                    }

                    button {
                        class: "px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs font-semibold tracking-wide transition-all shadow-md shadow-emerald-950/40 cursor-pointer active:scale-95 flex items-center gap-1.5",
                        onclick: move |_| {
                            let current_map = env_keys.read().clone();
                            let key_validator = string().min(4).block(" \t\r\n");
                            for (var, val) in current_map.iter() {
                                let trimmed = val.trim();
                                if !trimmed.is_empty() && key_validator.validate(trimmed).is_err() {
                                    save_status.set(Some(format!("Invalid value for {}: must be at least 4 characters without spaces.", var)));
                                    return;
                                }
                            }

                            match save_all_env_keys(&current_map) {
                                Ok(_) => save_status.set(Some("Credentials saved and activated successfully!".to_string())),
                                Err(e) => save_status.set(Some(format!("Failed to save credentials: {}", e))),
                            }
                        },
                        "Save All Changes"
                    }
                }

                // Providers List
                div { class: "flex flex-col gap-2.5 pt-1",
                    for (var_name, label, placeholder, desc) in PROVIDER_KEY_VARS {
                        {
                            let v_name = *var_name;
                            let v_label = *label;
                            let v_placeholder = *placeholder;
                            let v_desc = *desc;

                            let current_val = env_keys.read().get(v_name).cloned().unwrap_or_default();
                            let is_configured = !current_val.trim().is_empty();
                            let is_expanded = expanded_provider.read().as_deref() == Some(v_name);
                            let is_visible = *visible_keys.read().get(v_name).unwrap_or(&false);

                            let v_click = v_name.to_string();
                            let v_input = v_name.to_string();
                            let v_toggle_vis = v_name.to_string();
                            let v_clear = v_name.to_string();

                            rsx! {
                                div {
                                    key: "{v_name}",
                                    class: "border rounded-xl transition-all overflow-hidden",
                                    class: if is_expanded {
                                        "bg-[#141414] border-neutral-700 shadow-md"
                                    } else {
                                        "bg-[#121212] border-[#222222] hover:border-[#2e2e2e]"
                                    },

                                    // Provider Summary Card (Click to open)
                                    div {
                                        class: "px-4 py-3 flex items-center justify-between cursor-pointer select-none",
                                        onclick: move |_| {
                                            if expanded_provider.read().as_deref() == Some(&v_click) {
                                                expanded_provider.set(None);
                                            } else {
                                                expanded_provider.set(Some(v_click.clone()));
                                            }
                                        },

                                        div { class: "flex items-center gap-3",
                                            div { class: "w-8 h-8 rounded-lg bg-[#1a1a1a] border border-[#2a2a2a] flex items-center justify-center font-mono text-xs font-bold text-neutral-300",
                                                "{v_label.chars().next().unwrap_or('A')}"
                                            }
                                            div { class: "flex flex-col",
                                                span { class: "font-medium text-sm text-neutral-100", "{v_label}" }
                                                span { class: "text-[11px] text-neutral-500", "{v_desc}" }
                                            }
                                        }

                                        div { class: "flex items-center gap-2.5",
                                            span {
                                                class: if is_configured {
                                                    "px-2.5 py-0.5 rounded text-[10px] font-mono font-bold bg-emerald-950/80 border border-emerald-800/80 text-emerald-300 uppercase tracking-wider"
                                                } else {
                                                    "px-2.5 py-0.5 rounded text-[10px] font-mono bg-[#1a1a1a] border border-[#2a2a2a] text-neutral-500 uppercase tracking-wider"
                                                },
                                                if is_configured { "✓ Configured" } else { "Not Set" }
                                            }
                                            span { class: "text-neutral-500 text-xs font-mono transition-transform",
                                                if is_expanded { "▴" } else { "▾" }
                                            }
                                        }
                                    }

                                    // Expanded Input Drawer
                                    if is_expanded {
                                        div { class: "px-4 pb-4 pt-2 border-t border-[#1f1f1f] flex flex-col gap-2.5 bg-[#0e0e0e]",
                                            div { class: "flex items-center gap-2",
                                                input {
                                                    class: "flex-1 bg-[#141414] border border-[#262626] rounded-lg px-3 py-2 text-xs text-neutral-100 placeholder-neutral-600 focus:outline-none focus:border-cyan-500 font-mono transition-colors",
                                                    r#type: if is_visible { "text" } else { "password" },
                                                    placeholder: "{v_placeholder}",
                                                    value: "{current_val}",
                                                    oninput: move |evt| {
                                                        env_keys.write().insert(v_input.clone(), evt.value());
                                                    },
                                                }
                                                button {
                                                    class: "px-2.5 py-2 rounded-lg bg-[#181818] hover:bg-[#242424] text-neutral-400 hover:text-neutral-200 text-xs font-mono transition-colors border border-[#2a2a2a] cursor-pointer",
                                                    title: if is_visible { "Hide key" } else { "Show key" },
                                                    onclick: move |_| {
                                                        let current = *visible_keys.read().get(&v_toggle_vis).unwrap_or(&false);
                                                        visible_keys.write().insert(v_toggle_vis.clone(), !current);
                                                    },
                                                    if is_visible { "Hide" } else { "Show" }
                                                }
                                                if is_configured {
                                                    button {
                                                        class: "px-2.5 py-2 rounded-lg bg-rose-950/30 hover:bg-rose-950/60 text-rose-400 border border-rose-800/40 text-xs font-mono transition-colors cursor-pointer",
                                                        title: "Clear this key",
                                                        onclick: move |_| {
                                                            env_keys.write().insert(v_clear.clone(), String::new());
                                                            let current_map = env_keys.read().clone();
                                                            let _ = save_all_env_keys(&current_map);
                                                            save_status.set(Some(format!("Key for {} cleared.", v_label)));
                                                        },
                                                        "Clear"
                                                    }
                                                }
                                            }
                                            div { class: "flex items-center justify-between text-[10px] text-neutral-500 font-mono",
                                                span { "Secure local persistence" }
                                                if is_configured {
                                                    span { class: "text-emerald-400", "● Active in current process" }
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

            // ========================================================
            // 3. DATABASE & SECURITY ANALYSIS CREDENTIALS (analyze)
            // ========================================================
            div { class: "bg-[#111111] border border-[#202020] rounded-xl p-5 flex flex-col gap-4 shadow-xl",
                div { class: "flex items-center justify-between border-b border-[#1c1c1c] pb-3 flex-wrap gap-2",
                    div {
                        h2 { class: "text-sm font-semibold text-neutral-100 flex items-center gap-2",
                            "Database & Security Analysis"
                        }
                        p { class: "text-xs text-neutral-500 mt-0.5",
                            "Connection string and credentials used by Uneven for automated SQL schema analysis and security audits."
                        }
                    }
                }

                // Database Keys List
                div { class: "flex flex-col gap-2.5 pt-1",
                    for (var_name, label, placeholder, desc) in DATABASE_KEY_VARS {
                        {
                            let v_name = *var_name;
                            let v_label = *label;
                            let v_placeholder = *placeholder;
                            let v_desc = *desc;

                            let current_val = env_keys.read().get(v_name).cloned().unwrap_or_default();
                            let is_configured = !current_val.trim().is_empty();
                            let is_expanded = expanded_db.read().as_deref() == Some(v_name);
                            let is_visible = *visible_keys.read().get(v_name).unwrap_or(&false);

                            let v_click = v_name.to_string();
                            let v_input = v_name.to_string();
                            let v_toggle_vis = v_name.to_string();
                            let v_clear = v_name.to_string();

                            rsx! {
                                div {
                                    key: "{v_name}",
                                    class: "border rounded-xl transition-all overflow-hidden",
                                    class: if is_expanded {
                                        "bg-[#141414] border-neutral-700 shadow-md"
                                    } else {
                                        "bg-[#121212] border-[#222222] hover:border-[#2e2e2e]"
                                    },

                                    // Card Summary Header
                                    div {
                                        class: "px-4 py-3 flex items-center justify-between cursor-pointer select-none",
                                        onclick: move |_| {
                                            if expanded_db.read().as_deref() == Some(&v_click) {
                                                expanded_db.set(None);
                                            } else {
                                                expanded_db.set(Some(v_click.clone()));
                                            }
                                        },

                                        div { class: "flex items-center gap-3",
                                            div { class: "w-8 h-8 rounded-lg bg-[#1a1a1a] border border-[#2a2a2a] flex items-center justify-center font-mono text-xs font-bold text-emerald-400",
                                                "DB"
                                            }
                                            div { class: "flex flex-col",
                                                span { class: "font-medium text-sm text-neutral-100", "{v_label}" }
                                                span { class: "text-[11px] text-neutral-500", "{v_desc}" }
                                            }
                                        }

                                        div { class: "flex items-center gap-2.5",
                                            span {
                                                class: if is_configured {
                                                    "px-2.5 py-0.5 rounded text-[10px] font-mono font-bold bg-emerald-950/80 border border-emerald-800/80 text-emerald-300 uppercase tracking-wider"
                                                } else {
                                                    "px-2.5 py-0.5 rounded text-[10px] font-mono bg-[#1a1a1a] border border-[#2a2a2a] text-neutral-500 uppercase tracking-wider"
                                                },
                                                if is_configured { "✓ Configured" } else { "Not Set" }
                                            }
                                            span { class: "text-neutral-500 text-xs font-mono transition-transform",
                                                if is_expanded { "▴" } else { "▾" }
                                            }
                                        }
                                    }

                                    // Expanded DB Input Drawer
                                    if is_expanded {
                                        div { class: "px-4 pb-4 pt-2 border-t border-[#1f1f1f] flex flex-col gap-2.5 bg-[#0e0e0e]",
                                            div { class: "flex items-center gap-2",
                                                input {
                                                    class: "flex-1 bg-[#141414] border border-[#262626] rounded-lg px-3 py-2 text-xs text-neutral-100 placeholder-neutral-600 focus:outline-none focus:border-emerald-500 font-mono transition-colors",
                                                    r#type: if is_visible { "text" } else { "password" },
                                                    placeholder: "{v_placeholder}",
                                                    value: "{current_val}",
                                                    oninput: move |evt| {
                                                        env_keys.write().insert(v_input.clone(), evt.value());
                                                    },
                                                }
                                                button {
                                                    class: "px-2.5 py-2 rounded-lg bg-[#181818] hover:bg-[#242424] text-neutral-400 hover:text-neutral-200 text-xs font-mono transition-colors border border-[#2a2a2a] cursor-pointer",
                                                    title: if is_visible { "Hide value" } else { "Show value" },
                                                    onclick: move |_| {
                                                        let current = *visible_keys.read().get(&v_toggle_vis).unwrap_or(&false);
                                                        visible_keys.write().insert(v_toggle_vis.clone(), !current);
                                                    },
                                                    if is_visible { "Hide" } else { "Show" }
                                                }
                                                if is_configured {
                                                    button {
                                                        class: "px-2.5 py-2 rounded-lg bg-rose-950/30 hover:bg-rose-950/60 text-rose-400 border border-rose-800/40 text-xs font-mono transition-colors cursor-pointer",
                                                        title: "Clear this key",
                                                        onclick: move |_| {
                                                            env_keys.write().insert(v_clear.clone(), String::new());
                                                            let current_map = env_keys.read().clone();
                                                            let _ = save_all_env_keys(&current_map);
                                                            save_status.set(Some(format!("Database parameter {} cleared.", v_label)));
                                                        },
                                                        "Clear"
                                                    }
                                                }
                                            }
                                            div { class: "flex items-center justify-between text-[10px] text-neutral-500 font-mono",
                                                span { "Secure local persistence" }
                                                if is_configured {
                                                    span { class: "text-emerald-400", "● Ready for 'analyze' audits" }
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
}

