use super::navbar::Navbar;
use super::sidebar::Sidebar;
use super::status_bar::StatusBar;
use super::terminal::TerminalPanel;
use crate::routes::Route;
use crate::state::AppState;
use dioxus::prelude::*;

#[component]
pub fn AppLayout() -> Element {
    let state = use_context::<Signal<AppState>>();
    let current_state = state.read();
    let show_terminal = current_state.show_terminal;
    let is_terminal_expanded = show_terminal && current_state.terminal.is_expanded;

    rsx! {
        div { class: "h-screen w-screen flex flex-col bg-neutral-950 text-neutral-100 overflow-hidden font-sans",
            Navbar {}
            div { class: "flex-1 flex overflow-hidden",
                Sidebar {}
                main { class: "flex-1 flex flex-col overflow-hidden min-w-0",
                    div {
                        class: if is_terminal_expanded {
                            "hidden"
                        } else {
                            "flex-1 flex overflow-hidden min-h-0"
                        },
                        Outlet::<Route> {}
                    }
                    if show_terminal {
                        TerminalPanel {}
                    }
                }
            }
            StatusBar {}
        }
    }
}
