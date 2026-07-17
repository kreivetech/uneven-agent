use anyhow::{anyhow, Result};
use console::style;
use std::fs;
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};

use uneven_core::domain::session::{DiffStatus, PendingDiff};
use uneven_core::infrastructure::session::SessionManager;
use uneven_core::infrastructure::snapshots::SnapshotManager;
use uneven_core::infrastructure::config::loader;

pub async fn run(apply: Option<String>, reject: Option<String>, clear: bool) -> Result<()> {
    let root = loader::find_project_root(None);
    let manager = SessionManager::new(&root);

    if let Some(prefix) = apply {
        let diffs = manager.get_pending_diffs();
        let diff = diffs.iter().find(|d| d.id.starts_with(&prefix) && d.status == DiffStatus::Pending)
            .ok_or_else(|| anyhow!("No pending diff matching \"{}\"", prefix))?;

        println!("Applying diff {}...", &diff.id[..8]);
        apply_diff(&root, &manager, diff).await?;
        println!("{} Applied diff {} → {}", style("✓").green().bold(), &diff.id[..8], diff.file);
        return Ok(());
    }

    if let Some(prefix) = reject {
        let diffs = manager.get_pending_diffs();
        let diff = diffs.iter().find(|d| d.id.starts_with(&prefix) && d.status == DiffStatus::Pending)
            .ok_or_else(|| anyhow!("No pending diff matching \"{}\"", prefix))?;

        manager.resolve_diff(&diff.id, DiffStatus::Rejected)?;
        println!("{} Rejected diff {} ({}:{})", style("✗").yellow().bold(), &diff.id[..8], diff.file, diff.error_line);
        return Ok(());
    }

    if clear {
        manager.clear_resolved_diffs()?;
        println!("{}", style("Resolved diffs cleared.").dim());
        return Ok(());
    }

    let diffs = manager.get_pending_diffs();
    if diffs.is_empty() {
        println!("\n{}", style("No pending diffs.").yellow());
        println!("{}", style("  Pending diffs appear when Uneven runs with autoFix: false").dim());
        println!("{}", style("  and detects an error it can fix.\n").dim());
        return Ok(());
    }

    let pending_count = diffs.iter().filter(|d| d.status == DiffStatus::Pending).count();
    let resolved_count = diffs.len() - pending_count;

    println!("\n{}", style(format!("Pending diffs ({} pending, {} resolved)", pending_count, resolved_count)).bold());
    println!("{}", style("─".repeat(70)).dim());

    for diff in &diffs {
        let id_str = style(&diff.id[..8]).cyan();
        let status_str = match diff.status {
            DiffStatus::Pending => style("pending ").yellow(),
            DiffStatus::Approved => style("approved").blue(),
            DiffStatus::Applied => style("applied ").green(),
            DiffStatus::Rejected => style("rejected").red(),
        };
        let date_str = style(&diff.created_at).dim();
        let pct = (diff.confidence * 100.0).round() as u32;

        println!("  {}  {}  {}", id_str, status_str, date_str);
        println!("    {}   {}:{}", style("File:").dim(), diff.file, diff.error_line);
        println!("    {}  {} — {}", style("Error:").dim(), diff.error_code, diff.error_message);
        println!("    {}    {}", style("Fix:").dim(), diff.suggested_fix);
        println!("    {}   {}%", style("Conf:").dim(), pct);

        if !diff.after_code.is_empty() {
            println!("{}", style("    Proposed:").dim());
            let lines: Vec<&str> = diff.after_code.lines().take(5).collect();
            for line in lines {
                println!("      {}", style(line).color256(250));
            }
            if diff.after_code.lines().count() > 5 {
                println!("      ...");
            }
        }
        println!();
    }

    if pending_count > 0 {
        println!("{}", style("Apply:  uneven diff --apply <id>").dim());
        println!("{}", style("Reject: uneven diff --reject <id>").dim());
        println!("{}", style("Clear:  uneven diff --clear").dim());
    }
    println!();

    Ok(())
}

pub async fn apply_diff(root: &Path, manager: &SessionManager, diff: &PendingDiff) -> Result<()> {
    let abs_path = if Path::new(&diff.file).is_absolute() {
        PathBuf::from(&diff.file)
    } else {
        root.join(&diff.file)
    };

    if !abs_path.exists() {
        return Err(anyhow!("File not found: {}", abs_path.display()));
    }

    let content = fs::read_to_string(&abs_path)?;

    // Checksum guard
    if !diff.expected_checksum.is_empty() {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let current = format!("{:x}", hasher.finalize());
        if current != diff.expected_checksum {
            return Err(anyhow!(
                "File was modified after the diff was created — cannot safely apply.\n\
                 Use 'uneven diff --reject {}' to discard it.",
                &diff.id[..8]
            ));
        }
    }

    if !diff.before_code.is_empty() && !content.contains(&diff.before_code) {
        return Err(anyhow!(
            "Could not locate the original code block in {}.\n\
             The file may have been edited. Diff ID: {}",
            diff.file,
            &diff.id[..8]
        ));
    }

    let snapshots = SnapshotManager::new(root)?;
    snapshots.take(&diff.file, &format!("pending diff {} — {}", &diff.id[..8], diff.error_code))?;

    let updated = if !diff.before_code.is_empty() {
        content.replace(&diff.before_code, &diff.after_code)
    } else {
        content
    };

    fs::write(&abs_path, updated)?;
    manager.resolve_diff(&diff.id, DiffStatus::Applied)?;

    Ok(())
}
