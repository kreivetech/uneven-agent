use anyhow::Result;

/// Explains a target file, symbol, or architecture using the think engine with the `explain` skill.
pub async fn run(
    file_path: String,
    focus: Option<String>,
    provider_opt: Option<String>,
    model_opt: Option<String>,
    _no_guardian: bool,
) -> Result<()> {
    let mut task = format!("explain the architecture, flow, and responsibilities of `{}`", file_path);
    if let Some(f) = focus
        && !f.trim().is_empty() {
            task.push_str(&format!(" with specific focus on: {}", f.trim()));
        }
    let _ = crate::commands::think::run(task, vec![file_path], provider_opt, model_opt).await?;
    Ok(())
}
