#![allow(unused_imports)]
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use uneven_core::UnevenConfig;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::knowledge::scanner::ProjectScanner;
use uneven_core::infrastructure::security::SafetyGuard;
use uneven_core::infrastructure::llm::init as init_llm;
use crate::ui::{self, truecolor, truecolor_bold};
use super::patch_engine;
use super::resource_guardian::ResourceGuardian;
use super::think;

pub mod slash_commands;
pub mod terminal_runner;

pub use slash_commands::SlashOutcome;
pub use terminal_runner::{
    execute_terminal_command_streaming, format_duration, get_hint, log_command_execution,
    parse_analyze_input, print_shell_help, print_welcome, restore_terminal_ui, spawn_subcommand,
    spawn_subcommand_silent, spawn_subcommand_silent_in_dir, SUBPROCESS_RUNNING,
};

// Re-exports from core for full backward compatibility
pub use uneven_core::application::model_routing::{
    allocate_brains, match_model_string, parse_multi_brains,
};
pub use uneven_core::infrastructure::knowledge::context_builder::{
    is_binary_file, load_and_format_file, load_and_format_web_page, parse_all_need_file_tags,
    parse_all_web_search_tags, parse_all_web_url_tags, run_and_format_web_search,
    sync_external_project,
};
pub use uneven_core::infrastructure::llm::history_manager::{
    delete_history_from_disk, get_history_from_disk, get_project_files_map,
    list_project_sessions, save_history_to_disk, BrainTurn,
};
pub use uneven_core::application::shell_classifier::{
    classify_intent, classify_shell_intent, classify_with_brain, extract_file_path, json_to_intent,
    tokenize_shell_input, ClassifiedIntent, ShellAction,
};

pub async fn classify(
    input: &str,
    history: &[BrainTurn],
    files_map: &str,
    config: &UnevenConfig,
    external_files: Option<&[String]>,
) -> (ClassifiedIntent, String, usize) {

    let quick = classify_intent(input);
    let is_approval = uneven_core::application::shell_classifier::is_approval_intent(input);
    let is_utility = matches!(
        quick.action,
        ShellAction::Exit
            | ShellAction::Help
            | ShellAction::Log
            | ShellAction::Undo
            | ShellAction::Index
            | ShellAction::Scan
            | ShellAction::Watch
    ) || is_approval;

    // If the intent is a utility or approval intent, return it immediately
    if (quick.confidence == "high" && is_utility) || is_approval {
        let mut intent = quick;
        if is_approval {
            intent.action = ShellAction::Askf;
            intent.confidence = "high".to_string();
            intent.task = Some(input.to_string());
        }
        return (intent, String::new(), 0);
    }

    // If the intent is not a utility, continue with the spinner animation
    let frames = [
        format!("{} {}", style("⠋").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠙").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠹").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠸").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠼").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠴").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠦").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠧").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠇").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠏").cyan(), style("Classifying intent…").dim()),
    ];

    // Convert frames to a slice of strings for the spinner animation
    let frames_ref: Vec<&str> = frames.iter().map(|s| s.as_str()).collect();
    let classify_spinner = ProgressBar::new_spinner();
    classify_spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&frames_ref)
            .template("  ↳ {spinner}").unwrap(),
    );
    classify_spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    // If the brain provider is not local, attempt to classify with the brain
    if config.brain.provider != "local" && !config.brain.provider.is_empty()
        && let Some((mut intent, message, tokens)) = classify_with_brain(input, history, files_map, config, external_files).await {
            classify_spinner.finish_and_clear();

            // If the brain did not provide a clear intent or classified as ask, route to think
            if intent.action == ShellAction::Unknown || intent.action == ShellAction::None || intent.action == ShellAction::Ask {
                intent.action = ShellAction::Think;
                intent.task = Some(input.to_string());
            }
            return (intent, message, tokens);
        }

    classify_spinner.finish_and_clear();

    // If no clear intent, fall back to think engine
    let final_intent = if quick.action == ShellAction::Unknown || quick.action == ShellAction::Ask {
        ClassifiedIntent {
            action: ShellAction::Think,
            task: Some(input.to_string()),
            confidence: "high".to_string(),
            file: None,
            question: None,
            commit: None,
            from: None,
            staged: false,
            base: None,
            paths: Vec::new(),
            report: false,
            tasks: None,
        }
    } else {
        quick
    };

    // Return the final intent, an empty message, and 0 tokens
    (final_intent, String::new(), 0)
}


#[allow(dead_code)]
pub fn get_shell_prompt() -> String {
    "  \x1b[1;38;2;56;189;248mSnatchy\x1b[0m \x1b[1;38;2;56;189;248m›\x1b[0m ".to_string()
}

pub fn render_bottom_status_bar(
    config: &UnevenConfig,
    project_name: &str,
    is_loading: bool,
    rows: u16,
    cols: u16,
) {
    if rows <= 3 {
        return;
    }

    let scroll_bottom = if rows > 5 { rows - 3 } else { rows - 1 };
    print!("\x1b[1;{}r\x1b[{};1H\x1b[2K", scroll_bottom, rows);

    // Cyan color token: RGB(56, 189, 248) -> #38bdf8
    let cyan = |s: &str| format!("\x1b[1;38;2;56;189;248m{}\x1b[0m", s);
    let sep = "\x1b[38;2;55;65;81m│\x1b[0m";

    // 1. Left segments:
    let badge = " \x1b[1;38;2;240;244;250mSNATCHY\x1b[0m".to_string();

    let project_badge = format!(
        "\x1b[38;2;148;163;184m{}\x1b[0m",
        project_name
    );

    let shortcuts = format!(
        "{} \x1b[38;2;203;213;225mMenu\x1b[0m  {} \x1b[38;2;203;213;225mHelp\x1b[0m  {} \x1b[38;2;203;213;225mExit\x1b[0m",
        cyan("[/]"),
        cyan("[?]"),
        cyan("[^C]")
    );

    let max_cols = (cols as usize).saturating_sub(1);

    let left_full = format!("{} {} {} {} {}", badge, sep, project_badge, sep, shortcuts);
    let left_compact = format!("{} {} {}", badge, sep, shortcuts);
    let mut left_side = if max_cols < 80 {
        left_compact.clone()
    } else {
        left_full
    };

    // 2. Right segments:
    let quota = uneven_core::application::TokenQuotaService::get_quota_metrics();
    let quota_color = if quota.turn_5h_percent >= 90.0 {
        "\x1b[1;38;2;244;63;94m" // Rose
    } else if quota.turn_5h_percent >= 70.0 {
        "\x1b[1;38;2;251;191;36m" // Amber
    } else {
        "\x1b[1;38;2;56;189;248m" // Cyan
    };

    let quota_segment = format!(
        "\x1b[38;2;148;163;184m5h:\x1b[0m {}{:.0}%\x1b[0m",
        quota_color,
        quota.turn_5h_percent
    );

    let raw_brain_name = if is_loading {
        format!("{}/{} (Loading...)", config.brain.provider, config.brain.model)
    } else {
        format!("{}/{}", config.brain.provider, config.brain.model)
    };

    let right_full = format!(
        "{} {} \x1b[38;2;226;232;240m{}\x1b[0m ",
        quota_segment, sep, raw_brain_name
    );
    let right_compact = format!("\x1b[38;2;226;232;240m{}\x1b[0m ", raw_brain_name);

    let mut right_side = if max_cols < 95 {
        right_compact.clone()
    } else {
        right_full
    };

    // Ensure left + right fits strictly within max_cols without wrapping
    let mut left_len = console::measure_text_width(&left_side);
    let mut right_len = console::measure_text_width(&right_side);

    if left_len + 2 + right_len > max_cols {
        right_side = right_compact;
        right_len = console::measure_text_width(&right_side);
    }
    if left_len + 2 + right_len > max_cols {
        left_side = left_compact;
        left_len = console::measure_text_width(&left_side);
    }
    if left_len + 2 + right_len > max_cols {
        let avail_for_brain = max_cols.saturating_sub(left_len + 3);
        let truncated_brain: String = if avail_for_brain > 1 {
            let mut s: String = raw_brain_name.chars().take(avail_for_brain - 1).collect();
            s.push('…');
            s
        } else {
            String::new()
        };
        right_side = if truncated_brain.is_empty() {
            String::new()
        } else {
            format!("\x1b[38;2;226;232;240m{}\x1b[0m ", truncated_brain)
        };
        right_len = console::measure_text_width(&right_side);
    }

    let total_len = left_len + right_len;
    let padding = max_cols.saturating_sub(total_len);

    let bar = format!("{}{:<width$}{}", left_side, "", right_side, width = padding);
    let bar_bg = format!("\x1b[?7l\x1b[48;2;14;16;22m{}\x1b[0m\x1b[?7h", bar);
    print!("{}", bar_bg);

    if rows > 5 {
        print!("\x1b[{};1H\x1b[2K", rows - 1);
        print!("\x1b[{};1H\x1b[2K", rows - 2);
    }
    let _ = std::io::stdout().flush();
}

#[allow(dead_code)]
pub async fn run() -> Result<()> {
    struct TerminalGuard;
    impl Drop for TerminalGuard {
        fn drop(&mut self) {
            print!("\x1b[r");
            let _ = std::io::stdout().flush();
        }
    }
    let _guard = TerminalGuard;

    let root = loader::find_project_root(None);
    let is_first_run = std::env::var("UNEVEN_FIRST_RUN").is_ok();

    let mut config = loader::load_config(&root)?;
    // run index in the background
    tokio::spawn(async move {
        let _ = spawn_subcommand_silent(&["index"]).await;
    });

    // load external project files if configured
    let mut external_files_list: Option<Vec<String>> = None;
    if let Some(ref ext_project_path) = config.knowledge.as_ref().and_then(|k| k.external_project.as_ref()) {
        let ext_path = std::path::Path::new(ext_project_path);
        let ext_hash = uneven_core::infrastructure::utils::paths::project_hash(ext_path);
        let cache_path = uneven_core::infrastructure::utils::paths::global_bin_dir()
            .join("cache")
            .join(&ext_hash)
            .join("file-list-cache.json");
        if cache_path.exists()
            && let Ok(content) = std::fs::read_to_string(&cache_path)
                && let Ok(files) = serde_json::from_str::<Vec<String>>(&content) {
                    external_files_list = Some(files);
                }

        let ext_dir = std::path::PathBuf::from(ext_project_path);
        tokio::spawn(async move {
            let _ = spawn_subcommand_silent_in_dir(&["index"], &ext_dir).await;
        });
    }

    // Session id based on brain provider
    let mut last_session_id: Option<String> = None;

    // set threads and project name based on brain local config
    let threads = config.brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    let project_name = root.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let version = env!("CARGO_PKG_VERSION");
    let active_brain = format!("{} / {}", config.brain.provider, config.brain.model);
    print_welcome(version, &active_brain, is_first_run);

    // show welcome message and setup instructions if this is the first run
    if is_first_run {
        let accent = |s: &str| style(s.to_string()).color256(111).bold();
        let dim = |s: &str| style(s.to_string()).dim();
        let bold = |s: &str| style(s.to_string()).bold();

        println!("  {} Welcome to Uneven Agent! Let's get you set up.", style("◈").color256(111).bold());
        println!("  {} To get started, type {} (to open the menu) and select {} to choose your model before continuing.",
            style("›").color256(111).bold(), accent("/"), accent("/models")
        );
        println!();
        println!("  {} Configuring Cloud API Keys (if using cloud brains):", style("ℹ").color256(111).bold());
        println!("  You can supply your API keys (e.g., GOOGLE_API_KEY or ANTHROPIC_API_KEY) in three ways:");
        println!("    {}  {}   Export them directly in your current terminal session (e.g., export GOOGLE_API_KEY=...).", dim("1."), bold("Session:"));
        println!("    {}  {}   Create a {} file in the root directory of your project.", dim("2."), bold("Project:"), accent(".env"));
        println!("    {}  {}   Use the {} command inside this shell. This is a global, secure,", dim("3."), bold("Global:"), accent("/export <KEY> [value]"));
        println!("                  and persistent setup stored at {} that works across all your projects.", dim("~/.uneven/bin/env"));
        println!();
    }

    // determine if the brain is local or cloud-based
    let is_local = config.brain.provider == "local";
    let term = console::Term::stdout();
    let (rows, cols) = term.size();

    // Render initial status bar and prompt
    if rows > 5 {
        render_bottom_status_bar(&config, project_name, is_local, rows, cols);

        // Draw initial prompt
        print!("\x1b[{};1H\x1b[2K", rows - 3);
        let prompt = get_shell_prompt();
        print!("{}", prompt);
        let _ = std::io::stdout().flush();
    }

    // Hide cursor if local
    if is_local {
        print!("\x1b[?25l");
        let _ = std::io::stdout().flush();
    }

    let _ = init_llm(&config.brain.provider, &config.brain.model, threads).await;

    // Restore cursor and status bar if local
    if is_local {
        print!("\x1b[?25h");
        if rows > 5 {
            render_bottom_status_bar(&config, project_name, false, rows, cols);
            print!("\x1b[{};1H", rows - 3);
            let _ = std::io::stdout().flush();
        }
    }

    // spawn a task to handle Ctrl+C signal
    tokio::spawn(async move {
        loop {
            let _ = tokio::signal::ctrl_c().await;
            if !SUBPROCESS_RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                restore_terminal_ui();
                println!("{}", truecolor("\n  Goodbye!\n", 107, 122, 150));
                {
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }
                std::process::exit(0);
            }
        }
    });

    // initialize pending index for tracking pending index updates
    let pending_index = Arc::new(AtomicBool::new(false));
    let pending_index_clone = pending_index.clone();

    // load configuration from cache
    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
    let mut _watcher = None;
    let handle = tokio::runtime::Handle::current();
    // watch for changes to the config file
    if config_path.exists() {
        let handle_clone = handle.clone();
        if let Ok(mut fw) = uneven_core::infrastructure::watcher::file::FileWatcher::new(move |path| {
            let path_lower = path.to_lowercase();
            let path_buf = std::path::Path::new(&path_lower);
            let is_ignored = path_buf.components().any(|c| {
                if let Some(s) = c.as_os_str().to_str() {
                    s == ".env"
                        || s == ".uneven"
                        || s == "node_modules"
                        || s == "target"
                        || s == "dist"
                        || s == "build"
                        || s == "venv"
                        || s == ".venv"
                } else {
                    false
                }
            });

            // if the path is not ignored, mark pending index as true and spawn a task to update it
            if !is_ignored {
                pending_index_clone.store(true, Ordering::Relaxed);
                handle_clone.spawn(async move {
                    let _ = spawn_subcommand_silent(&["index"]).await;
                });
            }
        }) {
            let _ = fw.watch(&root.to_string_lossy());
            let _ = fw.watch(&config_path.to_string_lossy());
            _watcher = Some(fw);
        }
    }

    // initialize history and safety guards for the interactive session
    let mut history: Vec<BrainTurn> = Vec::new();
    let safety = SafetyGuard::new();
    let mut auto_prompt: Option<String> = None;
    let mut always_approve_commands = false;

    // initialize readline for user input
    let rl = rustyline::DefaultEditor::new()?;
    let rl = Arc::new(std::sync::Mutex::new(rl));

    // main interactive loop
    loop {
        // reload config if pending index is set
        if pending_index.swap(false, Ordering::Relaxed)
            && let Ok(new_cfg) = loader::load_config(&root) {
                config = new_cfg;
            }

        // Get user input
        let input = if let Some(ap) = auto_prompt.take() {
            ap
        } else {
            // Draw the sticky bottom status bar
            let term = console::Term::stdout();
            let (rows, cols) = term.size();
            render_bottom_status_bar(&config, project_name, false, rows, cols);
            if rows > 5 {
                // Move cursor to the prompt line (rows - 3) and clear it
                print!("\x1b[{};1H\x1b[2K", rows - 3);
                let _ = std::io::stdout().flush();
            } else if rows > 3 {
                print!("\x1b[{};1H\x1b[2K", rows - 1);
                let _ = std::io::stdout().flush();
            }

            // Force cursor to be shown and blinking before reading input, and clear prompt line
            print!("\x1b[?25h\x1b[?12h\r\x1b[2K");
            let _ = std::io::stdout().flush();

            let prompt = get_shell_prompt();

            let rl_clone = rl.clone();
            let prompt_clone = prompt.clone();

            // readline in a blocking task to avoid blocking the tokio runtime
            let line_res = match tokio::task::spawn_blocking(move || {
                let mut guard = rl_clone.lock().unwrap();
                guard.readline(&prompt_clone)
            }).await {
                Ok(Ok(line)) => Ok(line),
                Ok(Err(rustyline::error::ReadlineError::Interrupted)) => Err("interrupted"),
                Ok(Err(rustyline::error::ReadlineError::Eof)) => Err("eof"),
                _ => Err("error"),
            };

            // handle readline result
            match line_res {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let mut guard = rl.lock().unwrap();
                    let _ = guard.add_history_entry(trimmed);
                    line
                }
                Err("interrupted") => {
                    restore_terminal_ui();
                    println!("{}", truecolor("\n  Goodbye!\n", 107, 122, 150));
                    std::process::exit(0);
                }
                _ => break,
            }
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        // handle session id command
        if trimmed.starts_with("/id") {
            // Get session_id from input
            let raw_id = trimmed.strip_prefix("/id").unwrap_or("").to_string();

            if let Some(mut loaded_history) = get_history_from_disk(&raw_id, &root) {
                uneven_core::application::ContextCompactorService::auto_compact(&mut loaded_history);
                history = loaded_history;
                last_session_id = Some(raw_id);
                println!(" ✓ History received successfully!");
            } else {
                println!(" ✗ No history found for session '{raw_id}'.");
            }
        }

        // handle slash commands
        if trimmed.starts_with('/') {
            match slash_commands::handle_slash_command(trimmed, &root, &mut config, &mut external_files_list).await {
                SlashOutcome::Handled => continue,
                SlashOutcome::Exit => break,
                SlashOutcome::Unknown => break,
            }
        }

        // Generate session id if not provided
        let interactive_session_id = last_session_id.get_or_insert_with(|| uuid::Uuid::new_v4().to_string()[..6].to_uppercase()).clone();

        // Parse the command and arguments
        let tokens = tokenize_shell_input(trimmed);
        if tokens.is_empty() {
            continue;
        }

        let cmd = tokens[0].to_lowercase();

        let is_subprocess_cmd = matches!(
            cmd.as_str(),
            "init"
                | "info"
                | "start"
                | "watch"
                | "index"
                | "chat"
                | "autofix"
                | "analyze"
                | "test"
                | "undo"
                | "restore"
                | "diff"
                | "ci"
                | "log"
                | "reset"
                | "license"
                | "update"
        );

        if is_subprocess_cmd {
            let args = if cmd == "analyze" {
                let input_after_analyze = trimmed.strip_prefix("analyze").unwrap_or("").trim();
                parse_analyze_input(input_after_analyze)
            } else {
                tokens.clone()
            };

            let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

            // Elegant execution message
            let action_desc = match cmd.as_str() {
                "init" => "Initializing Uneven…",
                "info" => "Loading system diagnostics…",
                "start" => "Starting Uneven agent…",
                "watch" => "Starting watcher… Press Ctrl+C to stop and return to the shell.",
                "index" => "Indexing your project…",
                "chat" => "Starting chat mode…",
                "autofix" => "Starting proactive autofix service…",
                "analyze" => "Running database analysis and BI generation…",
                "pentest" => "Running security pentest…",
                "scan" => "Scanning for vulnerabilities and malicious patterns…",
                "test" => "Running project tests…",
                "undo" => "Reverting last fix…",
                "restore" => "Restoring file snapshot…",
                "diff" => "Reviewing pending fix suggestions…",
                "ci" => "Running CI pipeline…",
                "log" => "Loading activity log…",
                "reset" => "Clearing memory and starting fresh…",
                "license" => "Managing Uneven license…",
                "update" => "Checking for updates…",
                _ => "Running command…",
            };

            println!("\n{}", truecolor(&format!("  ↳ {}", action_desc), 107, 122, 150));
            println!();

            let code = spawn_subcommand(&args_refs).await.unwrap_or(1);

            // Success hint if code == 0
            if code == 0 {
                let hint = match cmd.as_str() {
                    "index" => Some("Project indexed. Try \"scan\" or ask anything about your code."),
                    "scan" => Some("Scan complete. Use \"watch\" to monitor errors in real time."),
                    "watch" => Some("Watcher stopped. Back to interactive mode."),
                    "undo" => Some("Fix reverted. The code is back to its previous state."),
                    _ => None,
                };
                if let Some(h) = hint {
                    println!("\n{} {}", truecolor("  › ", 0, 255, 140), truecolor(h, 107, 122, 150));
                    println!();
                }
            }

            continue;
        }

        let is_approval = uneven_core::application::shell_classifier::is_approval_intent(trimmed);
        let is_inline_cmd = matches!(
            cmd.as_str(),
            "exit" | "quit" | "help" | "ask" | "askf" | "think" | "plan" | "explain" | "docs"
        ) || is_approval;

        // Classify the command and arguments
        let (intent, message) = if is_inline_cmd {
            let mut intent = ClassifiedIntent {
                action: ShellAction::Unknown,
                file: None,
                task: None,
                question: None,
                commit: None,
                from: None,
                staged: false,
                confidence: "high".to_string(),
                base: None,
                paths: Vec::new(),
                report: false,
                tasks: None,
            };

            if is_approval {
                intent.action = ShellAction::Askf;
                intent.task = Some(trimmed.to_string());
            } else {
                match cmd.as_str() {
                "exit" | "quit" => {
                    intent.action = ShellAction::Exit;
                }
                "help" => {
                    intent.action = ShellAction::Help;
                }
                "ask" | "think" | "plan" => {
                    intent.action = ShellAction::Think;
                    let raw_args = trimmed.strip_prefix(cmd.as_str()).unwrap_or("").trim();
                    let task = if (raw_args.starts_with('"') && raw_args.ends_with('"'))
                        || (raw_args.starts_with('\'') && raw_args.ends_with('\''))
                    {
                        if raw_args.len() >= 2 {
                            &raw_args[1..raw_args.len() - 1]
                        } else {
                            raw_args
                        }
                    } else {
                        raw_args
                    };
                    intent.task = Some(task.to_string());
                }
                "askf" => {
                    intent.action = ShellAction::Askf;
                    let raw_args = trimmed.strip_prefix("askf").unwrap_or("").trim();
                    let task = if (raw_args.starts_with('"') && raw_args.ends_with('"'))
                        || (raw_args.starts_with('\'') && raw_args.ends_with('\''))
                    {
                        if raw_args.len() >= 2 {
                            &raw_args[1..raw_args.len() - 1]
                        } else {
                            raw_args
                        }
                    } else {
                        raw_args
                    };
                    intent.task = Some(task.to_string());
                }
                "explain" => {
                    intent.action = ShellAction::Explain;
                    let raw_args = trimmed.strip_prefix("explain").unwrap_or("").trim();
                    let file = if (raw_args.starts_with('"') && raw_args.ends_with('"'))
                        || (raw_args.starts_with('\'') && raw_args.ends_with('\''))
                    {
                        if raw_args.len() >= 2 {
                            &raw_args[1..raw_args.len() - 1]
                        } else {
                            raw_args
                        }
                    } else {
                        raw_args
                    };
                    intent.file = if file.is_empty() { None } else { Some(file.to_string()) };
                }
                "docs" => {
                    intent.action = ShellAction::Docs;
                    let raw_args = trimmed.strip_prefix("docs").unwrap_or("").trim();
                    let file = if (raw_args.starts_with('"') && raw_args.ends_with('"'))
                        || (raw_args.starts_with('\'') && raw_args.ends_with('\''))
                    {
                        if raw_args.len() >= 2 {
                            &raw_args[1..raw_args.len() - 1]
                        } else {
                            raw_args
                        }
                    } else {
                        raw_args
                    };
                    intent.file = if file.is_empty() { None } else { Some(file.to_string()) };
                }
                _ => {}
            }
            }

            (intent, String::new())
        } else {
            // Get project files map for brain classifier
            let dirs = config.knowledge.as_ref()
                .and_then(|k| k.dirs.clone())
                .unwrap_or_else(|| vec![".".to_string()]);
            let scanner = ProjectScanner::new(&root);
            let mut files = scanner.scan(dirs.clone()).unwrap_or_default();

            // Add extra files from config
            if let Some(extra_files) = config.knowledge.as_ref().and_then(|k| k.files.clone()) {
                for f in extra_files {
                    let normalized_f = f.replace('\\', "/");
                    let abs_path = root.join(&normalized_f);
                    if abs_path.exists() && abs_path.is_file() {
                        files.push(normalized_f);
                    }
                }
                files.sort();
                files.dedup();
            }

            // Classify the command and arguments
            let files_map = files.join("\n");
            let (intent, msg, _tokens) = classify(trimmed, &history, &files_map, &config, external_files_list.as_deref()).await;
            (intent, msg)
        };

        // Add the user's command to the history and auto-compact context
        history.push(BrainTurn { role: "user".to_string(), content: trimmed.to_string() });
        uneven_core::application::ContextCompactorService::auto_compact(&mut history);

        // Handle the classified intent
        if intent.action == ShellAction::Exit {
            // Save history with session ID
            let _ = save_history_to_disk(&history, &interactive_session_id, &root);
            break;
        }
        let has_files_in_message = intent.action == ShellAction::RunCmd;
        let is_none_action = intent.action == ShellAction::None;

        // Display the message if it's not empty and the action is not None or RunCmd
        if !message.is_empty() && (is_none_action || has_files_in_message) {
            let label = if intent.action == ShellAction::RunCmd {
                "  › Command summary: "
            } else {
                "  › "
            };
            println!();
            println!(
                "{}{}",
                truecolor(label, 168, 200, 255),
                truecolor(&message, 232, 237, 245)
            );
            println!();
            history.push(BrainTurn { role: "assistant".to_string(), content: message.clone() });
            uneven_core::application::ContextCompactorService::auto_compact(&mut history);
        }

        // Execute the command and handle the result
        let code = match intent.action {
            ShellAction::Index => {
                println!("{}", truecolor("\n  ↳ Indexing your project…\n", 107, 122, 150));
                spawn_subcommand(&["index"]).await.unwrap_or(1)
            }
            ShellAction::Scan => {
                let target_dir = intent.clean_target_dir().unwrap_or_else(|| ".".to_string());
                println!("{}", truecolor(&format!("\n  ↳ Running Snatchy security scan on '{}'…\n", target_dir), 107, 122, 150));
                let _ = crate::commands::scan::run(target_dir, false, false).await;
                0
            }
            ShellAction::Pentest => {
                let is_active = intent.is_active_pentest();
                let mode_str = if is_active { "active" } else { "static" };
                println!("{}", truecolor(&format!("\n  ↳ Running Snatchy pentest audit ({})…\n", mode_str), 107, 122, 150));
                let _ = crate::commands::pentest::run(mode_str.to_string(), None, false, None, None).await;
                0
            }
            ShellAction::Zeroday => {
                let is_deep = intent.is_deep();
                let target_dir = intent.clean_target_dir();
                let mode_label = if is_deep { "deep autonomous Think audit" } else { "taint scan" };
                println!("{}", truecolor(&format!("\n  ↳ Running Snatchy zero-day vulnerability {}…\n", mode_label), 107, 122, 150));
                let _ = crate::commands::zeroday::run(target_dir, None, is_deep).await;
                0
            }
            ShellAction::Analyze => {
                let prompt_arg = intent.task.clone().unwrap_or_else(|| trimmed.to_string());
                println!("{}", truecolor("\n  ↳ Running database analysis…\n", 107, 122, 150));
                let _ = crate::commands::analyze::run(
                    None,
                    Some(prompt_arg),
                    "html".to_string(),
                    "dark".to_string(),
                    false,
                    false,
                    None,
                    None,
                ).await;
                0
            }
            ShellAction::Watch => {
                println!("{}", truecolor("\n  Starting watcher… Press Ctrl+C to stop and return to the shell.\n", 107, 122, 150));
                spawn_subcommand(&["watch"]).await.unwrap_or(1)
            }
            ShellAction::Log => {
                println!("{}", truecolor("\n  ↳ Loading activity log…\n", 107, 122, 150));
                spawn_subcommand(&["log"]).await.unwrap_or(1)
            }
            ShellAction::Stats => {
                println!("{}", truecolor("\n  ↳ Loading usage statistics…\n", 107, 122, 150));
                spawn_subcommand(&["stats"]).await.unwrap_or(1)
            }
            ShellAction::Undo => {
                println!("{}", truecolor("\n  ↳ Reverting last fix…\n", 107, 122, 150));
                spawn_subcommand(&["undo"]).await.unwrap_or(1)
            }
            ShellAction::Help => {
                print_shell_help();
                0
            }
            ShellAction::Exit => {
                println!("Session history saved with ID: {}", interactive_session_id);
                println!("Return conversation with command in chat: /history {}", interactive_session_id);
                break;
            }
            ShellAction::None => {
                0
            }
            ShellAction::Explain => {
                let file = match intent.file {
                    Some(f) => f,
                    None => {
                        println!("\n{}", style("  Which file should I explain? e.g. \"explain src/app.ts\"").yellow());
                        "".to_string()
                    }
                };

                if !file.is_empty() {
                    let task = format!("explain the architecture, flow, and responsibilities of `{}`", file);
                    let _ = think::run(task, vec![file], None, None).await;
                    0
                } else {
                    1
                }
            }
            ShellAction::Docs => {
                let file = match intent.file {
                    Some(f) => f,
                    None => {
                        println!("\n{}", style("  Which file should I document? e.g. \"docs src/parser.ts\"").yellow());
                        "".to_string()
                    }
                };

                if !file.is_empty() {
                    let task = format!("document all exported symbols in `{}` with idiomatic documentation comments", file);
                    let _ = think::run(task, vec![file], None, None).await;
                    0
                } else {
                    1
                }
            }
            ShellAction::Ask | ShellAction::Think => {
                let task = intent.task.or(intent.question).unwrap_or_default();
                if !task.is_empty() {
                    match think::run(task, intent.paths, None, None).await {
                        Ok(answer) => {
                            if !answer.is_empty() {
                                history.push(BrainTurn {
                                    role: "assistant".to_string(),
                                    content: answer,
                                });
                                uneven_core::application::ContextCompactorService::auto_compact(&mut history);
                            }
                            0
                        }
                        Err(e) => {
                            println!("{}", style(format!("  Failed to think/plan: {}", crate::errors::sanitize_error_msg(&e.to_string()))).red());
                            1
                        }
                    }
                } else {
                    1
                }
            }
            ShellAction::Askf => {
                // Askf: file-based question-answering / patch application
                let task = match intent.task {
                    Some(t) => t,
                    None => "".to_string(),
                };
                let session_id = interactive_session_id.clone();
                let is_approval = uneven_core::application::shell_classifier::is_approval_intent(&task)
                    || task.trim().is_empty()
                    || task.trim().to_lowercase() == "apply";

                // 1. Check if there is a pending patch on disk or in recent history
                let pending_disk = uneven_core::infrastructure::llm::history_manager::get_pending_patch_from_disk(&session_id, &root)
                    .or_else(|| uneven_core::infrastructure::llm::history_manager::get_pending_patch_from_disk("default", &root));

                let candidate_file_obj = if let Some(p) = pending_disk {
                    Some(p)
                } else {
                    let candidate = if let Some(last) = history.last() {
                        if last.role == "assistant" && !last.content.is_empty() {
                            &last.content
                        } else {
                            &message
                        }
                    } else {
                        &message
                    };
                    patch_engine::file_object(candidate, &session_id)
                };

                if let Some(file_obj) = candidate_file_obj
                    && !file_obj.patch.is_empty() {
                        println!(
                            "{} {}",
                            truecolor("  › ", 168, 200, 255),
                            truecolor("Applying file actions:", 107, 122, 150)
                        );
                        patch_engine::file_action(&root, &file_obj);
                        println!();

                        let _ = uneven_core::infrastructure::llm::history_manager::clear_pending_patch_from_disk(&session_id, &root);
                        let _ = uneven_core::infrastructure::llm::history_manager::clear_pending_patch_from_disk("default", &root);

                        history.push(BrainTurn {
                            role: "assistant".to_string(),
                            content: "Applied proposed file patches to disk successfully.".to_string(),
                        });
                        uneven_core::application::ContextCompactorService::auto_compact(&mut history);

                        0
                } else if is_approval {
                    let last_assistant_msg = history.iter().rev().find(|t| t.role == "assistant" && !t.content.trim().is_empty());
                    if let Some(turn) = last_assistant_msg {
                        println!(
                            "{} {}",
                            truecolor("  › ", 100, 200, 255),
                            truecolor("Plan approved. Generating and applying file actions…", 107, 122, 150)
                        );
                        let execution_prompt = format!(
                            "Execute the following approved implementation plan by generating the concrete file patches:\n\n{}",
                            turn.content
                        );
                        match think::run(execution_prompt, intent.paths.clone(), None, None).await {
                            Ok(answer) => {
                                if !answer.is_empty() {
                                    if let Some(file_obj) = patch_engine::file_object(&answer, &session_id)
                                        && !file_obj.patch.is_empty() {
                                            println!(
                                                "{} {}",
                                                truecolor("  › ", 168, 200, 255),
                                                truecolor("Applying file actions to disk:", 107, 122, 150)
                                            );
                                            patch_engine::file_action(&root, &file_obj);
                                            println!();
                                    }
                                    history.push(BrainTurn {
                                        role: "assistant".to_string(),
                                        content: answer,
                                    });
                                    uneven_core::application::ContextCompactorService::auto_compact(&mut history);
                                }
                                0
                            }
                            Err(e) => {
                                println!("{}", style(format!("  Failed to execute approved plan: {}", crate::errors::sanitize_error_msg(&e.to_string()))).red());
                                1
                            }
                        }
                    } else {
                        println!("\n  {}\n", console::style("No pending patch or proposal found to apply.").yellow());
                        0
                    }
                } else if !task.is_empty() {
                    // 2. If no approved proposal was found in history, invoke the think engine first
                    // to inspect context, perform the 17-point self-critique, and present the structured proposal.
                    println!(
                        "{} {}",
                        truecolor("  › ", 100, 200, 255),
                        truecolor("Running pre-flight reasoning & architectural critique before applying changes...", 107, 122, 150)
                    );
                    match think::run(task, intent.paths, None, None).await {
                        Ok(answer) => {
                            if !answer.is_empty() {
                                history.push(BrainTurn {
                                    role: "assistant".to_string(),
                                    content: answer,
                                });
                                uneven_core::application::ContextCompactorService::auto_compact(&mut history);
                            }
                            0
                        }
                        Err(e) => {
                            let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                            println!(
                                "{}",
                                style(format!("  Failed to reason/plan: {}", sanitized)).red()
                            );
                            1
                        }
                    }
                } else {
                    1
                }
            }
            ShellAction::RunCmd => {
                let cmd_to_run = match intent.question {
                    Some(ref q) => q.clone(),
                    None => "".to_string(),
                };
                if cmd_to_run.is_empty() {
                    println!("\n{}", style("  No command suggested to run.").yellow());
                    0
                } else {
                    if let Err(err) = safety.validate_command(&cmd_to_run) {
                        println!("\n{} {}", truecolor_bold("⚠️ [SECURITY POLICY]:", 255, 107, 53), err);
                        let feedback = format!(
                            "[System: Command `{}` was BLOCKED by the security policy. Reason: {}]",
                            cmd_to_run, err
                        );
                        auto_prompt = Some(feedback);
                        0
                    } else {
                        let answer = if always_approve_commands {
                            "y".to_string()
                        } else {
                            let ans = loop {
                                print!(
                                    "\n{} Run command: {}? [y/n/a] › ",
                                    truecolor("  ? ", 255, 180, 0),
                                    truecolor(&cmd_to_run, 232, 237, 245)
                                );
                                std::io::stdout().flush().unwrap();

                                let input_confirm = match tokio::task::spawn_blocking(|| {
                                    let mut buf = String::new();
                                    std::io::stdin().read_line(&mut buf).map(|_| buf)
                                }).await {
                                    Ok(Ok(line)) => line.trim().to_lowercase(),
                                    _ => "a".to_string(),
                                };

                                if input_confirm == "y" || input_confirm == "yes" {
                                    break "y".to_string();
                                } else if input_confirm == "n" || input_confirm == "no" {
                                    break "n".to_string();
                                } else if input_confirm == "a" || input_confirm == "abort" || input_confirm == "all" || input_confirm == "always" {
                                    break "a".to_string();
                                }
                            };
                            if ans == "a" {
                                always_approve_commands = true;
                                "y".to_string()
                            } else {
                                ans
                            }
                        };

                        if answer == "y" {
                            println!("\n{}", truecolor(&format!("  ↳ Executing: {}", cmd_to_run), 107, 122, 150));
                            println!("{}", truecolor("  ┌─ Output", 107, 122, 150));

                            let (success, output) = execute_terminal_command_streaming(&cmd_to_run).await;

                            println!("{}", truecolor("  └─ End of Output", 107, 122, 150));
                            let status_str = if success { "success" } else { "failure" };
                            println!("  ↳ Command finished with {}.", status_str);
                            println!();

                            log_command_execution(&root, &config, &cmd_to_run, &message, status_str);

                            let feedback = format!(
                                "[System: Command `{}` finished with status: {}. Output:\n{}]",
                                cmd_to_run, status_str, output
                            );
                            auto_prompt = Some(feedback);
                            0
                        } else if answer == "n" {
                            println!("\n  ↳ Command execution skipped by user.\n");

                            log_command_execution(&root, &config, &cmd_to_run, &message, "skipped");

                            let feedback = format!(
                                "[System: User declined execution of command `{}`.]",
                                cmd_to_run
                            );
                            auto_prompt = Some(feedback);
                            0
                        } else {
                            println!("\n  ↳ Execution aborted.\n");
                            0
                        }
                    }
                }
            }
            ShellAction::MultiAgents => {
                let tasks = match intent.tasks {
                    Some(ref t) => t.clone(),
                    None => Vec::new(),
                };
                let cached_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("multi_agent_tasks_interactive.json");
                let has_cached = cached_path.exists();

                if !tasks.is_empty() || has_cached {
                    let config_brain_max_tokens = config.brain.max_tokens;
                    match crate::commands::multi_agents::run(&tasks, config_brain_max_tokens, &config, "interactive").await {
                        Ok(reports) => {
                            let mut report_msg = String::new();
                            report_msg.push_str("Multi-agent task execution completed.\n\n");
                            for (i, (task_desc, status)) in reports.iter().enumerate() {
                                report_msg.push_str(&format!("Agent {} [Task: {}]:\n{}\n\n", i + 1, task_desc, status));
                            }

                            auto_prompt = Some(format!(
                                "[System: All subagent tasks executed. Results are as follows. Please review them, make any adjustments if needed, and give a unified final explanation to the user:\n\n{}]",
                                report_msg
                            ));
                            0
                        }
                        Err(e) => {
                            let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                            println!("{}", style(format!("  Failed to run multi-agent tasks: {}", sanitized)).red());
                            1
                        }
                    }
                } else {
                    println!("{}", style("  No tasks specified for multi-agents.").yellow());
                    1
                }
            }
            ShellAction::CreatePod => {
                println!(
                    "\n  {} Agent Pods are autonomously orchestrated and instantiated by AI.\n     Manual creation has been disabled. The Think engine instantiates and provisions new pods according to architecture.\n",
                    style("ℹ").yellow()
                );

                let pods = uneven_core::domain::pod::AgentPod::load_project_pods(&root);
                println!("  {}", style("Registered Project Pods:").bold());
                for p in &pods {
                    println!(
                        "  {} {} {} [Prime: {}]\n     Scope: {}",
                        p.category.icon(),
                        style(&p.title).bold(),
                        p.status.badge_class(),
                        style(&p.prime_name).dim(),
                        style(&p.instructions).dim()
                    );
                }
                println!();

                history.push(BrainTurn {
                    role: "assistant".to_string(),
                    content: "Agent Pods are autonomously orchestrated and instantiated by AI. Manual creation has been disabled.".to_string(),
                });
                0
            }
            ShellAction::DispatchPod => {
                let raw_spec = intent.task.clone().unwrap_or_else(|| trimmed.to_string());
                let parts: Vec<&str> = raw_spec.split('|').map(|s| s.trim()).collect();
                let target = parts.first().copied().unwrap_or("database");
                let mission = parts.get(1).copied().unwrap_or(&raw_spec);

                println!(
                    "\n  {} Dispatching macro mission to Pod Cluster: {}\n     Mission: {}\n",
                    ui::green("⚡"),
                    style(target).bold(),
                    style(mission).dim()
                );

                let sess_id = interactive_session_id.clone();
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(
                    ProgressStyle::default_spinner()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                        .template("  {spinner} {msg}")
                        .unwrap(),
                );
                spinner.enable_steady_tick(std::time::Duration::from_millis(100));

                let spinner_clone = spinner.clone();
                let on_phase = move |phase: uneven_core::application::ThoughtPhase| match phase {
                    uneven_core::application::ThoughtPhase::ResolvingContext => {
                        spinner_clone.set_message("Context");
                    }
                    uneven_core::application::ThoughtPhase::Drafting => {
                        spinner_clone.set_message("Drafting");
                    }
                    uneven_core::application::ThoughtPhase::RubberDuckingDraft => {
                        spinner_clone.set_message("Reasoning");
                    }
                    uneven_core::application::ThoughtPhase::Auditing => {
                        spinner_clone.set_message("Auditing");
                    }
                    uneven_core::application::ThoughtPhase::VerifyingIntegrity => {
                        spinner_clone.set_message("Verifying");
                    }
                    uneven_core::application::ThoughtPhase::RubberDuckingSimulation => {
                        spinner_clone.set_message("Simulating");
                    }
                    uneven_core::application::ThoughtPhase::Synthesizing => {
                        spinner_clone.set_message("Synthesizing");
                    }
                    uneven_core::application::ThoughtPhase::Completed => {
                        spinner_clone.set_message("Done");
                    }
                };

                let outcome_res = uneven_core::application::dispatch_pod_mission(
                    target,
                    mission,
                    &config,
                    &root,
                    &sess_id,
                    Some(on_phase),
                ).await;

                spinner.finish_and_clear();

                match outcome_res {
                    Ok(outcome) => {
                        let cleaned = uneven_core::infrastructure::utils::markdown_cleaner::clean_markdown_for_display(&outcome.summary_message);
                        let formatted = ui::TerminalMarkdownFormatter::format(&cleaned);
                        println!("\n{}\n", formatted);

                        history.push(BrainTurn {
                            role: "assistant".to_string(),
                            content: outcome.summary_message,
                        });
                        if let Some(fo) = outcome.file_object.filter(|fo| !fo.patch.is_empty()) {
                            println!(
                                "  {} Staged patch mutations ready ({} file(s)). Applying to disk...",
                                ui::green("✓"),
                                fo.patch.len()
                            );
                            patch_engine::file_action(&root, &fo);
                        }
                    }
                    Err(e) => {
                        println!(
                            "\n  {} Failed to execute pod mission: {}\n",
                            style("✗").red(),
                            e
                        );
                    }
                }
                0
            }
            ShellAction::Unknown => {
                println!(
                    "\n{} {}\n{}{}",
                    truecolor("  Not sure what you meant. ", 255, 235, 59),
                    truecolor("I can help with:", 107, 122, 150),
                    truecolor("  index · scan · watch · explain <file> · docs <file>\n", 107, 122, 150),
                    truecolor("  questions about your project · log · undo · help · exit\n", 107, 122, 150)
                );
                1
            }
        };

        if code == 0
            && let Some(hint) = get_hint(&intent.action) {
                println!();
                println!("{} {}", truecolor("  › ", 0, 255, 140), truecolor(hint, 107, 122, 150));
                println!();
            }

    }

    // Restore terminal UI
    restore_terminal_ui();
    println!("{}", truecolor("\n  Goodbye!\n", 107, 122, 150));
    {
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    std::process::exit(0);
}



#[cfg(test)]
mod tests;
