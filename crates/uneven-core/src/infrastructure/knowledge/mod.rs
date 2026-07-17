//! Knowledge base indexing module
//!
//! Indexes all knowledge sources:
//! - Local files and directories
//! - Code files (Rust, TypeScript, etc.)
//! - Documentation (MD, TXT)
//! - Structured data (MD files with frontmatter)
//!
//! TypeScript layer handles:
//! - Database loading via knex/mongodb
//! - URL fetching via undici

pub mod indexer;
pub mod file_loader;
pub mod database_loader;
pub mod document_loader;
pub mod web_loader;
pub mod format_readers;
pub mod scanner;
pub mod chunker;
pub mod dependency_graph;
pub mod incremental_index;
pub mod prompts;
pub mod parser;

pub use document_loader::DocumentLoader;
pub use web_loader::{WebScraper, WebDocument};
pub use chunker::{Chunker, Chunk};
pub use dependency_graph::DependencyGraph;
pub use incremental_index::IncrementalIndex;
pub use database_loader::DatabaseAnalyst;
pub use parser::{ResponseParser, GeneratedQuery};

use crate::UnevenError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexedDocument {
    pub id: String,
    pub path: String,
    pub content: String,
    pub doc_type: String, // "file", "database", "url"
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexStats {
    pub total_documents: usize,
    pub disk_size: u64,
}

/// Initialize knowledge base
pub async fn init() -> Result<(), UnevenError> {
    Ok(())
}

/// Index all knowledge sources
pub async fn index_all(dirs: Vec<&str>) -> Result<IndexStats, UnevenError> {
    tracing::info!("Starting knowledge base indexing for {} directories", dirs.len());

    let ki = indexer::KnowledgeIndexer::new();
    let mut total_stats = IndexStats {
        total_documents: 0,
        disk_size: 0,
    };

    for dir in dirs {
        let stats = ki.index_directory(dir, |doc| {
            // Here we could store in vector store, but for now we follow the 
            // current architecture where Node.js orchestrates storage.
            // We just trace for now to stay safe.
            tracing::debug!("Document indexed: {}", doc.path);
            Ok(())
        }).await?;
        
        total_stats.total_documents += stats.total_documents;
        total_stats.disk_size += stats.disk_size;
    }
    
    Ok(total_stats)
}
