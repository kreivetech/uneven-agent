use anyhow::Result;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::mpsc;
use tokio::time::Duration;


use std::sync::Arc;
use uneven_core::domain::event::EventType;
use uneven_core::domain::notifications::{NotificationPayload, NotificationType};
use uneven_core::infrastructure::notifications::WebhookNotifier;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::watcher::WatcherSession;
use crate::commands::license;

pub async fn run(
    commands: Vec<String>,
    ignore: Vec<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<()> {
    let root = loader::find_project_root(None);
    let mut config = loader::load_config(&root)?;
    let confirm_before_fix = config.watch.as_ref()
        .and_then(|w| w.confirm_before_fix)
        .unwrap_or(false);

    // Override provider/model if supplied via CLI args
    if let Some(p) = provider {
        config.brain.provider = p;
    }
    if let Some(m) = model {
        config.brain.model = m;
    }

    // Merge CLI ignores with config ignore patterns
    let mut all_ignores = config.watch.as_ref()
        .and_then(|w| w.ignore_patterns.clone())
        .unwrap_or_default();
    for p in ignore {
        if !all_ignores.contains(&p) {
            all_ignores.push(p);
        }
    }
    if let Some(ref mut w) = config.watch {
        w.ignore_patterns = Some(all_ignores);
    }

    // Resolve watch commands
    let resolved_commands = if !commands.is_empty() {
        commands.clone()
    } else if let Some(ref w) = config.watch {
        if let Some(ref terms) = w.terminals {
            terms.clone()
        } else if let Some(ref term) = w.terminal {
            vec![term.clone()]
        } else {
            vec!["npm run dev".to_string()]
        }
    } else {
        vec!["npm run dev".to_string()]
    };

    if let Some(ref mut w) = config.watch {
        w.terminals = Some(resolved_commands.clone());
    }

    let display_label = if resolved_commands.len() > 1 {
        resolved_commands.iter()
            .map(|c| label_of_command(c))
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        resolved_commands[0].clone()
    };

    // Webhook notifications check
    let mut notifier = None;
    if let Some(ref notifications) = config.notifications
        && notifications.webhook_url.is_some() {
            if license::check_team() {
                notifier = Some(Arc::new(WebhookNotifier::new(notifications.clone())));
                if let Some(ref url) = notifications.webhook_url {
                    println!("{}", style(format!("  ◈ Webhook notifications → {}", url)).dim());
                }
            } else {
                println!("{}", style("  ⚠ webhookUrl is configured but requires Uneven Team — notifications disabled").yellow());
            }
        }

    let starting_frames = [
        format!("{} {}", style("⠋").cyan(), style(format!("Starting terminal watcher for: {}.  ", display_label)).dim()),
        format!("{} {}", style("⠙").cyan(), style(format!("Starting terminal watcher for: {}.. ", display_label)).dim()),
        format!("{} {}", style("⠹").cyan(), style(format!("Starting terminal watcher for: {}...", display_label)).dim()),
        format!("{} {}", style("⠸").cyan(), style(format!("Starting terminal watcher for: {}.  ", display_label)).dim()),
        format!("{} {}", style("⠼").cyan(), style(format!("Starting terminal watcher for: {}.. ", display_label)).dim()),
        format!("{} {}", style("⠴").cyan(), style(format!("Starting terminal watcher for: {}...", display_label)).dim()),
        format!("{} {}", style("⠦").cyan(), style(format!("Starting terminal watcher for: {}.  ", display_label)).dim()),
        format!("{} {}", style("⠧").cyan(), style(format!("Starting terminal watcher for: {}.. ", display_label)).dim()),
        format!("{} {}", style("⠇").cyan(), style(format!("Starting terminal watcher for: {}...", display_label)).dim()),
        format!("{} {}", style("⠏").cyan(), style(format!("Starting terminal watcher for: {}.  ", display_label)).dim()),
    ];
    let starting_frames_ref: Vec<&str> = starting_frames.iter().map(|s| s.as_str()).collect();

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&starting_frames_ref)
            .template("{spinner}").unwrap(),
    );
    spinner.enable_steady_tick(Duration::from_millis(80));

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut session = WatcherSession::new(config.clone(), root.clone(), tx);
    session.start().await?;

    let root_clone = root.clone();
    let is_multi = resolved_commands.len() > 1;


    let event_loop = async {
        while let Some(event) = rx.recv().await {
            match event.r#type {
                EventType::BrainReady => {
                    spinner.finish_and_clear();
                    println!(
                        "{} Watching {} process{}: {}",
                        style("✓").green().bold(),
                        resolved_commands.len(),
                        if is_multi { "es" } else { "" },
                        style(&display_label).green()
                    );
                    println!(
                        "{}",
                        style(format!(
                            "  brain: {}/{}",
                            config.brain.provider, config.brain.model
                        ))
                        .dim()
                    );
                    if is_multi {
                        for (i, cmd) in resolved_commands.iter().enumerate() {
                            println!("{}", style(format!("  [{}] {}", i + 1, cmd)).dim());
                        }
                    }
                    println!("{}", style("  Press Ctrl+C to stop\n").dim());
                }
                EventType::Info => {
                    // This could be stdout/stderr from process or generic message
                    if let Some(channel) = event.data.get("channel").and_then(|c| c.as_str()) {
                        if let Some(data) = event.data.get("data").and_then(|d| d.as_str()) {
                            // Forward output to terminal
                            if channel == "stderr" {
                                eprint!("{}", data);
                            } else {
                                print!("{}", data);
                            }
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                            std::io::stderr().flush().ok();
                        }
                    } else if let Some(msg) = event.data.get("message").and_then(|m| m.as_str()) {
                        println!("  {}", style(msg).dim());
                    }
                }
                EventType::ErrorDetected => {
                    let proc = event.data.get("process")
                        .and_then(|p| p.as_str())
                        .map(|s| format!("[{}] ", s))
                        .unwrap_or_default();
                    let file = event.data.get("file").and_then(|f| f.as_str()).unwrap_or("unknown");
                    let line = event.data.get("line").and_then(|l| l.as_u64()).unwrap_or(0);
                    let msg = event.data.get("message").and_then(|m| m.as_str()).unwrap_or("");

                    println!(
                        "\n  {} {}Error detected: {}",
                        style("✗").red().bold(),
                        style(proc).cyan(),
                        style(msg).red()
                    );
                    println!("    at {}:{}", file, line);

                    // Send webhook notification
                    if let Some(ref notifier_instance) = notifier {
                        let payload = NotificationPayload {
                            r#type: Some(NotificationType::Error),
                            error_message: msg.to_string(),
                            file: Some(file.to_string()),
                            line: Some(line as u32),
                            process_label: event.data.get("process").and_then(|p| p.as_str()).map(|s| s.to_string()),
                            fix_applied: None,
                        };
                        let notifier_clone = notifier_instance.clone();
                        tokio::spawn(async move {
                            let _ = notifier_clone.notify(&payload).await;
                        });
                    }
                }
                EventType::FixSuggested => {
                    let proc = event.data.get("process")
                        .and_then(|p| p.as_str())
                        .map(|s| format!("[{}] ", s))
                        .unwrap_or_default();
                    let confidence = event.data.get("confidence")
                        .and_then(|c| c.as_f64())
                        .map(|c| format!("{:.0}%", c * 100.0))
                        .unwrap_or_else(|| "?%".to_string());
                    let explanation = event.data.get("explanation").and_then(|e| e.as_str()).unwrap_or("");

                    println!(
                        "\n  {} {}Fix suggested ({}): {}",
                        style("⚡").color256(111), // soft blue
                        style(proc).cyan(),
                        confidence,
                        explanation
                    );
                }
                EventType::FixApplied => {
                    let suggestion = event.data.get("suggestion");
                    let proc = suggestion.and_then(|s| s.get("process"))
                        .and_then(|p| p.as_str())
                        .map(|s| format!("[{}] ", s))
                        .unwrap_or_default();
                    let confidence = suggestion.and_then(|s| s.get("confidence"))
                        .and_then(|c| c.as_f64())
                        .map(|c| format!("{:.0}%", c * 100.0))
                        .unwrap_or_else(|| "?%".to_string());
                    let explanation = suggestion.and_then(|s| s.get("explanation"))
                        .and_then(|e| e.as_str())
                        .unwrap_or("");

                    println!(
                        "  {} {}Fix applied ({})",
                        style("✓").color256(48).bold(), // bright green
                        style(proc).cyan(),
                        confidence
                    );
                    if !explanation.is_empty() {
                        println!("    {}", style(explanation).dim());
                    }

                    // Send webhook notification
                    if let Some(ref notifier_instance) = notifier {
                        let payload = NotificationPayload {
                            r#type: Some(NotificationType::Fix),
                            error_message: explanation.to_string(),
                            file: suggestion.and_then(|s| s.get("file")).and_then(|f| f.as_str()).map(|s| s.to_string()),
                            line: None, // Line number not directly passed in fix applied event structure
                            process_label: suggestion.and_then(|s| s.get("process")).and_then(|p| p.as_str()).map(|s| s.to_string()),
                            fix_applied: Some(explanation.to_string()),
                        };
                        let notifier_clone = notifier_instance.clone();
                        tokio::spawn(async move {
                            let _ = notifier_clone.notify(&payload).await;
                        });
                    }
                }
                EventType::FixReverted => {
                    println!(
                        "  {}",
                        style("↩ Fix reverted — tests failed, file restored").yellow()
                    );
                }
                EventType::FixPending => {
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
                    println!("{}", style("  ⚡ UNEVEN AGENT AUTO-FIX — Authorization Required").bold().yellow());
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
                EventType::FileChanged if !confirm_before_fix => {
                    if let Some(path) = event.data.get("path").and_then(|p| p.as_str()) {
                        println!("  ~ {}", style(path).dim());
                    }
                }
                EventType::Warning => {
                    if let Some(msg) = event.data.get("message").and_then(|m| m.as_str()) {
                        println!("  {}", style(msg).yellow());
                    }
                }
                _ => {}
            }
        }
        Ok::<(), anyhow::Error>(())
    };

    let is_pro = license::check_pro();
    let timeout_future = async {
        if !is_pro {
            tokio::time::sleep(Duration::from_secs(15 * 60)).await;
            println!(
                "\n  {}\n  Upgrade to Pro for unlimited background monitoring: https://uneven.dev/pricing\n",
                style("[UE-01000005] ⚠ 15-minute Free tier limit for 'watch' reached.").yellow()
            );
        } else {
            std::future::pending::<()>().await;
        }
    };

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            spinner.finish_and_clear();
            println!("\n{}", style("  Stopping watch session...").dim());
        }
        res = event_loop => {
            res?;
        }
        _ = timeout_future => {
            spinner.finish_and_clear();
        }
    }

    session.stop().await?;
    println!("{}", style("  Goodbye!").dim());

    Ok(())
}

fn label_of_command(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() >= 3 && parts[0] == "npm" && parts[1] == "run" {
        return parts[2].to_string();
    }
    if parts.len() >= 3 && parts[0] == "pnpm" && parts[1] == "run" {
        return parts[2].to_string();
    }
    if parts.len() >= 2 && parts[0] == "yarn" && !["add", "install", "remove", "upgrade", "dlx"].contains(&parts[1]) {
        return parts[1].to_string();
    }
    if let Some(last) = parts.last()
        && let Some(name) = std::path::Path::new(last).file_name().and_then(|n| n.to_str()) {
            return name.to_string();
        }
    "process".to_string()
}
