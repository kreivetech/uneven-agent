use serde::{Deserialize, Serialize};
use crate::domain::common::SecurityFinding;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmVerdict {
    Confirmed,
    FalsePositive,
    Uncertain,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedFinding {
    #[serde(flatten)]
    pub finding: SecurityFinding,
    pub llm_verdict: Option<LlmVerdict>,
    pub llm_note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewVerdict {
    pub verdict: Option<LlmVerdict>,
    pub note: String,
}
