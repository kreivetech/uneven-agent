use super::patch_engine;
use super::resource_guardian::ResourceGuardian;
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{IsTerminal, Write};
use uneven_core::application::think_service::{
    execute_thought_loop, ThoughtPhase,
};
#[allow(unused_imports)]
pub use uneven_core::application::think_service::parse_need_file_tags;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::security::SafetyGuard;

/// Main execution function for the `think` command in the CLI.
/// Delegates to `uneven_core::application::think_service::execute_thought_loop`
/// while maintaining terminal spinners, interactive prompt, and styled rendering.
pub async fn run(
    task: String,
    files: Vec<String>,
    provider_opt: Option<String>,
    model_opt: Option<String>,
) -> Result<String> {
    let root = loader::find_project_root(None);

    // 1. Safety Guard Validation (Shield)
    let safety = SafetyGuard::new();
    if let Err(err) = safety.validate_intent(&task) {
        println!(
            "\n{} {}",
            crate::ui::truecolor_bold("⚠️ [SECURITY POLICY]:", 255, 107, 53),
            err
        );
        return Ok(String::new());
    }

    // 2. Load configuration
    let config = loader::load_config(&root)?;
    let mut brain = config.get_brain_for_command("think");
    if brain.provider.is_empty() || brain.provider == "default" {
        brain = config.get_brain_for_command("ask");
    }
    let provider = provider_opt.as_deref().unwrap_or(&brain.provider);
    let model = model_opt.as_deref().unwrap_or(&brain.model);

    // 3. Setup Resource Guardian
    let guardian = ResourceGuardian::start(provider.to_string(), root.clone());

    // 4. Setup Multi-Phase Progress Spinner
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("  {spinner} {msg}")
            .unwrap(),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let spinner_clone = spinner.clone();
    let on_phase = move |phase: ThoughtPhase| match phase {
        ThoughtPhase::ResolvingContext => spinner_clone.set_message("Context"),
        ThoughtPhase::Drafting => spinner_clone.set_message("Drafting"),
        ThoughtPhase::RubberDuckingDraft => spinner_clone.set_message("Reasoning"),
        ThoughtPhase::Auditing => spinner_clone.set_message("Auditing"),
        ThoughtPhase::VerifyingIntegrity => spinner_clone.set_message("Verifying"),
        ThoughtPhase::RubberDuckingSimulation => spinner_clone.set_message("Simulating"),
        ThoughtPhase::Synthesizing => spinner_clone.set_message("Synthesizing"),
        ThoughtPhase::Completed => {}
    };

    let result = execute_thought_loop(
        &task,
        &files,
        &config,
        &root,
        Some(provider),
        Some(model),
        Some(on_phase),
    )
    .await;

    spinner.finish_and_clear();
    guardian.stop();

    let thought_res = result?;
    let final_answer = thought_res.final_response;

    // 5. Display the Pre-Flight Assured Proposal
    println!("\n{}", crate::ui::dim("  ┌─ Think Engine: Pre-Flight Review & Proposal (3-Turn Reflection)"));
    println!("  │  {}", crate::ui::accent(&task));
    println!("  │");
    println!("{}", crate::ui::dim("  └─ Plan & Assured Patch"));
    crate::ui::blank();

    let display_answer =
        uneven_core::infrastructure::utils::markdown_cleaner::clean_markdown_for_display(
            &final_answer,
        );
    let mut renderer = crate::ui::StreamRenderer::new();
    for line in display_answer.lines() {
        renderer.push(&format!("{}\n", line));
    }
    crate::ui::blank();

    // 6. Check if there are file actions and persist pending patch to disk
    if let Some(ref file_obj) = thought_res.file_object
        && !file_obj.patch.is_empty()
    {
        let _ = uneven_core::infrastructure::llm::history_manager::save_pending_patch_to_disk(
            file_obj,
            &file_obj.session_id,
            &root,
        );
        let _ = uneven_core::infrastructure::llm::history_manager::save_pending_patch_to_disk(
            file_obj,
            "default",
            &root,
        );

        if std::io::stdin().is_terminal() {
            print!(
                "{} {} [y/N]: ",
                crate::ui::truecolor("  ›", 100, 200, 255),
                crate::ui::truecolor_bold("Would you like to apply these changes to disk?", 230, 235, 245)
            );
            let _ = std::io::stdout().flush();

            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_ok() {
                let trimmed = input.trim().to_lowercase();
                if trimmed == "y" || trimmed == "yes" || trimmed == "s" || trimmed == "sim" {
                    println!(
                        "\n{} {}",
                        crate::ui::truecolor("  › ", 168, 200, 255),
                        crate::ui::truecolor("Applying file actions:", 107, 122, 150)
                    );
                    patch_engine::file_action(&root, file_obj);
                    let _ = uneven_core::infrastructure::llm::history_manager::clear_pending_patch_from_disk(
                        &file_obj.session_id,
                        &root,
                    );
                    let _ = uneven_core::infrastructure::llm::history_manager::clear_pending_patch_from_disk(
                        "default",
                        &root,
                    );
                    println!("\n{}", crate::ui::dim("  └─ Changes successfully applied to disk."));
                }
            }
        }
    }

    Ok(final_answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_need_file_tags_delegation() {
        let text = "I need to inspect [NEED_FILE: src/main.rs] and [NEED_FILE: crates/core/lib.rs] before deciding.";
        let tags = parse_need_file_tags(text);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains("src/main.rs"));
        assert!(tags.contains("crates/core/lib.rs"));
    }

    #[test]
    fn test_parse_need_file_tags_empty() {
        let text = "Here is my final plan without any file tags.";
        let tags = parse_need_file_tags(text);
        assert!(tags.is_empty());
    }
}
