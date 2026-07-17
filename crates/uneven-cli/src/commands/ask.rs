use anyhow::Result;
use std::io::Write;
use indicatif::{ProgressBar, ProgressStyle};
use console::style;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::security::laws;
use uneven_core::infrastructure::security::SafetyGuard;
use uneven_core::infrastructure::llm::inference::chat_template::{
    detect_template, apply_template,
};
use uneven_core::infrastructure::llm::{
    InferParams, init as init_llm, infer_with_params_streaming,
};
use super::context_assembler::ContextAssembler;
use super::resource_guardian::ResourceGuardian;

pub async fn run(
    question: String,
    provider_opt: Option<String>,
    model_opt: Option<String>,
) -> Result<()> {
    let root = loader::find_project_root(None);
    
    // 1. Safety Guard Validation (Shield)
    let safety = SafetyGuard::new();
    if let Err(err) = safety.validate_intent(&question) {
        println!("\n{} {}", crate::ui::truecolor_bold("⚠️ [SECURITY POLICY]:", 255, 107, 53), err);
        return Ok(());
    }

    // 2. Load configuration
    let config = loader::load_config(&root)?;
    let mut brain = config.get_brain_for_command("ask");
    if let Some(ref p) = provider_opt {
        brain.provider = p.clone();
    }
    if let Some(ref m) = model_opt {
        brain.model = m.clone();
    }

    // 3. Assemble Context
    let context = ContextAssembler::assemble(&question, &config, &brain, &root, None, &[]).await
        .unwrap_or_else(|_| "(No relevant context found)\n\n".to_string());

    // 4. Initialize LLM
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    init_llm(&brain.provider, &brain.model, threads).await?;

    // 5. Setup Resource Guardian
    let guardian = ResourceGuardian::start(brain.provider.clone(), root.clone());

    // 6. Prompt formatting
    let system_instructions = laws::get_context("ask");
    let user_content = format!("{}\nQuestion: {}", context, question);

    let chat_template = detect_template(&brain.model);
    let formatted_prompt = apply_template(&chat_template, &system_instructions, &[], &user_content);

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

    let thinking_frames = [
        format!("{} {}", style("⠋").blue(), style("Thinking.  ").dim()),
        format!("{} {}", style("⠙").blue(), style("Thinking.. ").dim()),
        format!("{} {}", style("⠹").blue(), style("Thinking...").dim()),
        format!("{} {}", style("⠸").blue(), style("Thinking.  ").dim()),
        format!("{} {}", style("⠼").blue(), style("Thinking.. ").dim()),
        format!("{} {}", style("⠴").blue(), style("Thinking...").dim()),
        format!("{} {}", style("⠦").blue(), style("Thinking.  ").dim()),
        format!("{} {}", style("⠧").blue(), style("Thinking.. ").dim()),
        format!("{} {}", style("⠇").blue(), style("Thinking...").dim()),
        format!("{} {}", style("⠏").blue(), style("Thinking.  ").dim()),
    ];
    let thinking_frames_ref: Vec<&str> = thinking_frames.iter().map(|s| s.as_str()).collect();

    // Initialize Progress Spinner
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&thinking_frames_ref)
            .template("  {spinner}").unwrap(),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
    let tx_clone = tx.clone();
    
    let prompt_to_use = if brain.provider == "local" {
        formatted_prompt
    } else {
        user_content
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

    let mut renderer = crate::ui::StreamRenderer::new();
    let mut streaming_started = false;

    while let Some(token) = rx.recv().await {
        if !streaming_started {
            spinner.finish_and_clear();
            println!("\n{}", crate::ui::dim("  ┌─ Response"));
            println!();
            print!("  ");
            let _ = std::io::stdout().flush();
            streaming_started = true;
        }
        renderer.push(&token);
    }

    // Stop guardian thread
    guardian.stop();

    let result = match inference_handle.await {
        Ok(Ok(res)) => res,
        Ok(Err(e)) => {
            spinner.finish_and_clear();
            return Err(e.into());
        }
        Err(e) => {
            spinner.finish_and_clear();
            return Err(anyhow::anyhow!("Task panicked: {}", e));
        }
    };

    // Handle streaming vs fallback formatting
    if !streaming_started {
        spinner.finish_and_clear();
        println!("\n{}", crate::ui::dim("  ┌─ Question"));
        println!("  │  {}", crate::ui::accent(&question));
        println!("  │");
        println!("{}", crate::ui::dim("  └─ Answer"));
        crate::ui::blank();
        for line in result.content.lines() {
            println!("     {}", crate::ui::crystal(line));
        }
    } else {
        println!("\n{}", crate::ui::dim("  └─ Done"));
    }

    // Safety Response Censorship Check
    let sanitized_content = safety.sanitize_response(&result.content);
    if sanitized_content != result.content {
        println!("  {}", crate::ui::rust("⚠ [CENSORSHIP]: Some potentially unsafe contents in the output were blocked for security."));
    }

    let (cost, saved) = uneven_core::infrastructure::llm::pricing::calculate_cost_and_saved(
        &result.model,
        result.prompt_tokens,
        result.completion_tokens,
    );
    println!(
        "{}",
        crate::ui::dim(&format!(
            "  [Tokens used: {} | Cost: ${:.5} | Saved: ${:.5}]",
            result.tokens_used, cost, saved
        ))
    );
    crate::ui::blank();

    Ok(())
}
