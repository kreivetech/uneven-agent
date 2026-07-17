use serde::{Deserialize, Serialize};
use serde_json::Value;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEvent {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub stack_trace: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventFixSuggestion {
    pub file: String,
    pub line: u32,
    pub before: String,
    pub after: String,
    pub confidence: f32,
    pub explanation: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    pub total_documents: u32,
    pub disk_size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EventType {
    ErrorDetected,
    FixSuggested,
    FixApplied,
    FixReverted,
    FixPending,
    PentestFinding,
    Indexed,
    Warning,
    Info,
    BrainReady,
    WatchStarted,
    Stopped,
    Initialized,
    FileChanged,
    FileAdded,
    FileRemoved,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnevenEvent {
    pub r#type: EventType,
    pub timestamp: DateTime<Utc>,
    pub data: Value,
}

pub trait UnevenEventHandler: Send + Sync {
    fn handle_event(&self, event: UnevenEvent) -> futures::future::BoxFuture<'_, Result<(), crate::domain::error::UnevenError>>;
}
