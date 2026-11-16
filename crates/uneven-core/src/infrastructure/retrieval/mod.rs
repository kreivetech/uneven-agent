//! Vector store and semantic retrieval module
//!
//! Persistence layout
//!   .uneven/vectors.bin        — postcard binary store (default backend)
//!   .uneven/vectors.json        — legacy — migration only
//!   .uneven/vectors.usearch    — HNSW graph   (vector-store backend)
//!   .uneven/vectors.meta.bin   — doc metadata  (vector-store backend)

pub mod types;
pub mod utils;

#[cfg(not(feature = "vector-store"))]
pub mod hashmap;
#[cfg(feature = "vector-store")]
pub mod hnsw;

pub use types::*;
pub use utils::*;

use crate::UnevenError;

// ─── Storage paths ────────────────────────────────────────────────────────────
pub const STORE_DIR:  &str = ".uneven";
pub const STORE_BIN:  &str = ".uneven/vectors.bin";
pub const STORE_JSON: &str = ".uneven/vectors.json";
#[cfg(feature = "vector-store")]
pub const STORE_USEARCH:     &str = ".uneven/vectors.usearch";
#[cfg(feature = "vector-store")]
pub const STORE_USEARCH_TMP: &str = ".uneven/vectors.usearch.tmp";
#[cfg(feature = "vector-store")]
pub const STORE_META_BIN: &str = ".uneven/vectors.meta.bin";

pub fn get_store_path(relative: &str) -> std::path::PathBuf {
    use crate::domain::PROJECT_ROOT;
    let root = PROJECT_ROOT.get().cloned().unwrap_or_else(|| ".".to_string());
    std::path::Path::new(&root).join(relative)
}

// ─── Public API ───────────────────────────────────────────────────────────────

pub async fn init_store() -> Result<(), UnevenError> {
    #[cfg(feature = "vector-store")]
    { hnsw::init().await }
    #[cfg(not(feature = "vector-store"))]
    { hashmap::init().await }
}

pub async fn add_documents(
    docs: Vec<(StoredDocument, Vec<f32>)>,
) -> Result<(), UnevenError> {
    #[cfg(feature = "vector-store")]
    { hnsw::add(docs).await }
    #[cfg(not(feature = "vector-store"))]
    { hashmap::add(docs).await }
}

pub async fn flush_store() -> Result<(), UnevenError> {
    #[cfg(feature = "vector-store")]
    { hnsw::flush().await }
    #[cfg(not(feature = "vector-store"))]
    { hashmap::flush().await }
}

pub async fn semantic_search(
    query_embedding: Vec<f32>,
    top_k: usize,
) -> Result<RetrievalResult, UnevenError> {
    #[cfg(feature = "vector-store")]
    { hnsw::search(query_embedding, top_k).await }
    #[cfg(not(feature = "vector-store"))]
    { hashmap::search(query_embedding, top_k).await }
}

pub async fn clear_index() -> Result<(), UnevenError> {
    #[cfg(feature = "vector-store")]
    { hnsw::clear().await }
    #[cfg(not(feature = "vector-store"))]
    { hashmap::clear().await }
}

pub async fn get_stats() -> Result<(usize, usize), UnevenError> {
    #[cfg(feature = "vector-store")]
    { Ok(hnsw::stats().await) }
    #[cfg(not(feature = "vector-store"))]
    { Ok(hashmap::stats().await) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_unit_vectors_is_one() {
        let v = vec![1.0_f32, 0.0, 0.0];
        let s = cosine_sim(&v, &v);
        assert!((s - 1.0).abs() < 1e-6);
    }

    #[test]
    fn assemble_empty_results() {
        let r = RetrievalResult { documents: vec![], scores: vec![] };
        let ctx = assemble_context(&r);
        assert!(ctx.contains("No relevant documents found"));
    }
}
