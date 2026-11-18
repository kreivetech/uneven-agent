use dioxus::prelude::*;

mod components;
mod pages;
mod routes;
mod state;
mod utils;

use routes::Route;
use state::AppState;

const MAIN_CSS: &str = include_str!("../assets/main.css");
const TAILWIND_CSS: &str = include_str!("../assets/tailwind.css");
const BASE_STYLE: &str = "html, body { background-color: #0a0a0a !important; color: #ededed !important; margin: 0 !important; padding: 0 !important; overflow: hidden !important; }";

fn is_home_directory(path: &std::path::Path) -> bool {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Ok(home) = std::env::var("HOME")
        && let Ok(home_path) = std::path::Path::new(&home).canonicalize()
        && canonical == home_path
    {
        return true;
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE")
        && let Ok(user_path) = std::path::Path::new(&userprofile).canonicalize()
        && canonical == user_path
    {
        return true;
    }
    false
}

fn resolve_initial_project_path() -> Option<std::path::PathBuf> {
    let arg = std::env::args().nth(1)?;
    let trimmed = arg.trim();
    if trimmed.is_empty() || trimmed == "~" {
        return None;
    }
    let p = std::path::PathBuf::from(trimmed);
    let resolved = p.canonicalize().unwrap_or(p);
    if resolved.is_dir() && !is_home_directory(&resolved) {
        Some(resolved)
    } else {
        None
    }
}

fn main() {
    // 1. Initialize process environment from global and project .env locations
    crate::utils::env_manager::init_process_env();

    use crate::utils::window::desktop_window::{
        create_fullscreen_config, get_monitor_dimensions,
    };

    let dims = get_monitor_dimensions();
    println!(
        "🚀 Launching Uneven IDE (Target Screen: {}x{}) in maximized window...",
        dims.width, dims.height
    );

    let initial_project = resolve_initial_project_path();
    if let Some(ref p) = initial_project {
        println!("📂 Opening initial project workspace: {}", p.display());
    }

    let cfg = create_fullscreen_config("Uneven IDE").with_menu(None);
    dioxus::LaunchBuilder::desktop()
        .with_cfg(cfg)
        .launch(App);
}

#[component]
fn App() -> Element {
    // Provide global reactive AppState context to all components
    let mut app_state = use_context_provider(|| {
        let mut state = AppState::default();
        if let Some(path) = resolve_initial_project_path() {
            state.open_project(path);
        }
        Signal::new(state)
    });

    use_effect(move || {
        spawn(async move {
            let current = env!("CARGO_PKG_VERSION").to_string();
            let res = tokio::spawn(async move {
                uneven_core::application::UpdateService::check_ide_update(&current).await
            })
            .await;

            if let Ok(Ok(Some(info))) = res {
                app_state.write().available_ide_update = Some(info.latest_version);
            }
        });
    });

    rsx! {
        document::Link { rel: "icon", href: crate::utils::icon_logo_data_uri() }
        document::Style { "{BASE_STYLE}" }
        document::Style { "{MAIN_CSS}" }
        document::Style { "{TAILWIND_CSS}" }
        Router::<Route> {}
    }
}
