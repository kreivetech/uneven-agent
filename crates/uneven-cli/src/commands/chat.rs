use anyhow::Result;
use crate::ui::truecolor;
use std::io::{self, Write};
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::security::laws;
use uneven_core::infrastructure::security::SafetyGuard;
use uneven_core::infrastructure::llm::inference::chat_template::{
    detect_template, apply_template,
};
use uneven_core::infrastructure::llm::{
    InferParams, init as init_llm, infer_with_params_streaming,
};
use super::resource_guardian::ResourceGuardian;

pub const MAX_HISTORY_TURNS: usize = 10;

pub fn print_welcome(version: &str, brain: &str) {
    let line1 = format!("◈  U N E V E N   A I   v{}", version);
    let line2 = "Agent: Snatchy (Senior Engineer)".to_string();
    let line3 = format!("Brain: {}", brain);
    
    let bright_color = |s: &str| truecolor(s, 208, 232, 255);
    let dim_color = |s: &str| truecolor(s, 107, 122, 150);

    println!();
    println!("  {}", bright_color(&line1));
    println!("  {}", dim_color(&line2));
    println!("  {}", dim_color(&line3));
    println!();
}

pub async fn run(
    provider_opt: Option<String>,
    model_opt: Option<String>,
) -> Result<()> {
    let root = loader::find_project_root(None);
    let config = loader::load_config(&root)?;
    let mut brain = config.get_brain_for_command("chat");
    
    if let Some(ref p) = provider_opt {
        brain.provider = p.clone();
    }
    if let Some(ref m) = model_opt {
        brain.model = m.clone();
    }

    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    init_llm(&brain.provider, &brain.model, threads).await?;

    let version = env!("CARGO_PKG_VERSION"); // standard CLI version
    let active_brain = format!("{} / {}", brain.provider, brain.model);
    print_welcome(version, &active_brain);

    println!("\n\n\n\n");
    println!("{}", truecolor("  How can I help you today? (type \"help\" or \"exit\")", 107, 122, 150));
    println!();

    let mut history: Vec<(String, String)> = Vec::new();
    let safety = SafetyGuard::new();

    // Setup Resource Guardian
    let guardian = ResourceGuardian::start(brain.provider.clone(), root.clone());

    loop {
        print!("{}", truecolor("  › ", 168, 200, 255));
        std::io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let question = input.trim();

        if question.is_empty() {
            continue;
        }

        if question.to_lowercase() == "exit" || question.to_lowercase() == "quit" {
            break;
        }

        if question.to_lowercase() == "help" {
            println!("{}", truecolor("  Available commands: help, exit, quit", 107, 122, 150));
            println!();
            continue;
        }

        // Validate intent using SafetyGuard
        if let Err(err) = safety.validate_intent(question) {
            println!("\n{} {}", crate::ui::truecolor_bold("⚠️ [SECURITY POLICY]:", 255, 107, 53), err);
            println!();
            continue;
        }

        println!();
        println!("{}", truecolor("  ┌─ Response", 107, 122, 150));
        println!();
        print!("  ");
        std::io::stdout().flush()?;

        let system_instructions = laws::get_context("chat");
        
        let mut prompt_to_use = question.to_string();
        let mut system_instructions_to_use = system_instructions.clone();

        if brain.provider != "local" {
            let files_map = crate::commands::interactive::get_project_files_map(&config, &root);
            prompt_to_use = uneven_core::infrastructure::llm::history_manager::optimize_context(
                &mut history,
                question,
                &system_instructions,
                Some(&files_map),
            );
            system_instructions_to_use = String::new();
        }

        let chat_template = detect_template(&brain.model);
        let chat_history: Vec<uneven_core::infrastructure::llm::inference::chat_template::ChatTurn> = history.iter().map(|(r, c)| {
            uneven_core::infrastructure::llm::inference::chat_template::ChatTurn {
                role: r.clone(),
                content: c.clone(),
            }
        }).collect();
        let formatted_prompt = apply_template(&chat_template, &system_instructions_to_use, &chat_history, &prompt_to_use);

        let system_param = if system_instructions_to_use.is_empty() {
            None
        } else {
            Some(system_instructions_to_use)
        };

        let params = InferParams {
            model: brain.model.clone(),
            threads,
            max_tokens: brain.max_tokens,
            temperature: brain.temperature as f64,
            repeat_penalty: 1.1,
            max_context: 4096,
            grammar: None,
            provider: Some(brain.provider.clone()),
            system: system_param,
            history: Some(history.clone()),
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
        let tx_clone = tx.clone();
        
        let final_prompt = if brain.provider == "local" {
            formatted_prompt
        } else {
            prompt_to_use.clone()
        };
        let prompt_to_use_clone = final_prompt.clone();
        
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

        let mut renderer = crate::ui::StreamRenderer::new();
        let mut answer = String::new();
        while let Some(token) = rx.recv().await {
            renderer.push(&token);
            answer.push_str(&token);
        }

        let result = inference_handle.await??;
        
        let turn_label = history.len() / 2 + 1;
        let remaining = MAX_HISTORY_TURNS.saturating_sub(history.len() + 2);
        let hint = if remaining > 0 {
            format!("turn {} · {} turns until oldest context drops", turn_label, remaining)
        } else {
            format!("turn {} · history window full", turn_label)
        };

        println!("\n{}", truecolor(&format!("  └─ Done  {}", hint), 107, 122, 150));

        // Safety Response Censorship Check
        let sanitized_answer = safety.sanitize_response(&answer);
        if sanitized_answer != answer {
            println!("  {}", crate::ui::rust("⚠ [CENSORSHIP]: Some potentially unsafe contents in the output were blocked for security."));
            answer = sanitized_answer;
        }

        let (cost, saved) = uneven_core::infrastructure::llm::pricing::calculate_cost_and_saved(
            &result.model,
            result.prompt_tokens,
            result.completion_tokens,
        );
        println!(
            "{}",
            truecolor(
                &format!(
                    "  [Tokens used: {} | Cost: ${:.5} | Saved: ${:.5}]",
                    result.tokens_used, cost, saved
                ),
                107, 122, 150
            )
        );
        println!();

        history.push(("user".to_string(), prompt_to_use));
        history.push(("assistant".to_string(), answer));
        if history.len() > MAX_HISTORY_TURNS {
            history.splice(0..history.len() - MAX_HISTORY_TURNS, std::iter::empty());
        }
    }

    // Stop guardian thread
    guardian.stop();
    println!("{}", truecolor("  Goodbye.", 107, 122, 150));
    println!();

    Ok(())
}
