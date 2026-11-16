use thiserror::Error;

#[derive(Error, Debug)]
pub enum UnevenError {
    #[error("[UE-01000003] Initialization error: {0}")]
    InitFailed(String),

    #[error("[UE-02000002] Security policy violation: {0}")]
    SecurityError(String),

    #[error("[UE-03000001] IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("[UE-03000002] Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("[UE-04000001] Knowledge base error: {0}")]
    KnowledgeError(String),

    #[error("[UE-04000002] Retrieval error: {0}")]
    RetrievalError(String),

    #[error("[UE-05000001] LLM error: {0}")]
    LlmError(String),

    #[error("[UE-05000002] Model not found: {0}")]
    ModelNotFound(String),

    #[error("[UE-05000003] Out of memory (OOM): {0}")]
    OutOfMemory(String),

    #[error("[UE-05000004] Inference interrupted by user")]
    Interrupted,

    #[error("[UE-05000005] Inference timed out: {0}")]
    Timeout(String),

    #[error("[UE-06000001] Watcher error: {0}")]
    WatcherError(String),

    #[error("[UE-06000002] Agent error: {0}")]
    AgentError(String),

    #[error("[UE-06000012] Subagent LLM initialization failed: {0}")]
    SubagentInitFailed(String),

    #[error("[UE-06000013] Subagent inference failed: {0}")]
    SubagentInferenceFailed(String),

    #[error("[UE-06000014] Subagent file write failed: {0}")]
    SubagentFileWriteFailed(String),

    #[error("[UE-06000015] Subagent file modification limit exceeded")]
    SubagentLimitExceeded,

    #[error("[UE-07000003] Pentester error: {0}")]
    PentesterError(String),

    #[error("[UE-03000003] Git error: {0}")]
    GitError(String),

    #[error("[UE-02000001] Configuration error: {0}")]
    ConfigError(String),

    #[error("[UE-01000002] Not initialized")]
    NotInitialized,

    #[error("[UE-01999999] Unknown error: {0}")]
    Unknown(String),
}

impl From<infercraft::InferError> for UnevenError {
    fn from(err: infercraft::InferError) -> Self {
        match err {
            infercraft::InferError::LlmError(msg) => UnevenError::LlmError(msg),
            infercraft::InferError::ModelNotFound(msg) => UnevenError::ModelNotFound(msg),
            infercraft::InferError::OutOfMemory(msg) => UnevenError::OutOfMemory(msg),
            infercraft::InferError::Interrupted => UnevenError::Interrupted,
            infercraft::InferError::Io(e) => UnevenError::IoError(e),
            infercraft::InferError::HardwareError(msg) => UnevenError::KnowledgeError(msg),
            infercraft::InferError::Other(msg) => UnevenError::Unknown(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Display / error messages ──────────────────────────────────────────────

    #[test]
    fn init_failed_message_contains_detail() {
        let e = UnevenError::InitFailed("engine not ready".into());
        assert!(e.to_string().contains("engine not ready"));
        assert!(e.to_string().contains("Initialization error:"));
        assert!(e.to_string().starts_with("[UE-01000003]"));
    }

    #[test]
    fn llm_error_message_contains_detail() {
        let e = UnevenError::LlmError("context too long".into());
        assert!(e.to_string().contains("context too long"));
        assert!(e.to_string().contains("LLM error:"));
        assert!(e.to_string().starts_with("[UE-05000001]"));
    }

    #[test]
    fn model_not_found_message_contains_model_name() {
        let e = UnevenError::ModelNotFound("llama-3.2-1b-q4".into());
        assert!(e.to_string().contains("llama-3.2-1b-q4"));
    }

    #[test]
    fn interrupted_has_fixed_message() {
        let e = UnevenError::Interrupted;
        assert_eq!(e.to_string(), "[UE-05000004] Inference interrupted by user");
    }

    #[test]
    fn not_initialized_has_fixed_message() {
        let e = UnevenError::NotInitialized;
        assert_eq!(e.to_string(), "[UE-01000002] Not initialized");
    }

    #[test]
    fn timeout_message_contains_detail() {
        let e = UnevenError::Timeout("300000ms elapsed".into());
        assert!(e.to_string().contains("300000ms elapsed"));
        assert!(e.to_string().contains("Inference timed out:"));
        assert!(e.to_string().starts_with("[UE-05000005]"));
    }

    #[test]
    fn config_error_message_contains_detail() {
        let e = UnevenError::ConfigError("missing project_root".into());
        assert!(e.to_string().contains("missing project_root"));
    }

    #[test]
    fn knowledge_error_message_contains_detail() {
        let e = UnevenError::KnowledgeError("dir not found".into());
        assert!(e.to_string().contains("Knowledge base error:"));
        assert!(e.to_string().starts_with("[UE-04000001]"));
    }

    #[test]
    fn retrieval_error_message_contains_detail() {
        let e = UnevenError::RetrievalError("empty query".into());
        assert!(e.to_string().contains("Retrieval error:"));
        assert!(e.to_string().starts_with("[UE-04000002]"));
    }

    #[test]
    fn out_of_memory_message_contains_detail() {
        let e = UnevenError::OutOfMemory("4 GiB requested".into());
        assert!(e.to_string().contains("Out of memory (OOM):"));
        assert!(e.to_string().starts_with("[UE-05000003]"));
        assert!(e.to_string().contains("4 GiB"));
    }

    #[test]
    fn agent_error_message_prefix() {
        let e = UnevenError::AgentError("fix loop diverged".into());
        assert!(e.to_string().contains("Agent error:"));
        assert!(e.to_string().starts_with("[UE-06000002]"));
    }

    #[test]
    fn subagent_errors_contain_prefix_and_details() {
        let e = UnevenError::SubagentInitFailed("no api key".into());
        assert!(e.to_string().starts_with("[UE-06000012]"));
        assert!(e.to_string().contains("no api key"));

        let e = UnevenError::SubagentInferenceFailed("timeout".into());
        assert!(e.to_string().starts_with("[UE-06000013]"));
        assert!(e.to_string().contains("timeout"));

        let e = UnevenError::SubagentFileWriteFailed("permission error".into());
        assert!(e.to_string().starts_with("[UE-06000014]"));
        assert!(e.to_string().contains("permission error"));

        let e = UnevenError::SubagentLimitExceeded;
        assert_eq!(e.to_string(), "[UE-06000015] Subagent file modification limit exceeded");
    }

    #[test]
    fn pentester_error_message_prefix() {
        let e = UnevenError::PentesterError("host cannot be empty".into());
        assert!(e.to_string().contains("Pentester error:"));
        assert!(e.to_string().starts_with("[UE-07000003]"));
    }

    // ── From conversions ──────────────────────────────────────────────────────

    #[test]
    fn from_io_error_wraps_correctly() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let e: UnevenError = io.into();
        assert!(matches!(e, UnevenError::IoError(_)));
        assert!(e.to_string().contains("IO error:"));
        assert!(e.to_string().starts_with("[UE-03000001]"));
    }

    #[test]
    fn from_serde_error_wraps_correctly() {
        let serde_err = serde_json::from_str::<serde_json::Value>("not-json").unwrap_err();
        let e: UnevenError = serde_err.into();
        assert!(matches!(e, UnevenError::SerializationError(_)));
        assert!(e.to_string().contains("Serialization error:"));
        assert!(e.to_string().starts_with("[UE-03000002]"));
    }

    #[test]
    fn watcher_error_message_contains_detail() {
        let e = UnevenError::WatcherError("inotify failed".into());
        assert!(e.to_string().contains("Watcher error:"));
        assert!(e.to_string().starts_with("[UE-06000001]"));
        assert!(e.to_string().contains("inotify"));
    }

    #[test]
    fn unknown_error_message_contains_detail() {
        let e = UnevenError::Unknown("strange behavior".into());
        assert!(e.to_string().contains("Unknown error:"));
        assert!(e.to_string().starts_with("[UE-01999999]"));
        assert!(e.to_string().contains("strange behavior"));
    }
}
