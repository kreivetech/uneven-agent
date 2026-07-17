use anyhow::{anyhow, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::process::Command as SysCommand;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::git;

pub async fn run(commit: Option<String>, list: bool) -> Result<()> {
    let root = loader::find_project_root(None);

    // Validate Git Repository
    let is_git = SysCommand::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&root)
        .output()
        .map(|out| out.status.success() && root.join(".git").exists())
        .unwrap_or(false);

    if !is_git {
        println!("{}", style("  [UE-03000003] ✗ Not a Git repository. uneven undo requires Git.").red());
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        std::process::exit(1);
    }

    let repo_path = root.to_string_lossy();

    if list {
        let commits = git::list_fix_commits(&repo_path, 20)?;

        if commits.is_empty() {
            println!("{}", style("No Uneven fix commits found on current branch.").yellow());
            return Ok(());
        }

        println!();
        println!("{}", style(format!("Uneven fix commits ({})", commits.len())).bold());
        println!("{}", style("─".repeat(60)).dim());

        for c in commits {
            let short_hash = if c.hash.len() > 8 { &c.hash[..8] } else { &c.hash };
            println!("  {}  {}", style(short_hash).cyan(), style(&c.date).dim());
            let first_line = c.message.lines().next().unwrap_or("").trim();
            println!("    {}", first_line);
            println!();
        }

        println!("{}", style("Revert a specific commit: uneven undo --commit <hash>").dim());
        println!("{}", style("Revert last fix: uneven undo").dim());
        return Ok(());
    }

    if let Some(commit_hash) = commit {
        let short_hash = if commit_hash.len() > 8 { &commit_hash[..8] } else { &commit_hash };
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")?,
        );
        spinner.set_message(format!("Reverting commit {}...", short_hash));
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        match git::revert_commit(&repo_path, &commit_hash) {
            Ok(result) => {
                spinner.finish_and_clear();
                let short_reverted = if result.reverted_commit.len() > 8 { &result.reverted_commit[..8] } else { &result.reverted_commit };
                let short_revert = if result.revert_commit.len() > 8 { &result.revert_commit[..8] } else { &result.revert_commit };
                println!("{} {}", style("✓").green().bold(), style(format!("Reverted {}", short_reverted)).green());
                println!("{}", style(format!("  Revert commit: {}", short_revert)).dim());
                println!("{}", style(format!("  Branch: {}", result.branch)).dim());
            }
            Err(err) => {
                spinner.finish_and_clear();
                return Err(anyhow!("Failed to revert commit: {}", err));
            }
        }
        return Ok(());
    }

    // Default: undo last fix
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")?,
    );
    spinner.set_message("Reverting last Uneven fix...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    match git::undo_last_fix(&repo_path) {
        Ok(result) => {
            spinner.finish_and_clear();
            let short_reverted = if result.reverted_commit.len() > 8 { &result.reverted_commit[..8] } else { &result.reverted_commit };
            let short_revert = if result.revert_commit.len() > 8 { &result.revert_commit[..8] } else { &result.revert_commit };
            println!("{} {}", style("✓").green().bold(), style(format!("Reverted fix {}", short_reverted)).green());
            println!("{}", style(format!("  Revert commit: {}", short_revert)).dim());
            println!("{}", style(format!("  Branch: {}", result.branch)).dim());
            println!();
            println!("{}", style("To also restore the file snapshot: uneven-ai restore --list").dim());
        }
        Err(err) => {
            spinner.finish_and_clear();
            return Err(anyhow!("Failed to undo fix: {}", err));
        }
    }

    Ok(())
}
