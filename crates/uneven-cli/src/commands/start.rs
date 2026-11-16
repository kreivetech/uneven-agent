use anyhow::Result;
use console::style;
use tokio::sync::mpsc;

use uneven_core::domain::event::EventType;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::watcher::WatcherSession;

pub async fn run() -> Result<()> {
    let root = loader::find_project_root(None);
    let mut config = loader::load_config(&root)?;

    // Set watch configuration to empty terminals (proactive-only mode)
    if config.watch.is_none() {
        config.watch = Some(uneven_core::domain::WatchConfig {
            terminals: Some(vec![]),
            auto_fix: Some(true),
            confirm_before_fix: Some(false),
            ..Default::default()
        });
    } else if let Some(ref mut w) = config.watch {
        w.terminals = Some(vec![]);
        w.auto_fix = Some(true);
        w.confirm_before_fix = Some(false);
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut session = WatcherSession::new(config.clone(), root.clone(), tx);
    session.start().await?;

    println!("\n  Uneven is watching your project…");
    println!("  Press Ctrl+C to stop\n");

    let event_loop = async {
        while let Some(event) = rx.recv().await {
            match event.r#type {
                EventType::BrainReady => {
                    println!(
                        "{} Proactive watch session active  brain: {}/{}",
                        style("✓").green().bold(),
                        config.brain.provider,
                        config.brain.model
                    );
                }
                EventType::FileChanged => {
                    if let Some(path) = event.data.get("path").and_then(|p| p.as_str()) {
                        let name = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path);
                        println!("  ~ {} saved", style(name).dim());
                    }
                }
                EventType::ErrorDetected => {
                    let msg = event.data.get("message").and_then(|m| m.as_str()).unwrap_or("");
                    println!("  {} Error detected: {}", style("✗").red().bold(), style(msg).red());
                }
                EventType::FixSuggested => {
                    let explanation = event.data.get("explanation").and_then(|e| e.as_str()).unwrap_or("");
                    let confidence = event.data.get("confidence")
                        .and_then(|c| c.as_f64())
                        .map(|c| format!("{:.0}%", c * 100.0))
                        .unwrap_or_else(|| "?%".to_string());
                    println!("  ⚡ Fix suggested ({}): {}", style(confidence).cyan(), explanation);
                }
                EventType::FixApplied => {
                    let suggestion = event.data.get("suggestion");
                    let explanation = suggestion.and_then(|s| s.get("explanation")).and_then(|e| e.as_str()).unwrap_or("");
                    let confidence = suggestion.and_then(|s| s.get("confidence"))
                        .and_then(|c| c.as_f64())
                        .map(|c| format!("{:.0}%", c * 100.0))
                        .unwrap_or_else(|| "?%".to_string());
                    println!("  {} Fix applied ({})", style("✓").color256(48).bold(), confidence);
                    if !explanation.is_empty() {
                        println!("    {}", style(explanation).dim());
                    }
                }
                EventType::FixReverted => {
                    println!("  {}", style("↩ Fix reverted — tests failed, file restored").yellow());
                }
                _ => {}
            }
        }
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\n  Stopping watch session...");
        }
        res = event_loop => {
            res?;
        }
    }

    session.stop().await?;
    println!("  Goodbye!");

    Ok(())
}
