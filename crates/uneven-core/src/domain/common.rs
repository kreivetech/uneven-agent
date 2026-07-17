use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    #[default]
    None,
    Info,
    Low,
    Medium,
    High,
    Critical,
}

pub trait Logger: Send + Sync {
    fn info(&self, msg: &str);
    fn error(&self, msg: &str);
    fn warn(&self, msg: &str);
    fn debug(&self, _msg: &str) {}
}

#[derive(Clone)]
pub struct NoopLogger;

impl Logger for NoopLogger {
    fn info(&self, _msg: &str) {}
    fn error(&self, _msg: &str) {}
    fn warn(&self, _msg: &str) {}
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityContext {
    pub project_root: String,
    pub project_id: Option<String>,
    pub report_path: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityFinding {
    pub severity: Severity,
    pub message: String,
    pub recommendation: String,
    pub cvss: Option<f32>,
    #[serde(rename = "type")]
    pub finding_type: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    #[serde(rename = "match")]
    pub match_context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderResponse {
    pub content: String,
    pub tokens_used: usize,
    pub model: String,
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmResponse {
    pub content: String,
    pub tokens_used: usize,
    pub model: String,
}

