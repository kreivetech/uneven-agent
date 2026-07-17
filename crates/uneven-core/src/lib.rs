//! Uneven Core - Clean Architecture Entry Point
//! 
//! Layers:
//! - application: Use cases and orchestration logic
//! - domain: Core entities, configuration, and errors
//! - infrastructure: Concrete implementations (LLM, retrieval, FS, etc.)
pub mod domain;
pub mod application;
pub mod infrastructure;

// Re-export common types for convenience within the crate
pub use domain::config::*;
pub use domain::error::UnevenError;
pub use application::UnevenEngine;
