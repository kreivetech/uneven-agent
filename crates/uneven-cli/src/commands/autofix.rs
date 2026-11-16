use anyhow::Result;
use console::style;
use std::fs;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

use uneven_core::domain::event::EventType;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::watcher::WatcherSession;
use uneven_core::infrastructure::utils::process_lock::is_process_alive;
use crate::commands::license;

pub async fn run(off: bool, provider: Option<String>, model: Option<String>) -> Result<()> {
    license::require_pro()?;

    let root = loader::find_project_root(None);
    let pid_file = uneven_core::infrastructure::utils::paths::project_pid_file(&root, "autofix");
    let tokens_file = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("autofix_tokens.txt");

    if off {
        if pid_file.exists() {
            let pid_str = fs::read_to_string(&pid_file)?
                .trim()
                .to_string();
            
            if let Ok(pid) = pid_str.parse::<u32>() {
                // Terminate process
                #[cfg(unix)]
                {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGINT);
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/PID", &pid_str, "/F"])
                        .status();
                }

                // Wait up to 2 seconds for tokens file
                let mut tokens = "0".to_string();
                for _ in 0..20 {
                    sleep(Duration::from_millis(100)).await;
                    if tokens_file.exists()
                        && let Ok(content) = fs::read_to_string(&tokens_file) {
                            tokens = content.trim().to_string();
                            break;
                        }
                }

                println!(
                    "{} Proactive autofix service (PID {}) stopped.",
                    style("✓").green().bold(),
                    pid
                );

                if let Ok(t) = tokens.parse::<u32>()
                    && t > 0 {
                        println!("{}", style(format!("  [Total tokens used by autofix: {}]", t)).dim());
                    }

                let _ = fs::remove_file(&tokens_file);
            }
            let _ = fs::remove_file(&pid_file);
        } else {
            println!("{}", style("⚠ No active autofix service found.").yellow());
        }
        return Ok(());
    }

    // Check if already running
    if pid_file.exists()
        && let Ok(existing_pid_str) = fs::read_to_string(&pid_file)
            && let Ok(pid) = existing_pid_str.trim().parse::<u32>() {
                if pid != std::process::id() && is_process_alive(pid) {
                    println!(
                        "{} Proactive autofix service is already running (PID {}).",
                        style("⚠").yellow().bold(),
                        pid
                    );
                    println!("{}", style("  To stop it, run: uneven autofix --off").dim());
                    return Ok(());
                } else if pid != std::process::id() {
                    // Stale pid file
                    let _ = fs::remove_file(&pid_file);
                }
            }

    let mut config = loader::load_config(&root)?;
    if let Some(p) = provider {
        config.brain.provider = p;
    }
    if let Some(m) = model {
        config.brain.model = m;
    }

    // Ensure proactive watch is active
    if config.proactive.is_none() {
        config.proactive = Some(uneven_core::domain::ProactiveConfig {
            enabled: Some(true),
            auto_fix: Some(true),
            confirm_before_fix: Some(false),
            ..Default::default()
        });
    } else if let Some(ref mut p) = config.proactive {
        p.enabled = Some(true);
    }

    let confirm_before_fix = config.proactive.as_ref()
        .and_then(|p| p.confirm_before_fix)
        .unwrap_or(false)
        || config.watch.as_ref()
            .and_then(|w| w.confirm_before_fix)
            .unwrap_or(false);

    let is_background_child = std::env::var("UNEVEN_AUTOFIX_BACKGROUND").unwrap_or_default() == "true";

    if !confirm_before_fix && !is_background_child {
        // Parent process - daemonize!
        println!(
            "{} Proactive autofix service active  brain: {}/{}",
            style("✓").green().bold(),
            config.brain.provider,
            config.brain.model
        );
        let obs_dirs = config.proactive.as_ref()
            .and_then(|p| p.dirs.clone())
            .unwrap_or_else(|| vec!["project root".to_string()]);
        println!("{}", style(format!("  Observing: {}", obs_dirs.join(", "))).dim());
        println!("{}", style("  Press Ctrl+C to stop or run \"uneven autofix --off\"\n").dim());

        let exe = std::env::current_exe()?;
        let args: Vec<String> = std::env::args().collect();

        let mut cmd = std::process::Command::new(exe);
        if args.len() > 1 {
            cmd.args(&args[1..]);
        }
        let logs_dir = root.join(".uneven/logs");
        fs::create_dir_all(&logs_dir)?;
        let log_path = logs_dir.join("autofix.log");
        let log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        cmd.env("UNEVEN_AUTOFIX_BACKGROUND", "true");
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(log_file.try_clone()?);
        cmd.stderr(log_file);

        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }

        let mut child = cmd.spawn()?;
        let child_pid = child.id();

        // Write PID file for the child
        if let Some(parent) = pid_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&pid_file, child_pid.to_string())?;

        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }

        // Wait a bit to see if the child exits early (fails to start)
        sleep(Duration::from_millis(500)).await;

        if let Ok(Some(status)) = child.try_wait() {
            // Child exited early! It failed to start.
            let err_msg = fs::read_to_string(&log_path).unwrap_or_default();
            let display_err = if err_msg.trim().is_empty() {
                format!("Process exited early with status: {}", status)
            } else {
                err_msg.trim().to_string()
            };
            eprintln!(
                "  {} Proactive autofix service failed to start:\n  {}",
                style("✗").red().bold(),
                display_err
            );
            let _ = fs::remove_file(&pid_file);
            {
                use std::io::Write;
                let _ = std::io::stdout().flush();
                let _ = std::io::stderr().flush();
            }
            std::process::exit(1);
        }

        // Exit parent successfully
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let _ = std::io::stderr().flush();
        }
        std::process::exit(0);
    }

    // Child process or foreground interactive mode
    if let Some(parent) = pid_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&pid_file, std::process::id().to_string())?;

    if let Some(parent) = tokens_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Set watch configuration to empty terminals (proactive-only mode)
    if config.watch.is_none() {
        config.watch = Some(uneven_core::domain::WatchConfig {
            terminals: Some(vec![]),
            auto_fix: Some(true),
            confirm_before_fix: Some(confirm_before_fix),
            ..Default::default()
        });
    } else if let Some(ref mut w) = config.watch {
        w.terminals = Some(vec![]);
        w.auto_fix = Some(true);
        w.confirm_before_fix = Some(confirm_before_fix);
    }

    let mut session = WatcherSession::new(config.clone(), root.clone(), tx);
    session.start().await?;

    let mut total_tokens = 0;
    let root_clone = root.clone();

    let event_loop = async {
        while let Some(event) = rx.recv().await {
            match event.r#type {
                EventType::BrainReady if !is_background_child => {
                    println!(
                        "{} Proactive autofix service active  brain: {}/{}",
                        style("✓").green().bold(),
                        config.brain.provider,
                        config.brain.model
                    );
                    let obs_dirs = config.proactive.as_ref()
                        .and_then(|p| p.dirs.clone())
                        .unwrap_or_else(|| vec!["project root".to_string()]);
                    println!("{}", style(format!("  Observing: {}", obs_dirs.join(", "))).dim());
                    println!("{}", style("  Press Ctrl+C to stop or run \"uneven autofix --off\"\n").dim());
                }
                EventType::FileChanged if !is_background_child && !confirm_before_fix => {
                    if let Some(path) = event.data.get("path").and_then(|p| p.as_str()) {
                        let name = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path);
                        println!("  ~ {} saved", style(name).dim());
                    }
                }
                EventType::FixApplied => {
                    total_tokens += 500; // Mock tokens used per fix
                    if !is_background_child {
                        let suggestion = event.data.get("suggestion");
                        let file = suggestion.and_then(|s| s.get("file")).and_then(|f| f.as_str()).unwrap_or("");
                        let name = std::path::Path::new(file)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(file);
                        let explanation = suggestion.and_then(|s| s.get("explanation")).and_then(|e| e.as_str()).unwrap_or("");
                        println!(
                            "  {} Auto-fixed {}: {}",
                            style("✓").color256(48).bold(),
                            name,
                            explanation
                        );
                    }
                }
                EventType::FixPending if !is_background_child => {
                    let id = event.data.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    let file = event.data.get("file").and_then(|f| f.as_str()).unwrap_or("");
                    let line = event.data.get("line").and_then(|l| l.as_u64()).unwrap_or(0);
                    let message = event.data.get("message").and_then(|m| m.as_str()).unwrap_or("");
                    let suggested_fix = event.data.get("suggestedFix").and_then(|s| s.as_str()).unwrap_or("");
                    let explanation = event.data.get("explanation").and_then(|e| e.as_str()).unwrap_or("");
                    let confidence = event.data.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0);
                    let after_code = event.data.get("afterCode").and_then(|a| a.as_str()).unwrap_or("");

                    let name = std::path::Path::new(file)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(file);

                    println!();
                    println!("{}", style("  ⚡ UNEVEN AUTO-FIX — Authorization Required").bold().yellow());
                    println!("{}", style("  ".to_string() + &"─".repeat(50)).dim());
                    println!("  File       : {}:{}", file, line);
                    println!("  Error      : {}", message);
                    println!("  Fix        : {}", suggested_fix);
                    println!("  Confidence : {:.0}%", confidence * 100.0);

                    if !after_code.is_empty() {
                        println!("{}", style("\n  Proposed code:").dim());
                        let lines: Vec<&str> = after_code.lines().take(10).collect();
                        for l in lines {
                            println!("    {}", style(l).color256(250));
                        }
                    }
                    println!();

                    print!("  [A]pply fix / [S]kip > ");
                    use std::io::Write;
                    std::io::stdout().flush().ok();

                    let mut input = String::new();
                    let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
                    if tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut input).await.is_ok() {
                        let choice = input.trim().to_uppercase();
                        if choice == "A" || choice == "APPLY" || choice == "Y" || choice == "YES" {
                            let manager = uneven_core::infrastructure::session::SessionManager::new(&root_clone);
                            let diffs = manager.get_pending_diffs();
                            if let Some(diff) = diffs.iter().find(|d| d.id == id) {
                                total_tokens += 500;
                                if let Err(e) = crate::commands::diff::apply_diff(&root_clone, &manager, diff).await {
                                    println!("{}", style(format!("  ✗ Failed to apply fix: {}", e)).red());
                                } else {
                                    println!(
                                        "  {} Applied fix {}: {}",
                                        style("✓").color256(48).bold(),
                                        name,
                                        explanation
                                    );
                                }
                            }
                        } else {
                            let manager = uneven_core::infrastructure::session::SessionManager::new(&root_clone);
                            let _ = manager.resolve_diff(id, uneven_core::domain::session::DiffStatus::Rejected);
                            println!("{}", style("  ✗ Fix skipped.").dim());
                        }
                    }
                }
                _ => {}
            }
        }
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = event_loop => {}
    }

    // Save tokens used before exit
    let _ = fs::write(&tokens_file, total_tokens.to_string());

    if !is_background_child && total_tokens > 0 {
        println!("{}", style(format!("\n  [Total tokens used in autofix session: {}]", total_tokens)).dim());
    }

    session.stop().await?;
    let _ = fs::remove_file(&pid_file);

    Ok(())
}
