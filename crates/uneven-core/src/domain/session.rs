use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiffStatus {
    Pending,
    Approved,
    Rejected,
    Applied,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PendingDiff {
    pub id: String,
    pub created_at: String,
    pub file: String,
    pub error_code: String,
    pub error_message: String,
    pub error_line: u32,
    pub explanation: String,
    pub suggested_fix: String,
    pub before_code: String,
    pub after_code: String,
    pub confidence: f64,
    pub expected_checksum: String,
    pub status: DiffStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSession {
    pub last_indexed_at: Option<String>,
    pub indexed_dirs: Vec<String>,
    pub indexed_files: Vec<String>,
    pub indexed_urls: Vec<String>,
    pub db_connected: bool,
    pub staleness: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BrainSession {
    pub provider: String,
    pub model: String,
    pub ready: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatcherSessionState {
    pub active: bool,
    pub terminal: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PentesterSession {
    pub enabled: bool,
    pub mode: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WriteLockSession {
    pub active: bool,
    pub locked_by: Option<String>,
    pub locked_at: Option<String>,
    pub lock_id: Option<String>,
    pub pid: Option<u32>,
    pub timeout_ms: u64,
    pub debounce_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnevenSession {
    pub session_id: String,
    pub started_at: String,
    pub uneven_version: String,
    pub project_root: String,
    pub mode: String,
    pub auto_fix: bool,
    pub confirm_before_fix: bool,
    pub alert_only: bool,
    pub knowledge: KnowledgeSession,
    pub brain: BrainSession,
    pub watcher: WatcherSessionState,
    pub pentester: PentesterSession,
    pub write_lock: WriteLockSession,
    pub pending_diffs: Vec<PendingDiff>,
}
