use anyhow::{anyhow, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::snapshots::SnapshotManager;

pub async fn run(file: Option<String>, id: Option<String>, list: bool) -> Result<()> {
    let root = loader::find_project_root(None);
    let manager = SnapshotManager::new(&root)?;

    if list {
        let all = manager.list()?;
        
        let filtered = if let Some(ref f) = file {
            let abs_path = if Path::new(f).is_absolute() {
                std::path::PathBuf::from(f)
            } else {
                std::env::current_dir()?.join(f)
            };
            let target_path = fs::canonicalize(&abs_path).unwrap_or(abs_path);
            let target_str = target_path.to_string_lossy().to_string();
            
            all.into_iter()
                .filter(|s| s.original_path == target_str)
                .collect::<Vec<_>>()
        } else {
            all
        };

        if filtered.is_empty() {
            println!("{}", style("No snapshots found.").yellow());
            if let Some(ref f) = file {
                println!("{}", style(format!("  File: {}", f)).dim());
            }
            return Ok(());
        }

        println!();
        println!("{}", style(format!("Snapshots ({})", filtered.len())).bold());
        println!("{}", style("─".repeat(60)).dim());

        for snap in &filtered {
            let date = snap.created_at.to_rfc3339();
            let size_kb = (snap.size as f64 / 1024.0).format_as_kb();
            println!(
                "  {}  {}  {}",
                style(&snap.id[..8.min(snap.id.len())]).cyan(),
                style(&date).dim(),
                style(format!("{}KB", size_kb)).dim()
            );
            println!("    {}   {}", style("File:").dim(), snap.original_path);
            println!("    {} {}", style("Reason:").dim(), snap.reason);
            println!();
        }

        let total_size: u64 = filtered.iter().map(|s| s.size).sum();
        let usage_mb = total_size as f64 / 1024.0 / 1024.0;
        println!(
            "{}",
            style(format!("Total: {} snapshots, {:.2} MB", filtered.len(), usage_mb)).dim()
        );
        println!();
        println!("{}", style("Restore with: uneven restore --id <id>").dim());
        println!("{}", style("Restore latest for a file: uneven restore --file <path>").dim());
        return Ok(());
    }

    if let Some(snapshot_id) = id {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")?,
        );
        spinner.set_message(format!("Restoring snapshot {}...", &snapshot_id[..8.min(snapshot_id.len())]));
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        match manager.restore(&snapshot_id) {
            Ok(snapshot) => {
                spinner.finish_and_clear();
                println!("{} {}: {}", style("✓").green().bold(), style("Restored").green(), snapshot.original_path);
                println!(
                    "{}",
                    style(format!("  From snapshot taken at {}", snapshot.created_at.to_rfc3339())).dim()
                );
                println!("{}", style(format!("  Reason: {}", snapshot.reason)).dim());
            }
            Err(err) => {
                spinner.finish_and_clear();
                return Err(anyhow!("Restore failed: {}", err));
            }
        }
        return Ok(());
    }

    if let Some(f) = file {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")?,
        );
        spinner.set_message(format!("Restoring latest snapshot for {}...", f));
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        let abs_path = if Path::new(&f).is_absolute() {
            std::path::PathBuf::from(&f)
        } else {
            std::env::current_dir()?.join(&f)
        };
        let target_path = fs::canonicalize(&abs_path).unwrap_or(abs_path);
        let target_str = target_path.to_string_lossy().to_string();

        let all = manager.list()?;
        let latest = all.into_iter().find(|s| s.original_path == target_str);

        match latest {
            Some(snapshot) => {
                match manager.restore(&snapshot.id) {
                    Ok(snapshot) => {
                        spinner.finish_and_clear();
                        println!("{} {}: {}", style("✓").green().bold(), style("Restored").green(), snapshot.original_path);
                        println!(
                            "{}",
                            style(format!("  Snapshot taken at {}", snapshot.created_at.to_rfc3339())).dim()
                        );
                        println!("{}", style(format!("  Reason: {}", snapshot.reason)).dim());
                        println!();
                        println!(
                            "{}",
                            style(format!("To see all snapshots for this file: uneven restore --file {} --list", f)).dim()
                        );
                    }
                    Err(err) => {
                        spinner.finish_and_clear();
                        return Err(anyhow!("Restore failed: {}", err));
                    }
                }
            }
            None => {
                spinner.finish_and_clear();
                return Err(anyhow!("No snapshots found for: {}", f));
            }
        }
        return Ok(());
    }

    println!();
    println!("{}", style("uneven restore — Revert files to pre-fix state").bold());
    println!();
    println!("Usage:");
    println!("  {}              List all snapshots", style("uneven restore --list").cyan());
    println!("  {} List snapshots for a file", style("uneven restore --list --file <path>").cyan());
    println!("  {}       Restore latest snapshot for file", style("uneven restore --file <path>").cyan());
    println!("  {}           Restore specific snapshot by ID", style("uneven restore --id <id>").cyan());
    println!();

    Ok(())
}

trait FormatKb {
    fn format_as_kb(self) -> String;
}

impl FormatKb for f64 {
    fn format_as_kb(self) -> String {
        format!("{:.1}", self)
    }
}
