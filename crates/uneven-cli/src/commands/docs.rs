use anyhow::Result;

/// Generates polyglot API documentation or in-place docstrings using the think engine with the `docs` skill.
pub async fn run(
    file_path: String,
    symbol: Option<String>,
    format: String,
    _out: Option<String>,
    provider_opt: Option<String>,
    model_opt: Option<String>,
) -> Result<()> {
    let mut task = format!("document all exported symbols in `{}` with idiomatic documentation comments", file_path);
    if let Some(s) = symbol
        && !s.trim().is_empty() {
            task = format!("document the `{}` symbol in `{}` with idiomatic documentation comments", s.trim(), file_path);
        }
    if !format.is_empty() && format != "markdown" {
        task.push_str(&format!(" using {} format", format));
    }
    let _ = crate::commands::think::run(task, vec![file_path], provider_opt, model_opt).await?;
    Ok(())
}
