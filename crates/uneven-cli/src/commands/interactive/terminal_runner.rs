use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use anyhow::Result;
use uneven_core::UnevenConfig;
use uneven_core::application::shell_classifier::{tokenize_shell_input, ShellAction};
use crate::ui::{self, truecolor};

pub static SUBPROCESS_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn parse_analyze_input(input_after_analyze: &str) -> Vec<String> {
    let raw_tokens = tokenize_shell_input(input_after_analyze);
    let mut args = vec!["analyze".to_string()];
    let mut prompt_words = Vec::new();

    let mut i = 0;
    while i < raw_tokens.len() {
        let token = &raw_tokens[i];
        if token == "--db"
            || token == "--report"
            || token == "--theme"
            || token == "--provider"
            || token == "--model"
            || token == "--prompt"
        {
            // Flag with argument
            args.push(token.clone());
            if i + 1 < raw_tokens.len() {
                args.push(raw_tokens[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        } else if token == "--package-exe" || token == "--clear-cache" {
            // Flag without argument
            args.push(token.clone());
            i += 1;
        } else {
            // Word belonging to the prompt
            prompt_words.push(token.clone());
            i += 1;
        }
    }

    if !prompt_words.is_empty() {
        let prompt_str = prompt_words.join(" ");
        args.push("--prompt".to_string());
        args.push(prompt_str);
    }

    args
}

pub async fn spawn_subcommand_silent(args: &[&str]) -> Result<i32> {
    let current_exe = std::env::current_exe()?;
    let mut child = tokio::process::Command::new(current_exe)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let status = child.wait().await?;
    Ok(status.code().unwrap_or(0))
}

pub async fn spawn_subcommand_silent_in_dir(args: &[&str], dir: &std::path::Path) -> Result<i32> {
    let current_exe = std::env::current_exe()?;
    let mut child = tokio::process::Command::new(current_exe)
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let status = child.wait().await?;
    Ok(status.code().unwrap_or(0))
}

pub async fn spawn_subcommand(args: &[&str]) -> Result<i32> {
    SUBPROCESS_RUNNING.store(true, Ordering::Relaxed);
    let current_exe = std::env::current_exe()?;
    let mut child = tokio::process::Command::new(current_exe)
        .args(args)
        .spawn()?;
    let status = child.wait().await?;
    SUBPROCESS_RUNNING.store(false, Ordering::Relaxed);
    Ok(status.code().unwrap_or(0))
}

#[allow(dead_code)]
pub fn format_duration(d: Duration) -> String {
    let secs_f = d.as_secs_f64();
    if secs_f < 1.0 {
        let ms = (secs_f * 1000.0).round() as u64;
        format!("{}ms", ms)
    } else if secs_f < 60.0 {
        let secs = secs_f.round() as u64;
        format!("{}s", secs)
    } else {
        let total_secs = secs_f.round() as u64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if secs == 0 {
            format!("{}m", mins)
        } else {
            format!("{}m {}s", mins, secs)
        }
    }
}

pub fn get_hint(action: &ShellAction) -> Option<&'static str> {
    match action {
        ShellAction::Index => Some("Project indexed. Try \"scan\" or ask anything about your code."),
        ShellAction::Scan => Some("Scan complete. Use \"watch\" to monitor errors in real time."),
        ShellAction::Watch => Some("Watcher stopped. Back to interactive mode."),
        ShellAction::Explain => Some("Want more detail? Ask a question or try \"docs <file>\"."),
        ShellAction::Docs => Some("Tip: use --out docs/<file>.md via CLI to save output to a file."),
        ShellAction::Undo => Some("Fix reverted. The code is back to its previous state."),
        _ => None,
    }
}

pub fn print_welcome(version: &str, brain: &str, is_first_run: bool) {
    let d = |s: &str| truecolor(s, 107, 122, 150);
    println!();
    ui::box_welcome(version, brain, is_first_run);
    println!();
    println!("{}", d("  How can I help you today?"));
    println!();
}

pub fn print_shell_help() {
    let a = |s: &str| truecolor(s, 168, 200, 255);
    let d = |s: &str| truecolor(s, 107, 122, 150);

    let w = |phrase: &str, desc: &str| {
        println!("  {}{}", a(&format!("{:width$}", phrase, width = 36)), d(desc));
    };

    println!();
    println!("{}", d("  What I can do for you:"));
    println!();
    w("\"index my project\"", "Learn your codebase and docs");
    w("\"scan\"", "Check for vulnerabilities and malware");
    w("\"watch\"", "Monitor terminal errors and fix them automatically");
    w("\"explain src/app.ts\"", "Explain a file in plain English");
    w("\"docs src/utils.ts\"", "Generate Markdown API documentation");
    w("\"create a login endpoint\"", "Create or edit files for a given task");
    w("\"<any question>\"", "Ask anything about the project");
    w("\"log\"", "Show Uneven activity history");
    w("\"undo\"", "Revert the last fix that was applied");
    w("\"exit\"", "Leave the interactive shell");
    println!();
    println!("{}", d("  Tip: all CLI subcommands still work directly (uneven watch, etc.)"));
    println!();
}

pub fn restore_terminal_ui() {
    let term = console::Term::stdout();
    let (rows, _) = term.size();
    if rows > 3 {
        print!("\x1b[r");
        print!("\x1b[{};1H\x1b[2K", rows);
        if rows > 5 {
            print!("\x1b[{};1H\x1b[2K", rows - 1);
            print!("\x1b[{};1H\x1b[2K", rows - 2);
        }
        print!("\x1b[{};1H", rows - 3);
        let _ = std::io::stdout().flush();
    }
}

pub fn log_command_execution(
    root: &std::path::Path,
    config: &UnevenConfig,
    cmd: &str,
    summary: &str,
    status: &str,
) {
    let log_path = config
        .log
        .as_ref()
        .and_then(|l| l.path.as_ref())
        .cloned()
        .unwrap_or_else(|| ".uneven/logs/main.md".to_string());
    let log_abs_path = root.join(&log_path);
    if let Some(parent) = log_abs_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if !log_abs_path.exists()
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_abs_path)
    {
        let header = format!("# Uneven Log — {}\n\n", chrono::Utc::now().format("%Y-%m-%d"));
        let _ = file.write_all(header.as_bytes());
    }

    if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(&log_abs_path) {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let entry = format!(
            "## [{}] 💻 Command Executed\n\n**Command:** `{}`\n\n**Summary:** {}\n\n**Status:** {}\n\n---\n\n",
            timestamp, cmd, summary, status
        );
        let _ = file.write_all(entry.as_bytes());
    }
}

pub async fn execute_terminal_command_streaming(cmd: &str) -> (bool, String) {
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncBufReadExt, BufReader};

    let child = if cfg!(target_os = "windows") {
        tokio::process::Command::new("cmd")
            .args(["/C", cmd])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    } else {
        tokio::process::Command::new("sh")
            .args(["-c", cmd])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    };

    let mut child = match child {
        Ok(c) => c,
        Err(e) => return (false, format!("Failed to execute command: {}", e)),
    };

    let stdout_handle = child.stdout.take().unwrap();
    let stderr_handle = child.stderr.take().unwrap();

    let output_data = Arc::new(Mutex::new(String::new()));
    let output_data_clone1 = output_data.clone();
    let output_data_clone2 = output_data.clone();

    let t1 = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout_handle).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            println!("  │  {}", line);
            let mut data = output_data_clone1.lock().unwrap();
            data.push_str(&line);
            data.push('\n');
        }
    });

    let t2 = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr_handle).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            eprintln!("  │  {}", line);
            let mut data = output_data_clone2.lock().unwrap();
            data.push_str(&line);
            data.push('\n');
        }
    });

    let status = child.wait().await;
    let _ = tokio::join!(t1, t2);

    let success = status.map(|s| s.success()).unwrap_or(false);
    let final_output = {
        let data = output_data.lock().unwrap();
        data.clone()
    };

    (success, final_output)
}

pub async fn ensure_local_model_files(model: &str) -> Result<(), String> {
    let global_models_dir = uneven_core::infrastructure::utils::paths::global_models_dir();
    let gguf_path = global_models_dir.join(format!("{}.gguf", model));
    let tokenizer_path = global_models_dir.join(format!("{}-tokenizer.json", model));

    if gguf_path.exists() && tokenizer_path.exists() {
        return Ok(());
    }

    let catalog_entry = uneven_core::domain::consts::models::SUPPORTED_LOCAL_MODELS
        .iter()
        .find(|m| m.id == model);

    match catalog_entry {
        Some(entry) => {
            println!(
                "\n  {} Local model files are missing for '{}'. Starting automatic download...",
                ui::accent("›"),
                model
            );
            for file_entry in entry.files {
                let suffix = file_entry
                    .dest
                    .strip_prefix(".uneven/models/")
                    .unwrap_or(file_entry.dest);
                let dest = global_models_dir.join(suffix);
                let label = dest.file_name().unwrap_or_default().to_string_lossy();
                if dest.exists() {
                    continue;
                }
                if let Err(e) = crate::commands::init::download_file(file_entry.url, &dest, &label).await {
                    return Err(format!("Failed to download {}: {}", label, e));
                }
            }
            println!();
            Ok(())
        }
        None => Err(format!(
            "Model '{}' is not in the supported local catalog for auto-download.\n  \
             Please place the GGUF file manually in {} as '{}.gguf' and tokenizer as '{}-tokenizer.json'.",
            model,
            global_models_dir.display(),
            model,
            model
        )),
    }
}
