use std::sync::OnceLock;

pub mod config;
pub mod database;
pub mod error;
pub mod common;
pub mod kb;
pub mod active_analysis;
pub mod analyst;
pub mod data_analyst;
pub mod data_security;
pub mod dependency_audit;
pub mod parsed_error;
pub mod excel_export;
pub mod fix_engine;
pub mod malware_analysis;
pub mod notifications;
pub mod pentest_security;
pub mod report_packaging;
pub mod reporting;
pub mod sbom;
pub mod security_review;
pub mod testing;
pub mod event;
pub mod watcher;
pub mod session;
pub mod consts;

// Re-exports
pub use config::*;
pub use database::*;
pub use error::UnevenError;
pub use common::*;
pub use kb::*;
pub use active_analysis::*;
pub use analyst::*;
pub use data_analyst::*;
pub use data_security::*;
pub use dependency_audit::*;
pub use parsed_error::*;
pub use excel_export::*;
pub use fix_engine::*;
pub use malware_analysis::*;
pub use notifications::*;
pub use pentest_security::*;
pub use report_packaging::*;
pub use reporting::*;
pub use sbom::*;
pub use security_review::*;
pub use testing::*;
pub use event::*;
pub use watcher::*;
pub use session::*;
pub use consts::prompts::*;
pub use consts::templates::*;
pub use consts::models::*;

pub static PROJECT_ROOT: OnceLock<String> = OnceLock::new();

#[cfg(test)]
mod tests;

