use anyhow::Result;
use uneven_core::UnevenConfig;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use console::style;
use crate::ui::truecolor;
use indicatif::{ProgressBar, ProgressStyle};
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::llm::{InferParams, init as init_llm, infer_with_params_streaming};
use std::time::Instant;
use std::fs;
use std::path::PathBuf;
use std::io::{Write, IsTerminal};

fn get_cache_path(root: &std::path::Path, session_id: &str) -> PathBuf {
    let sanitized_id = session_id.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect::<String>();
    let filename = format!("multi_agent_tasks_{}.json", sanitized_id);
    uneven_core::infrastructure::utils::paths::project_cache_dir(root).join(filename)
}

fn load_cached_tasks(root: &std::path::Path, session_id: &str) -> Vec<String> {
    let path = get_cache_path(root, session_id);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(tasks) = serde_json::from_str::<Vec<String>>(&content) {
                return tasks;
            }
        }
    }
    Vec::new()
}

fn save_cached_tasks(root: &std::path::Path, tasks: &[String], session_id: &str) {
    let path = get_cache_path(root, session_id);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(tasks) {
        let _ = fs::write(path, content);
    }
}

fn clear_cached_tasks(root: &std::path::Path, session_id: &str) {
    let path = get_cache_path(root, session_id);
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}

pub async fn run(
    tasks: &[String],
    total_tokens: usize,
    config: &UnevenConfig,
    session_id: &str,
) -> Result<Vec<(String, String)>> {
    let root = loader::find_project_root(None);
    let mut tasks_to_run = tasks.to_vec();

    let cached = load_cached_tasks(&root, session_id);
    if !cached.is_empty() {
        let should_resume = if tasks.is_empty() {
            true
        } else {
            let is_interactive = std::io::stdin().is_terminal();
            if is_interactive {
                println!("\n{}", style("  ⚠️ [MULTI-AGENT]: Found pending tasks from a previous interrupted run in cache.").yellow());
                print!("  ? Do you want to resume the pending tasks instead of starting a new list? [y/n] › ");
                let _ = std::io::stdout().flush();
                
                let confirm = tokio::task::spawn_blocking(|| {
                    let mut buf = String::new();
                    if std::io::stdin().read_line(&mut buf).is_ok() {
                        let trimmed = buf.trim().to_lowercase();
                        trimmed == "y" || trimmed == "yes"
                    } else {
                        false
                    }
                }).await.unwrap_or(false);
                
                confirm
            } else {
                false
            }
        };

        if should_resume {
            println!("{}", style("  [MULTI-AGENT]: Resuming tasks from cache...").bold().cyan());
            tasks_to_run = cached;
        }
    }

    if tasks_to_run.is_empty() {
        println!("{}", style("  [MULTI-AGENT]: No tasks to run.").yellow());
        return Ok(Vec::new());
    }

    // Save initial list
    save_cached_tasks(&root, &tasks_to_run, session_id);

    // If the user's limit is less than or equal to 2048, each subagent gets a safe floor of 10000.
    // If the user defined a larger limit, we respect that larger limit.
    let max_tokens_per_agent = std::cmp::max(total_tokens, 10000);
    
    println!("\n{}", style(format!("  [MULTI-AGENT ORCHESTRATOR]: Starting execution of {} subtask(s)...", tasks_to_run.len())).bold().cyan());
    println!("  ↳ Safety output token budget per agent: {}", max_tokens_per_agent);
    println!("  ↳ {}", style("Type 'stop' or 'abort' + Enter at any time to cancel all agent tasks.").yellow());
    
    let abort_signal = Arc::new(AtomicBool::new(false));
    let abort_signal_clone = abort_signal.clone();
    let abort_monitor = tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let trimmed = line.trim().to_lowercase();
            if trimmed == "stop" || trimmed == "abort" || trimmed == "exit" {
                abort_signal_clone.store(true, Ordering::Relaxed);
                break;
            }
        }
    });

    let mut reports = Vec::new();
    let file_regex = regex::Regex::new(r#"(?i)FILE[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n```[\s\S]*?```\s*END_FILE\n?"#)?;
    
    for (i, task_desc) in tasks_to_run.iter().enumerate() {
        if abort_signal.load(Ordering::Relaxed) {
            println!("\n{}", style("  ✗ [MULTI-AGENT]: Execution cancelled by user.").red().bold());
            break;
        }
        // Save the remaining tasks in the cache file before starting
        save_cached_tasks(&root, &tasks_to_run[i..], session_id);

        let agent_id = format!("AGENT_{}", i + 1);
        let agent_label = format!("{} (Session ID: {})", agent_id, session_id);
        
        println!("\n{}", style(format!("  ► Executing Task {}/{}: {}", i + 1, tasks_to_run.len(), task_desc)).bold().magenta());
        
        // 1. Context Assembly for this subagent
        let askf_brain = config.get_brain_for_command("askf");
        let context = match crate::commands::context_assembler::ContextAssembler::assemble(
            task_desc,
            config,
            &askf_brain,
            &root,
            None,
            &[],
        ).await {
            Ok(c) => c,
            Err(_) => "(No relevant context found)\n\n".to_string(),
        };
        
        // 2. Initialize LLM for the subagent
        let threads = askf_brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
        if let Err(e) = init_llm(&askf_brain.provider, &askf_brain.model, threads).await {
            let internal_err = uneven_core::domain::error::UnevenError::SubagentInitFailed(e.to_string());
            let error_msg = crate::errors::sanitize_error_msg(&internal_err.to_string());
            eprintln!("  [DEBUG] Subagent LLM initialization failed: {:?}", e);
            println!("  {}", style(&error_msg).red());
            reports.push((task_desc.clone(), error_msg));
            continue;
        }
        
        // 3. Build prompts and run inference
        let user_content = format!("{}\nUser request: {}", context, task_desc);
        let prompt = crate::commands::askf::build_prompt(&user_content, session_id);
        
        let params = InferParams {
            model: askf_brain.model.clone(),
            threads,
            max_tokens: max_tokens_per_agent,
            temperature: askf_brain.temperature as f64,
            repeat_penalty: 1.1,
            max_context: 4096,
            grammar: None,
            provider: Some(askf_brain.provider.clone()),
            system: Some(uneven_core::infrastructure::security::laws::get_context("askf")),
            history: None,
        };
        
        let thinking_frames = [
            format!("{} {}", style("⠋").cyan(), style(format!("{} Thinking.  ", agent_label)).dim()),
            format!("{} {}", style("⠙").cyan(), style(format!("{} Thinking.. ", agent_label)).dim()),
            format!("{} {}", style("⠹").cyan(), style(format!("{} Thinking...", agent_label)).dim()),
            format!("{} {}", style("⠸").cyan(), style(format!("{} Thinking.  ", agent_label)).dim()),
            format!("{} {}", style("⠼").cyan(), style(format!("{} Thinking.. ", agent_label)).dim()),
            format!("{} {}", style("⠴").cyan(), style(format!("{} Thinking...", agent_label)).dim()),
        ];
        let thinking_frames_ref: Vec<&str> = thinking_frames.iter().map(|s| s.as_str()).collect();
        
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&thinking_frames_ref)
                .template("  ↳ {spinner}").unwrap(),
        );
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        
        let start_time = Instant::now();
        let spinner_clone = spinner.clone();
        let first_token = Arc::new(AtomicBool::new(true));
        let first_token_clone = first_token.clone();
        let agent_label_clone = agent_label.clone();
        
        let infer_res = tokio::select! {
            res = infer_with_params_streaming(&prompt, &params, move |_token| {
                if first_token_clone.swap(false, Ordering::Relaxed) {
                    let generating_frames = [
                        format!("{} {}", style("⠋").cyan(), style(format!("{} Generating files.  ", agent_label_clone)).dim()),
                        format!("{} {}", style("⠙").cyan(), style(format!("{} Generating files.. ", agent_label_clone)).dim()),
                        format!("{} {}", style("⠹").cyan(), style(format!("{} Generating files...", agent_label_clone)).dim()),
                        format!("{} {}", style("⠸").cyan(), style(format!("{} Generating files.  ", agent_label_clone)).dim()),
                        format!("{} {}", style("⠼").cyan(), style(format!("{} Generating files.. ", agent_label_clone)).dim()),
                        format!("{} {}", style("⠴").cyan(), style(format!("{} Generating files...", agent_label_clone)).dim()),
                    ];
                    let generating_frames_ref: Vec<&str> = generating_frames.iter().map(|s| s.as_str()).collect();
                    spinner_clone.set_style(
                        ProgressStyle::default_spinner()
                            .tick_strings(&generating_frames_ref)
                            .template("  ↳ {spinner}").unwrap(),
                    );
                }
            }) => Some(res),
            _ = async {
                while !abort_signal.load(Ordering::Relaxed) {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            } => None,
        };
        
        spinner.finish_and_clear();
        let _elapsed = start_time.elapsed();
        
        if abort_signal.load(Ordering::Relaxed) {
            println!("\n{}", style("  ✗ [MULTI-AGENT]: Execution cancelled by user.").red().bold());
            break;
        }

        let res = match infer_res {
            Some(Ok(r)) => r,
            Some(Err(e)) => {
                let internal_err = uneven_core::domain::error::UnevenError::SubagentInferenceFailed(e.to_string());
                let error_msg = crate::errors::sanitize_error_msg(&internal_err.to_string());
                eprintln!("  [DEBUG] Subagent inference failed: {:?}", e);
                println!("  {}", style(&error_msg).red());
                reports.push((task_desc.clone(), error_msg));
                continue;
            }
            None => {
                break;
            }
        };
        // Parse generated files
                let mut files = crate::commands::askf::parse_files_with_id(&res.content, session_id);
                if files.is_empty() {
                    files = crate::commands::askf::parse_files(&res.content);
                }
                
                // Enforce safety limit of 3 modified files per subagent
                if files.len() > 3 {
                    let internal_err = uneven_core::domain::error::UnevenError::SubagentLimitExceeded;
                    let reject_msg = crate::errors::sanitize_error_msg(&internal_err.to_string());
                    eprintln!("  [DEBUG] Subagent file modification limit exceeded (tried to modify {} files)", files.len());
                    println!("  {}", style(&reject_msg).yellow().bold());
                    
                    // Retrieve summary explanation if any
                    let strip_pattern = format!(
                        r#"(?i)FILE_{}[\"\']?\s*[:=]?\s*[\"\']?[<\[\{{\(]?[^\s\]\[}}{{\"\',;<>\(\)]+[\"\']?\s*[>\]}})]?\s*\n\s*```[\s\S]*?```\s*END_(?:FILE_)?{}\n?"#,
                        session_id, session_id
                    );
                    let mut explanation = res.content.clone();
                    if let Ok(re) = regex::Regex::new(&strip_pattern) {
                        explanation = re.replace_all(&explanation, "").to_string();
                    }
                    explanation = file_regex.replace_all(&explanation, "").to_string();
                    
                    let final_status = format!(
                        "{} (Tried to modify {} files, limit is 3)\nAgent explanation/thoughts:\n{}",
                        reject_msg,
                        files.len(),
                        explanation.trim()
                    );
                    reports.push((task_desc.clone(), final_status));
                } else if !files.is_empty() {
                    // Write parsed files to disk since it is within limits
                    if let Err(e) = crate::commands::askf::write_parsed_files(&root, files) {
                        let internal_err = uneven_core::domain::error::UnevenError::SubagentFileWriteFailed(e.to_string());
                        let err_msg = crate::errors::sanitize_error_msg(&internal_err.to_string());
                        eprintln!("  [DEBUG] Subagent file write failed: {:?}", e);
                        println!("  {}", style(&err_msg).red());
                        reports.push((task_desc.clone(), err_msg));
                    } else {
                        // Success: retrieve explanation/summary
                        let strip_pattern = format!(
                            r#"(?i)FILE_{}[\"\']?\s*[:=]?\s*[\"\']?[<\[\{{\(]?[^\s\]\[}}{{\"\',;<>\(\)]+[\"\']?\s*[>\]}})]?\s*\n\s*```[\s\S]*?```\s*END_(?:FILE_)?{}\n?"#,
                            session_id, session_id
                        );
                        let mut explanation = res.content.clone();
                        if let Ok(re) = regex::Regex::new(&strip_pattern) {
                            explanation = re.replace_all(&explanation, "").to_string();
                        }
                        explanation = file_regex.replace_all(&explanation, "").to_string();
                        
                        let explanation = explanation.trim().to_string();
                        if !explanation.is_empty() {
                            println!("{} {}", truecolor("  › ", 168, 200, 255), truecolor("Summary:", 107, 122, 150));
                            for line in explanation.lines() {
                                println!("    {}", truecolor(line, 107, 122, 150));
                            }
                            println!();
                        }
                        
                        let final_status = format!(
                            "SUCCESS: File modifications applied successfully.\nSummary of modifications:\n{}",
                            explanation
                        );
                        reports.push((task_desc.clone(), final_status));
                    }
                } else {
                    // No files declared
                    println!("{}", truecolor("  › The subagent did not declare any file modifications.", 107, 122, 150));
                    let final_status = format!(
                        "SUCCESS: Execution finished without file modifications.\nAgent explanation/thoughts:\n{}",
                        res.content.trim()
                    );
                    reports.push((task_desc.clone(), final_status));
                }
    }
    
    // Success: clear the tasks list cache
    clear_cached_tasks(&root, session_id);

    abort_monitor.abort();

    println!("\n{}", style("  [MULTI-AGENT ORCHESTRATOR]: All subtasks processed.").bold().green());
    Ok(reports)
}
