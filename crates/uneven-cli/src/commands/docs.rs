use anyhow::{anyhow, Result};
use console::style;
use crate::ui::{truecolor, truecolor_bold};
use std::fs;
use std::io::Write;
use std::path::Path;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::security::laws;
use uneven_core::infrastructure::llm::inference::chat_template::{
    detect_template, apply_template,
};
use uneven_core::infrastructure::llm::{
    InferParams, init as init_llm, infer_with_params_streaming,
};
use super::explain::{get_free_memory_mb, get_cpu_stats};

pub const MAX_DOCS_FILE_CHARS: usize = 15_000;

pub fn truncate_docs_content(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }
    format!(
        "{}\n\n[file truncated — showing first {} of {} chars]",
        &content[..max_chars],
        max_chars,
        content.len()
    )
}

pub fn build_docs_prompt(
    file_path: &str,
    content: &str,
    symbol: &str,
    format: &str,
) -> String {
    let truncated = truncate_docs_content(content, MAX_DOCS_FILE_CHARS);
    let target = if !symbol.trim().is_empty() {
        format!("the `{}` symbol (function, class, or interface)", symbol.trim())
    } else {
        "all exported symbols".to_string()
    };

    let task_section = if format == "jsdoc" {
        format!(
            "Generate complete TSDoc/JSDoc documentation comments for {}.\n\n\
             Requirements:\n\
             - Use /** ... */ block comment style (TSDoc-compatible)\n\
             - Include @param with name, type, and description for every parameter\n\
             - Include @returns with return type and a practical description\n\
             - Include @throws if the function can throw, with the error type\n\
             - Include one @example with a realistic usage snippet\n\
             - Keep descriptions concise but informative — no filler phrases\n\
             - Output: for each symbol, the TSDoc block immediately followed by the function/class/interface signature on the next line (no other code)",
            target
        )
    } else {
        format!(
            "Generate Markdown API documentation for {}.\n\n\
             Requirements:\n\
             - Use ## as heading level for each exported symbol\n\
             - List parameters in a markdown table: Name | Type | Required | Description\n\
             - Include return type and a practical description\n\
             - Include a ```typescript example``` code block with realistic usage\n\
             - Note any exceptions that can be thrown\n\
             - Keep descriptions concrete and actionable — no filler phrases\n\
             - Output: clean Markdown ready to be published in a docs site",
            target
        )
    };

    format!(
        "You are a TypeScript documentation expert generating production-quality API docs.\n\n\
         ## File: {}\n\n\
         ```typescript\n\
         {}\n\
         ```\n\n\
         ---\n\
         ## Task\n\
         {}",
        file_path, truncated, task_section
    )
}

pub async fn run(
    file_path_str: String,
    symbol: Option<String>,
    format: String,
    out: Option<String>,
    provider_opt: Option<String>,
    model_opt: Option<String>,
) -> Result<()> {
    let root = loader::find_project_root(None);
    let abs_path = fs::canonicalize(Path::new(&file_path_str))
        .map_err(|e| anyhow!("Could not resolve path '{}': {}", file_path_str, e))?;
    
    let content = fs::read_to_string(&abs_path)
        .map_err(|e| anyhow!("Could not read file '{}': {}", abs_path.display(), e))?;

    let lines = content.lines().count();
    let bytes = content.len();
    let kb = (bytes as f64 / 1024.0).to_string();
    let is_truncated = bytes > MAX_DOCS_FILE_CHARS;
    
    let rel_path = abs_path.strip_prefix(&root).unwrap_or(&abs_path);
    let target_label = symbol.as_ref().map(|s| format!("  symbol: {}", s)).unwrap_or_else(|| "  all exports".to_string());

    println!(
        "{} {} ({}){}",
        style("✓").green().bold(),
        style(rel_path.display()).green(),
        style(format!(
            "{} lines, {:.1} KB{}",
            lines,
            kb,
            if is_truncated { ", truncated" } else { "" }
        )).dim(),
        style(target_label).dim()
    );

    // 2. Load configuration
    let config = loader::load_config(&root)?;
    let mut brain = config.get_brain_for_command("docs");
    if let Some(ref p) = provider_opt {
        brain.provider = p.clone();
    }
    if let Some(ref m) = model_opt {
        brain.model = m.clone();
    }

    // 3. Initialize LLM
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    init_llm(&brain.provider, &brain.model, threads).await?;

    // 4. Setup Resource Guardian (only if local provider is active)
    let stop_guardian = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_guardian_clone = stop_guardian.clone();

    if brain.provider == "local" {
        tokio::spawn(async move {
            let mut cpu_sample = get_cpu_stats();
            let mut cpu_violations = 0;

            while !stop_guardian_clone.load(std::sync::atomic::Ordering::Relaxed) {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                // Memory check
                let free_mem = get_free_memory_mb();
                if free_mem < 200 {
                    uneven_core::infrastructure::llm::interrupt_local_inference();
                    eprintln!("\n{}", truecolor_bold("  Safety interrupt: Memory critically low", 255, 107, 53));
                    {
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                        let _ = std::io::stderr().flush();
                    }
                    std::process::exit(0);
                }

                // CPU check
                if let Some((idle1, total1)) = cpu_sample {
                    if let Some((idle2, total2)) = get_cpu_stats() {
                        let idle_diff = idle2.saturating_sub(idle1);
                        let total_diff = total2.saturating_sub(total1);
                        if total_diff > 0 {
                            let usage = (1.0 - (idle_diff as f64) / (total_diff as f64)) * 100.0;
                            if usage > 87.0 {
                                cpu_violations += 1;
                                if cpu_violations >= 3 {
                                    uneven_core::infrastructure::llm::interrupt_local_inference();
                                    eprintln!("\n{}", truecolor_bold("  Safety interrupt: Sustained CPU usage too high", 255, 107, 53));
                                    {
                                        use std::io::Write;
                                        let _ = std::io::stdout().flush();
                                        let _ = std::io::stderr().flush();
                                    }
                                    std::process::exit(0);
                                }
                            } else {
                                cpu_violations = 0;
                            }
                        }
                        cpu_sample = Some((idle2, total2));
                    }
                } else {
                    cpu_sample = get_cpu_stats();
                }
            }
        });
    }

    // 5. Prompt formatting
    let symbol_str = symbol.unwrap_or_default();
    let raw_prompt = build_docs_prompt(&rel_path.to_string_lossy(), &content, &symbol_str, &format);
    let system_instructions = laws::get_context("docs");

    let chat_template = detect_template(&brain.model);
    let formatted_prompt = apply_template(&chat_template, &system_instructions, &[], &raw_prompt);

    let params = InferParams {
        model: brain.model.clone(),
        threads,
        max_tokens: brain.max_tokens,
        temperature: brain.temperature as f64,
        repeat_penalty: 1.1,
        max_context: 4096,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: Some(system_instructions.clone()),
        history: None,
    };

    let mode_label = if format == "jsdoc" { "TSDoc" } else { "Markdown" };
    println!("\n{}", style(format!("  ┌─ Documentation  [{}]", mode_label)).dim());
    print!("  │\n  │  ");
    std::io::stdout().flush()?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
    let tx_clone = tx.clone();
    
    let prompt_to_use = if brain.provider == "local" {
        formatted_prompt
    } else {
        raw_prompt
    };
    let prompt_to_use_clone = prompt_to_use.clone();
    
    let params_clone = InferParams {
        model: params.model.clone(),
        threads: params.threads,
        max_tokens: params.max_tokens,
        temperature: params.temperature,
        repeat_penalty: params.repeat_penalty,
        max_context: params.max_context,
        grammar: params.grammar.clone(),
        provider: params.provider.clone(),
        system: params.system.clone(),
        history: params.history.clone(),
    };

    let inference_handle = tokio::spawn(async move {
        infer_with_params_streaming(
            &prompt_to_use_clone,
            &params_clone,
            move |token| {
                let _ = tx_clone.try_send(token);
            }
        ).await
    });

    drop(tx);

    let mut docs_text = String::new();
    while let Some(token) = rx.recv().await {
        let parts: Vec<&str> = token.split('\n').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                print!("\n  │  ");
            }
            print!("{}", truecolor(part, 232, 237, 245));
        }
        std::io::stdout().flush()?;
        docs_text.push_str(&token);
    }

    // Stop guardian thread
    stop_guardian.store(true, std::sync::atomic::Ordering::Relaxed);

    let result = inference_handle.await??;
    println!("\n{}", style("  └─ Done").dim());
    let (cost, saved) = uneven_core::infrastructure::llm::pricing::calculate_cost_and_saved(
        &result.model,
        result.prompt_tokens,
        result.completion_tokens,
    );
    println!(
        "{}",
        style(format!(
            "  [Tokens used: {} | Cost: ${:.5} | Saved: ${:.5}]",
            result.tokens_used, cost, saved
        )).dim()
    );
    println!();

    if let Some(out_path_str) = out {
        let out_path = Path::new(&out_path_str);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(out_path, &docs_text)?;
        println!("{}", truecolor(&format!("  Saved: {}", out_path_str), 168, 200, 255));
        println!();
    }

    Ok(())
}
