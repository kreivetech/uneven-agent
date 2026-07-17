use anyhow::Result;
use console::style;
use std::fs;
use uneven_core::infrastructure::config::loader;

pub async fn run() -> Result<()> {
    let root = loader::find_project_root(None);
    let log_paths = [
        root.join(".uneven/log.md"),
        root.join(".uneven/logs/main.md"),
    ];

    let mut found = false;
    for path in &log_paths {
        if path.exists() {
            println!("{}", style("Uneven Log:\n").blue().bold());
            if let Ok(content) = fs::read_to_string(path) {
                print!("{}", content);
                found = true;
                break;
            }
        }
    }

    if !found {
        println!("{}", style("No log file found. Run: uneven init").yellow());
    }

    Ok(())
}
