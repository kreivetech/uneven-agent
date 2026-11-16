use anyhow::Result;
use std::fs;
use std::io::{self, Write};
use console::style;

pub async fn run(yes: bool) -> Result<()> {
    let global_dir = uneven_core::infrastructure::utils::paths::global_uneven_dir();

    if !yes {
        print!(
            "Are you sure you want to uninstall Uneven Agent? This will delete all global configurations, models, logs, and caches at {}. [y/N]: ",
            style(global_dir.to_string_lossy()).cyan()
        );
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            if trimmed != "y" && trimmed != "yes" {
                println!("  ✗ Uninstallation cancelled.");
                return Ok(());
            }
        } else {
            println!("  ✗ Uninstallation cancelled.");
            return Ok(());
        }
    }

    println!("{}", style("Uninstalling Uneven Agent...").yellow());

    // Remove global directory
    if global_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&global_dir) {
            println!("  ✗ Failed to delete global directory {}: {}", global_dir.display(), e);
        } else {
            println!("  ✓ Deleted global directory: {}", global_dir.display());
        }
    } else {
        println!("  ✓ Global directory was already clean.");
    }

    // Clean shell profiles
    let home = uneven_core::infrastructure::utils::paths::home_dir();
    let profiles = [".zshrc", ".bashrc", ".profile"];
    for profile in &profiles {
        let path = home.join(profile);
        if path.exists()
            && let Ok(content) = fs::read_to_string(&path) {
                let mut lines: Vec<String> = Vec::new();
                let mut changed = false;
                for line in content.lines() {
                    if line.contains(".uneven/bin") {
                        changed = true;
                    } else {
                        lines.push(line.to_string());
                    }
                }
                if changed {
                    // Re-write profile without .uneven/bin lines
                    let new_content = lines.join("\n");
                    let final_content = if new_content.is_empty() {
                        String::new()
                    } else {
                        new_content + "\n"
                    };
                    if let Err(e) = fs::write(&path, final_content) {
                        println!("  ✗ Failed to clean {}: {}", profile, e);
                    } else {
                        println!("  ✓ Removed Uneven PATH export from {}", profile);
                    }
                }
            }
    }

    println!("\n  {} Uneven Agent has been uninstalled successfully.", style("✓").green().bold());
    Ok(())
}
