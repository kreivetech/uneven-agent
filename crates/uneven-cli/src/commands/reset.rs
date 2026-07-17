use anyhow::Result;
use std::fs;
use chrono::Utc;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use uneven_core::infrastructure::config::loader;

pub async fn run() -> Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")?,
    );
    spinner.set_message("Resetting Uneven...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let root = loader::find_project_root(None);
    let uneven_dir = root.join(".uneven");

    // 1. Emergency release lock (delete uneven.lock)
    let lock_path = uneven_dir.join("uneven.lock");
    if lock_path.exists() {
        let _ = fs::remove_file(lock_path);
    }

    // 2. Clear incremental index state
    let state_path = uneven_dir.join("index-state.json");
    if state_path.exists() {
        let _ = fs::remove_file(state_path);
    }

    // 3. Clear embeddings cache
    let cache_dir = uneven_dir.join("cache");
    if cache_dir.exists() {
        spinner.set_message("Clearing embeddings cache...");
        let _ = fs::remove_dir_all(cache_dir);
    }

    // 4. Clear vector store
    spinner.set_message("Clearing vector store...");
    let vectors_json = uneven_dir.join("vectors.json");
    if vectors_json.exists() {
        let _ = fs::remove_file(vectors_json);
    }
    let vectors_usearch = uneven_dir.join("vectors.usearch");
    if vectors_usearch.exists() {
        let _ = fs::remove_file(vectors_usearch);
    }

    // 5. Reset log file
    let log_path = uneven_dir.join("log.md");
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let header = format!("# Uneven Log — {}\n\n", today);
    let _ = fs::write(log_path, header);

    spinner.finish_and_clear();
    println!("{} Uneven reset successfully", style("✓").green().bold());
    println!("{}", style("\nNext step: uneven index\n").dim());

    Ok(())
}
