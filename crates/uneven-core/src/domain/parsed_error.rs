use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorType {
    Compiler,
    Runtime,
    Linter,
    Test,
    ProactiveScan,
    #[default]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSeverity {
    #[default]
    Error,
    Warning,
    Info,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedError {
    pub r#type: ErrorType,
    pub severity: ErrorSeverity,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub code: String,
    pub context: Vec<String>,
    pub full_output: String,
}
