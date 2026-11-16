use anyhow::Result;

/// Main execution function for the `ask` command in the CLI.
/// Unifies with the `think` reasoning engine, providing pre-flight self-critique,
/// contextual investigation, and deterministic Markdown patch proposals with direct approval.
pub async fn run(
    question: String,
    provider_opt: Option<String>,
    model_opt: Option<String>,
) -> Result<()> {
    super::think::run(question, vec![], provider_opt, model_opt).await.map(|_| ())
}
