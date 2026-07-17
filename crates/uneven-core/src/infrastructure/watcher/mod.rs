//! Terminal and file watcher module
//!
//! Monitors running processes and file system changes:
//! - Terminal stdout/stderr capturing via tokio::process
//! - File system events via notify crate
//! - Error extraction from logs

pub mod terminal;
pub mod file;
pub mod error_parser;
pub mod process;
pub mod session;

pub use session::WatcherSession;

use crate::UnevenError;
use crate::domain::parsed_error::ErrorType;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ErrorEvent {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub stack_trace: String,
    pub error_type: Option<ErrorType>,
    pub code: Option<String>,
}

/// Initialize watcher
pub async fn init() -> Result<(), UnevenError> {
    Ok(())
}

/// Start watching directory for changes
pub async fn watch_directory(path: &str) -> Result<(), UnevenError> {
    if Path::new(path).components().any(|c| c == Component::ParentDir) {
        return Err(UnevenError::WatcherError("path traversal denied".into()));
    }
    tracing::info!("Watching directory: {}", path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_is_ok() {
        assert!(init().await.is_ok());
    }

    #[tokio::test]
    async fn test_watch_directory_ok() {
        assert!(watch_directory("src").await.is_ok());
    }

    #[tokio::test]
    async fn test_watch_directory_traversal_denied() {
        let result = watch_directory("src/../../etc").await;
        assert!(result.is_err());
        if let Err(UnevenError::WatcherError(msg)) = result {
            assert!(msg.contains("traversal"));
        } else {
            panic!("Expected WatcherError");
        }
    }
}
