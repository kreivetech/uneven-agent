use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use crate::domain::parsed_error::ParsedError;
use crate::domain::testing::TestResult;
use crate::domain::common::Logger;

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FixSuggestion {
    pub error: ParsedError,
    pub explanation: String,
    pub suggested_fix: String,
    pub before_code: String,
    pub after_code: String,
    pub confidence: f32,
    pub grounded_in_kb: Option<bool>,
    pub kb_sources: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FixOptions {
    pub create_branch: Option<bool>,
    pub commit_fix: Option<bool>,
    pub run_tests: Option<bool>,
    pub revert_on_failure: Option<bool>,
    pub expected_checksum: Option<String>,
}

pub trait LockCallbacks: Send + Sync {
    fn acquire(&self, name: &str) -> futures::future::BoxFuture<'_, Result<String, crate::domain::error::UnevenError>>;
    fn release(&self, lock_id: &str);
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FixApplyResult {
    pub applied: bool,
    pub checkpoint: Option<Value>,
    pub snapshot_id: Option<String>,
    pub branch: Option<String>,
    pub commit_hash: Option<String>,
    pub test_result: Option<TestResult>,
    pub test_validated: bool,
    pub reverted_due_to_failure: bool,
}

pub struct FixCtx {
    pub logger: Box<dyn Logger>,
    pub config: Value,
    pub checkpoints: Value,
    pub snapshots: Value,
    pub git: Value,
    pub test_runner: Value,
    pub retriever: Option<Value>,
    pub safety_guard: Value,
    pub lock: Option<Box<dyn LockCallbacks>>,
    pub fix_attempts: HashMap<String, u32>,
    pub max_retries: u32,
    pub project_root: String,
}

pub trait FixStrategy: Send + Sync {
    fn name(&self) -> &str;
    fn can_handle(&self, error: &ParsedError) -> bool;
    fn suggest(&self, error: &ParsedError) -> Option<FixSuggestion>;
}
