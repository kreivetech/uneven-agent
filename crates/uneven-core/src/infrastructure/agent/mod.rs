//! Autonomous agent module
//!
//! Diagnoses errors and applies fixes:
//! - Analyzes errors against knowledge base
//! - Generates fix candidates
//! - Applies surgical patches with similar crate

use crate::UnevenError;
use serde::{Deserialize, Serialize};

pub mod diagnose;
pub mod fix;
pub mod router;
pub mod strategies;
pub mod orchestrator;

pub use orchestrator::AgentOrchestrator;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FixSuggestion {
    pub file: String,
    pub line: usize,
    pub before: String,
    pub after: String,
    #[serde(default)]
    pub top_of_file: bool,
    #[serde(default)]
    pub append_eof: bool,
    pub confidence: f32,
    pub explanation: String,
    pub suggested_fix: String,
}

/// Diagnose an error
pub async fn diagnose(error_message: &str) -> Result<String, UnevenError> {
    tracing::info!("Diagnosing error: {}", error_message);
    diagnose::diagnose_error(error_message).await
}

/// Generate fix suggestions
pub async fn suggest_fixes(
    file_path: &str,
    error_message: &str,
) -> Result<Vec<FixSuggestion>, UnevenError> {
    tracing::info!("Generating fixes for: {}", file_path);
    let _ = diagnose::diagnose_error(error_message).await?;
    Ok(vec![])
}

/// Apply a fix to a file
pub async fn apply_fix(suggestion: &FixSuggestion) -> Result<bool, UnevenError> {
    tracing::info!("Applying fix to: {}", suggestion.file);
    fix::apply_patch(
        &suggestion.file, 
        &suggestion.before, 
        &suggestion.after, 
        suggestion.top_of_file, 
        suggestion.append_eof
    )
        .await
        .map(|_| true)
}
