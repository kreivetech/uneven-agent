use crate::state::{AppState, TerminalLineKind};
use crate::utils::project::pick_and_open_project;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};

static KILL_TRIGGER: std::sync::OnceLock<tokio::sync::broadcast::Sender<()>> =
    std::sync::OnceLock::new();

fn get_kill_sender() -> tokio::sync::broadcast::Sender<()> {
    KILL_TRIGGER
        .get_or_init(|| {
            let (tx, _rx) = tokio::sync::broadcast::channel(16);
            tx
        })
        .clone()
}

/// Copies currently selected text from terminal to the system clipboard
fn copy_selection_to_clipboard() {
    let _ = document::eval(
        r#"
        const sel = window.getSelection()?.toString();
        if (sel) {
            navigator.clipboard.writeText(sel).catch(() => {});
        }
        "#,
    );
}

/// Pastes text from system clipboard directly into the terminal command input
fn paste_clipboard_to_input() {
    let _ = document::eval(
        r#"
        navigator.clipboard.readText().then(text => {
            if (text) {
                const inp = document.getElementById('terminal-command-input');
                if (inp) {
                    const start = inp.selectionStart || 0;
                    const end = inp.selectionEnd || 0;
                    const val = inp.value;
                    inp.value = val.slice(0, start) + text + val.slice(end);
                    inp.dispatchEvent(new Event('input', { bubbles: true }));
                    inp.focus();
                }
            }
        }).catch(() => {});
        "#,
    );
}

/// Simple utility to strip ANSI escape codes so terminal output remains clean
fn clean_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_escape = false;
    for ch in input.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[component]
pub fn TerminalPanel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut input_val = use_signal(String::new);
    let history_index = use_signal(|| Option::<usize>::None);

    let current_state = state.read();
    let has_project = current_state.active_project.is_some();
    let is_running = current_state.terminal.is_running;
    let lines_count = current_state.terminal.lines.len();

    let active_dir = current_state
        .terminal
        .current_dir
        .clone()
        .or_else(|| current_state.project_root.clone())
        .unwrap_or_else(|| PathBuf::from("."));

    let dir_display = if let Some(ref root) = current_state.project_root {
        if active_dir == *root {
            current_state
                .active_project
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "project".to_string())
        } else if let Ok(rel) = active_dir.strip_prefix(root) {
            format!("{}/{}", current_state.active_project.as_ref().map(|p| p.name.as_str()).unwrap_or("project"), rel.display())
        } else {
            active_dir.display().to_string()
        }
    } else {
        active_dir.display().to_string()
    };

    // Auto-scroll to bottom whenever new lines are appended
    use_effect(use_reactive(&lines_count, move |_| {
        let _ = document::eval(
            r#"
            setTimeout(() => {
                const el = document.getElementById('terminal-bottom-anchor');
                if (el) {
                    el.scrollIntoView({ behavior: 'smooth' });
                }
            }, 30);
            "#,
        );
    }));

    if !has_project {
        return rsx! {
            div { class: "w-full h-64 bg-[#0c0c0c] border-t border-[#222222] flex flex-col items-center justify-center text-center p-6 select-none shrink-0",
                div { class: "w-10 h-10 rounded-xl bg-[#141414] border border-[#262626] flex items-center justify-center mb-2 text-emerald-400 font-mono text-sm",
                    ">_"
                }
                h3 { class: "text-xs font-semibold text-neutral-200 mb-1 font-sans",
                    "Integrated Terminal"
                }
                p { class: "text-[11px] text-neutral-400 max-w-sm mb-4 font-sans",
                    "No project open. Open a project workspace to launch an integrated terminal."
                }
                div { class: "flex items-center gap-2",
                    button {
                        class: "px-3 py-1.5 rounded-lg bg-neutral-100 hover:bg-white text-black font-semibold text-xs font-mono transition-all cursor-pointer shadow-sm active:scale-95",
                        onclick: move |_| {
                            pick_and_open_project(state);
                        },
                        "Open Project"
                    }
                    button {
                        class: "px-3 py-1.5 rounded-lg bg-[#1a1a1a] hover:bg-[#222222] text-neutral-300 font-mono text-xs border border-[#2e2e2e] transition-all cursor-pointer",
                        onclick: move |_| {
                            state.write().show_terminal = false;
                        },
                        "Dismiss"
                    }
                }
            }
        };
    }

    let dir_for_prompt = dir_display.clone();

    let is_expanded = current_state.terminal.is_expanded;
    let container_class = if is_expanded {
        "w-full flex-1 h-full bg-[#0a0a0a] border-t border-[#202020] flex flex-col font-mono text-xs overflow-hidden select-none outline-none"
    } else {
        "w-full h-64 bg-[#0a0a0a] border-t border-[#202020] flex flex-col font-mono text-xs overflow-hidden select-none shrink-0 shadow-2xl outline-none"
    };

    rsx! {
        div {
            class: "{container_class}",
            tabindex: "0",
            onkeydown: move |evt| {
                let key = evt.key();
                let mods = evt.modifiers();
                let has_ctrl = mods.ctrl() || mods.meta();
                let has_shift = mods.shift();
                let is_c = key == Key::Character("c".to_string()) || key == Key::Character("C".to_string());
                let is_v = key == Key::Character("v".to_string()) || key == Key::Character("V".to_string());

                if has_ctrl && has_shift && is_c {
                    copy_selection_to_clipboard();
                } else if has_ctrl && has_shift && is_v {
                    paste_clipboard_to_input();
                } else if has_ctrl && !has_shift && is_c && state.read().terminal.is_running {
                    let kill_sender = get_kill_sender();
                    let _ = kill_sender.send(());
                    state.write().append_terminal_line(TerminalLineKind::System, "^C");
                    input_val.set(String::new());
                }
            },
            // Terminal Header Bar with Tabs, Add Tab (+), and Controls (Expand & Close)
            div { class: "h-8 bg-[#111111] border-b border-[#1c1c1c] px-3 flex items-center justify-between text-neutral-300 select-none shrink-0 gap-2",
                // Left: Tabs list and Add Tab button
                div { class: "flex items-center gap-1.5 overflow-x-auto scrollbar-none max-w-[70%]",
                    for (tab_idx, tab) in current_state.terminal.tabs.iter().enumerate() {
                        {
                            let is_active = tab_idx == current_state.terminal.active_tab_index;
                            let tab_name = tab.name.clone();
                            let is_tab_running = tab.is_running;
                            rsx! {
                                div {
                                    key: "{tab.id}",
                                    class: if is_active {
                                        "flex items-center gap-1.5 px-2.5 py-1 bg-[#1a1a1a] border-t border-x border-[#333333] rounded-t-md text-xs font-mono text-white cursor-pointer select-none"
                                    } else {
                                        "flex items-center gap-1.5 px-2.5 py-1 bg-[#121212] border-t border-x border-transparent hover:border-[#222222] hover:bg-[#161616] rounded-t-md text-xs font-mono text-neutral-400 hover:text-neutral-200 cursor-pointer select-none transition-colors"
                                    },
                                    onclick: move |_| {
                                        state.write().terminal.select_tab(tab_idx);
                                    },
                                    span { class: if is_active { "text-emerald-400 font-bold" } else { "text-neutral-500" }, ">_" }
                                    span { class: "truncate max-w-[100px]", "{tab_name}" }
                                    if is_tab_running {
                                        span { class: "w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse ml-0.5" }
                                    }
                                    button {
                                        class: "ml-1.5 text-neutral-500 hover:text-rose-400 transition-colors cursor-pointer text-xs leading-none p-0.5 rounded hover:bg-[#262626]",
                                        title: "Close this terminal tab",
                                        onclick: move |evt| {
                                            evt.stop_propagation();
                                            if is_tab_running {
                                                let kill_sender = get_kill_sender();
                                                let _ = kill_sender.send(());
                                            }
                                            state.write().terminal.close_tab(tab_idx);
                                        },
                                        "✕"
                                    }
                                }
                            }
                        }
                    }
                    button {
                        class: "px-2 py-0.5 rounded text-xs font-mono text-neutral-400 hover:text-white hover:bg-[#202020] border border-transparent hover:border-[#2a2a2a] transition-all cursor-pointer flex items-center justify-center ml-0.5",
                        title: "New Terminal Tab (+)",
                        onclick: move |_| {
                            let root = state.read().project_root.clone();
                            state.write().terminal.add_tab(root);
                        },
                        "+"
                    }
                    div {
                        class: "px-2 py-0.5 rounded bg-[#161616] border border-[#262626] text-[10px] text-neutral-400 truncate max-w-xs ml-1.5",
                        title: "{active_dir.display()}",
                        "📁 {dir_display}"
                    }
                }

                // Right: Status indicator, Expand / Collapse toggle, and Close panel button
                div { class: "flex items-center gap-2",
                    if is_running {
                        div { class: "flex items-center gap-1.5 text-amber-400 text-[10px] font-mono mr-1",
                            span { class: "w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" }
                            span { "Running (Press ^C to abort)" }
                        }
                    }
                    button {
                        class: "p-1 rounded text-neutral-400 hover:text-white hover:bg-[#222222] transition-colors cursor-pointer text-xs font-mono flex items-center justify-center",
                        title: if is_expanded { "Collapse Terminal Screen" } else { "Expand Terminal Screen" },
                        onclick: move |_| {
                            state.write().terminal.toggle_expanded();
                        },
                        if is_expanded {
                            "⤡"
                        } else {
                            "⤢"
                        }
                    }
                    button {
                        class: "p-1 rounded text-neutral-400 hover:text-rose-400 hover:bg-[#222222] transition-colors cursor-pointer text-xs font-mono flex items-center justify-center leading-none",
                        title: "Close Terminal Panel",
                        onclick: move |_| {
                            if state.read().terminal.is_running {
                                let kill_sender = get_kill_sender();
                                let _ = kill_sender.send(());
                            }
                            state.write().show_terminal = false;
                        },
                        "✕"
                    }
                }
            }

            // Terminal Output Area
            div {
                class: "flex-1 overflow-y-auto p-3 font-mono text-[11.5px] leading-relaxed bg-[#080808] select-text scrollbar-thin scrollbar-thumb-neutral-800 cursor-text",
                onclick: move |_| {
                    let _ = document::eval("document.getElementById('terminal-command-input')?.focus();");
                },
                if current_state.terminal.lines.is_empty() {
                    div { class: "text-neutral-500 mb-2 select-none",
                        div { class: "text-neutral-400 font-bold", "Uneven Integrated Terminal v0.2.2" }
                        div { "Project: {active_dir.display()}" }
                        div { class: "text-neutral-600 mt-1", "Type shell commands (e.g. cargo check, git status, ls) or use ^C to abort." }
                    }
                } else {
                    for (idx, line) in current_state.terminal.lines.iter().enumerate() {
                        {
                            let key = format!("term-line-{}", idx);
                            match line.kind {
                                TerminalLineKind::Command => rsx! {
                                    div { key: "{key}", class: "text-emerald-400 font-medium mt-1 mb-0.5 select-text",
                                        "{line.text}"
                                    }
                                },
                                TerminalLineKind::Stdout => rsx! {
                                    div { key: "{key}", class: "text-neutral-300 whitespace-pre-wrap break-all select-text",
                                        "{line.text}"
                                    }
                                },
                                TerminalLineKind::Stderr => rsx! {
                                    div { key: "{key}", class: "text-rose-400/90 whitespace-pre-wrap break-all select-text",
                                        "{line.text}"
                                    }
                                },
                                TerminalLineKind::System => rsx! {
                                    div { key: "{key}", class: "text-cyan-400/80 italic select-text",
                                        "{line.text}"
                                    }
                                },
                            }
                        }
                    }
                }
                div { id: "terminal-bottom-anchor" }
            }

            // Command Prompt Input Row
            div { class: "h-9 bg-[#0e0e0e] border-t border-[#1a1a1a] px-3 flex items-center gap-2 select-none shrink-0",
                span { class: "text-emerald-400 font-bold select-none text-[11px]",
                    "➜ {dir_for_prompt} $"
                }
                input {
                    id: "terminal-command-input",
                    class: "flex-1 bg-transparent text-white focus:outline-none font-mono text-[11.5px] caret-emerald-400 placeholder:text-neutral-600",
                    placeholder: if is_running { "Running... Press Ctrl+C (^C) to kill process" } else { "Type shell command and press Enter..." },
                    value: "{input_val}",
                    autofocus: true,
                    oninput: move |evt| {
                        if !state.read().terminal.is_running {
                            input_val.set(evt.value());
                        }
                    },
                    onkeydown: move |evt| {
                        let key = evt.key();
                        let mods = evt.modifiers();
                        let has_ctrl = mods.ctrl() || mods.meta();
                        let has_shift = mods.shift();
                        let is_c = key == Key::Character("c".to_string()) || key == Key::Character("C".to_string());
                        let is_v = key == Key::Character("v".to_string()) || key == Key::Character("V".to_string());

                        // Ctrl+Shift+C / Cmd+Shift+C: Explicit copy selected text
                        if has_ctrl && has_shift && is_c {
                            copy_selection_to_clipboard();
                            return;
                        }

                        // Ctrl+Shift+V / Cmd+Shift+V: Explicit paste clipboard into terminal
                        if has_ctrl && has_shift && is_v {
                            paste_clipboard_to_input();
                            return;
                        }

                        // Ctrl+C without Shift: Kill running process (or abort input line)
                        if has_ctrl && !has_shift && is_c {
                            if state.read().terminal.is_running {
                                let kill_sender = get_kill_sender();
                                let _ = kill_sender.send(());
                                state.write().append_terminal_line(TerminalLineKind::System, "^C");
                                input_val.set(String::new());
                            } else {
                                copy_selection_to_clipboard();
                                if !input_val.read().is_empty() {
                                    let val = input_val.read().clone();
                                    state.write().append_terminal_line(
                                        TerminalLineKind::Command,
                                        format!("➜ {}^C", val),
                                    );
                                    input_val.set(String::new());
                                }
                            }
                            return;
                        }

                        if key == Key::Enter {
                            if state.read().terminal.is_running {
                                return;
                            }
                            let raw_cmd = input_val.read().trim().to_string();
                            if !raw_cmd.is_empty() {
                                execute_terminal_input(raw_cmd, state, input_val, history_index);
                            }
                        } else if key == Key::ArrowUp && !state.read().terminal.is_running {
                            navigate_history_up(state, input_val, history_index);
                        } else if key == Key::ArrowDown && !state.read().terminal.is_running {
                            navigate_history_down(state, input_val, history_index);
                        }
                    }
                }
            }
        }
    }
}

fn navigate_history_up(
    state: Signal<AppState>,
    mut input_val: Signal<String>,
    mut history_index: Signal<Option<usize>>,
) {
    let history = state.read().terminal.command_history.clone();
    if history.is_empty() {
        return;
    }

    let cur = *history_index.read();
    let next_idx = match cur {
        None => history.len().saturating_sub(1),
        Some(idx) => idx.saturating_sub(1),
    };

    if let Some(cmd) = history.get(next_idx) {
        history_index.set(Some(next_idx));
        input_val.set(cmd.clone());
    }
}

fn navigate_history_down(
    state: Signal<AppState>,
    mut input_val: Signal<String>,
    mut history_index: Signal<Option<usize>>,
) {
    let history = state.read().terminal.command_history.clone();
    if history.is_empty() {
        return;
    }

    let cur = *history_index.read();
    if let Some(idx) = cur {
        if idx + 1 < history.len() {
            let next_idx = idx + 1;
            if let Some(cmd) = history.get(next_idx) {
                history_index.set(Some(next_idx));
                input_val.set(cmd.clone());
            }
        } else {
            history_index.set(None);
            input_val.set(String::new());
        }
    }
}

fn execute_terminal_input(
    cmd: String,
    mut state: Signal<AppState>,
    mut input_val: Signal<String>,
    mut history_index: Signal<Option<usize>>,
) {
    input_val.set(String::new());
    history_index.set(None);

    // Save into history
    state.write().terminal.command_history.push(cmd.clone());

    // Echo command to terminal output
    state.write().append_terminal_line(
        TerminalLineKind::Command,
        format!("➜ {}", cmd),
    );

    let working_dir = state
        .read()
        .terminal
        .current_dir
        .clone()
        .or_else(|| state.read().project_root.clone())
        .unwrap_or_else(|| PathBuf::from("."));

    // Handle Built-in Commands
    let trimmed = cmd.trim();
    if trimmed == "clear" {
        state.write().clear_terminal();
        return;
    }

    if trimmed == "exit" {
        state.write().show_terminal = false;
        return;
    }

    if trimmed == "pwd" {
        state.write().append_terminal_line(
            TerminalLineKind::Stdout,
            working_dir.display().to_string(),
        );
        return;
    }

    if trimmed.starts_with("cd ") || trimmed == "cd" {
        let target = trimmed.strip_prefix("cd").unwrap_or("").trim();
        handle_cd_command(target, working_dir, state);
        return;
    }

    // External process execution
    state.write().terminal.is_running = true;
    state.write().terminal.current_command = Some(cmd.clone());

    let kill_sender = get_kill_sender();
    let mut kill_rx = kill_sender.subscribe();

    spawn(async move {
        // Multiplatform shell resolution:
        // On Windows: checks COMSPEC or falls back to cmd.exe.
        // On macOS & Linux: checks $SHELL (e.g. /bin/zsh on modern macOS, /bin/bash) or falls back to /bin/sh.
        let mut command_proc = if cfg!(windows) {
            let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
            let mut c = tokio::process::Command::new(comspec);
            c.args(["/C", &cmd]);
            c
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            let mut c = tokio::process::Command::new(shell);
            c.args(["-c", &cmd]);
            c
        };

        command_proc.current_dir(&working_dir);
        command_proc.stdout(std::process::Stdio::piped());
        command_proc.stderr(std::process::Stdio::piped());

        let mut child = match command_proc.spawn() {
            Ok(c) => c,
            Err(e) => {
                state.write().append_terminal_line(
                    TerminalLineKind::Stderr,
                    format!("Failed to spawn process: {}", e),
                );
                state.write().terminal.is_running = false;
                state.write().terminal.current_command = None;
                return;
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let (line_tx, mut line_rx) = tokio::sync::mpsc::channel::<(TerminalLineKind, String)>(128);

        if let Some(out) = stdout {
            let tx = line_tx.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let cleaned = clean_ansi(&line);
                    let _ = tx.send((TerminalLineKind::Stdout, cleaned)).await;
                }
            });
        }

        if let Some(err) = stderr {
            let tx = line_tx.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let cleaned = clean_ansi(&line);
                    let _ = tx.send((TerminalLineKind::Stderr, cleaned)).await;
                }
            });
        }
        drop(line_tx);

        let process_killed = Arc::new(AtomicBool::new(false));
        let killed_flag = Arc::clone(&process_killed);

        loop {
            tokio::select! {
                Some((kind, line)) = line_rx.recv() => {
                    state.write().append_terminal_line(kind, line);
                }
                res = child.wait() => {
                    // Flush any pending lines
                    while let Ok((kind, line)) = line_rx.try_recv() {
                        state.write().append_terminal_line(kind, line);
                    }

                    if killed_flag.load(Ordering::Relaxed) {
                        state.write().append_terminal_line(
                            TerminalLineKind::System,
                            "Process terminated by user (SIGKILL)",
                        );
                    } else {
                        match res {
                            Ok(status) => {
                                let code = status.code().unwrap_or(0);
                                if status.success() {
                                    state.write().append_terminal_line(
                                        TerminalLineKind::System,
                                        format!("Process finished with exit code {}", code),
                                    );
                                } else {
                                    state.write().append_terminal_line(
                                        TerminalLineKind::Stderr,
                                        format!("Process exited with status code {}", code),
                                    );
                                }
                            }
                            Err(e) => {
                                state.write().append_terminal_line(
                                    TerminalLineKind::Stderr,
                                    format!("Error waiting for process: {}", e),
                                );
                            }
                        }
                    }
                    break;
                }
                _ = kill_rx.recv() => {
                    killed_flag.store(true, Ordering::Relaxed);
                    let _ = child.kill().await;
                }
            }
        }

        state.write().terminal.is_running = false;
        state.write().terminal.current_command = None;
    });
}

fn handle_cd_command(target: &str, working_dir: PathBuf, mut state: Signal<AppState>) {
    let target_path = if target.is_empty() || target == "~" {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
        } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
            PathBuf::from(userprofile)
        } else {
            state.read().project_root.clone().unwrap_or(working_dir)
        }
    } else {
        let p = Path::new(target);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            working_dir.join(p)
        }
    };

    match target_path.canonicalize() {
        Ok(canonical) => {
            if canonical.is_dir() {
                state.write().terminal.current_dir = Some(canonical.clone());
                state.write().append_terminal_line(
                    TerminalLineKind::System,
                    format!("Directory changed to: {}", canonical.display()),
                );
            } else {
                state.write().append_terminal_line(
                    TerminalLineKind::Stderr,
                    format!("cd: not a directory: {}", target),
                );
            }
        }
        Err(e) => {
            state.write().append_terminal_line(
                TerminalLineKind::Stderr,
                format!("cd: cannot access '{}': {}", target, e),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_ansi_strips_color_codes() {
        let input = "\x1b[32mHello\x1b[0m \x1b[1;31mWorld\x1b[0m";
        let cleaned = clean_ansi(input);
        assert_eq!(cleaned, "Hello World");
    }

    #[test]
    fn test_terminal_state_default() {
        let state = AppState::default();
        assert!(!state.show_terminal);
        assert!(!state.terminal.is_running);
        assert!(state.terminal.lines.is_empty());
    }

    #[test]
    fn test_terminal_append_and_clear() {
        let mut state = AppState::default();
        state.append_terminal_line(TerminalLineKind::Command, "cargo check");
        state.append_terminal_line(TerminalLineKind::Stdout, "Finished dev");
        assert_eq!(state.terminal.lines.len(), 2);
        assert_eq!(state.terminal.lines[0].kind, TerminalLineKind::Command);
        assert_eq!(state.terminal.lines[1].kind, TerminalLineKind::Stdout);

        state.clear_terminal();
        assert!(state.terminal.lines.is_empty());
    }
}

