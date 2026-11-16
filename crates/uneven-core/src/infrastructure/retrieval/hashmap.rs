use super::*;
use super::types::{StoredDocument, RetrievalResult};
use tokio::sync::RwLock;
use once_cell::sync::Lazy;
use crate::UnevenError;
use ragcraft::VectorStore;

/// In-memory state powered by `ragcraft::VectorStore`.
pub type VectorStoreState = VectorStore;

pub static STORE: Lazy<RwLock<VectorStoreState>> =
    Lazy::new(|| RwLock::new(VectorStoreState::default()));

pub async fn init() -> Result<(), UnevenError> {
    tracing::info!("Vector store (HashMap backend): initializing");

    let dir = get_store_path(STORE_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| UnevenError::RetrievalError(format!("create_dir_all failed: {e}")))?;

    let mut store = STORE.write().await;

    // Try binary format first
    let bin_path = get_store_path(STORE_BIN);
    if bin_path.exists() {
        match VectorStore::load_from_disk(&bin_path) {
            Ok(loaded) => {
                let n = loaded.len();
                *store = loaded;
                tracing::info!("Vector store loaded from binary ({n} documents)");
                return Ok(());
            }
            Err(e) => tracing::warn!("Binary store load failed — trying JSON migration: {e}"),
        }
    }

    // Migrate legacy JSON
    let json_path = get_store_path(STORE_JSON);
    if json_path.exists() {
        tracing::info!("Migrating vector store from JSON to binary");
        if let Ok(json) = tokio::fs::read_to_string(&json_path).await {
            if let Ok(migrated) = serde_json::from_str::<VectorStore>(&json) {
                let n = migrated.len();
                *store = migrated;
                tracing::info!("Migrated {n} documents from JSON");
                if store.save_to_disk(&bin_path).is_ok() {
                    let _ = tokio::fs::remove_file(&json_path).await;
                    tracing::info!("Migration complete — legacy JSON removed");
                }
                return Ok(());
            }
        }
    }

    tracing::info!("Vector store initialized empty");
    Ok(())
}

pub async fn add(docs: Vec<(StoredDocument, Vec<f32>)>) -> Result<(), UnevenError> {
    if docs.is_empty() { return Ok(()); }
    tracing::debug!("Adding {} documents", docs.len());
    let mut store = STORE.write().await;
    store.add_batch(docs);
    Ok(())
}

pub async fn flush() -> Result<(), UnevenError> {
    let store = STORE.read().await;
    let bin_path = get_store_path(STORE_BIN);
    store.save_to_disk(&bin_path)
        .map_err(|e| UnevenError::RetrievalError(format!("Failed to save vector store: {e}")))
}

pub async fn search(query: Vec<f32>, top_k: usize) -> Result<RetrievalResult, UnevenError> {
    let store = STORE.read().await;
    store.search(&query, top_k)
        .map_err(|e| UnevenError::RetrievalError(e.to_string()))
}

pub async fn clear() -> Result<(), UnevenError> {
    let mut store = STORE.write().await;
    store.clear();
    for p in [STORE_BIN, STORE_JSON] {
        let _ = tokio::fs::remove_file(get_store_path(p)).await;
    }
    Ok(())
}

pub async fn stats() -> (usize, usize) {
    let s = STORE.read().await;
    s.stats()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::types::StoredDocument;

    // The vector store is a process-global static. Serialize all tests that
    // touch it so that clear()+search() sequences are not interleaved.
    static STORE_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn make_doc(id: &str, content: &str) -> StoredDocument {
        StoredDocument {
            id: id.to_string(),
            path: id.to_string(),
            content: content.to_string(),
            doc_type: "test".to_string(),
            metadata: std::collections::HashMap::new(),
        }
    }

    fn unit(dim: usize, idx: usize, scale: f32) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        v[idx] = scale;
        v
    }

    #[tokio::test]
    async fn search_empty_store_returns_empty() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        let result = search(unit(4, 0, 1.0), 5).await.unwrap();
        assert!(result.documents.is_empty());
        assert!(result.scores.is_empty());
    }

    #[tokio::test]
    async fn search_empty_query_returns_error() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        let err = search(vec![], 5).await.unwrap_err();
        assert!(matches!(err, UnevenError::RetrievalError(_)));
    }

    #[tokio::test]
    async fn add_empty_embedding_skipped() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        let doc = make_doc("doc-empty", "content");
        add(vec![(doc, vec![])]).await.unwrap();
        let (docs, embs) = stats().await;
        assert_eq!(docs, 0);
        assert_eq!(embs, 0);
    }

    #[tokio::test]
    async fn search_returns_results_in_descending_score_order() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        let doc_a = make_doc("doc-a", "alpha");
        let doc_b = make_doc("doc-b", "beta");
        let doc_c = make_doc("doc-c", "gamma");
        add(vec![
            (doc_a, unit(4, 0, 1.0)),
            (doc_b, unit(4, 1, 1.0)),
            (doc_c, unit(4, 2, 1.0)),
        ]).await.unwrap();

        let results = search(unit(4, 2, 1.0), 3).await.unwrap();
        assert_eq!(results.documents.len(), 3);
        assert!(results.scores[0] >= results.scores[1]);
        assert!(results.scores[1] >= results.scores[2]);
        assert_eq!(results.documents[0], "gamma");
    }

    #[tokio::test]
    async fn search_top_k_limits_results() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        for i in 0..10usize {
            let mut emb = vec![0.0f32; 16];
            emb[i % 16] = 1.0;
            add(vec![(make_doc(&format!("d{i}"), &format!("doc{i}")), emb)]).await.unwrap();
        }
        let results = search(unit(16, 0, 1.0), 3).await.unwrap();
        assert!(results.documents.len() <= 3);
    }

    #[tokio::test]
    async fn add_empty_docs_vec_is_noop() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        add(vec![]).await.unwrap();
        let (docs, embs) = stats().await;
        assert_eq!(docs, 0);
        assert_eq!(embs, 0);
    }

    #[tokio::test]
    async fn stats_reflects_added_documents() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        add(vec![
            (make_doc("s1", "content1"), unit(4, 0, 1.0)),
            (make_doc("s2", "content2"), unit(4, 1, 1.0)),
        ]).await.unwrap();
        let (docs, embs) = stats().await;
        assert_eq!(docs, 2);
        assert_eq!(embs, 2);
    }

    #[tokio::test]
    async fn add_duplicate_id_overwrites_document() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        add(vec![(make_doc("dup", "original"), unit(4, 0, 1.0))]).await.unwrap();
        add(vec![(make_doc("dup", "updated"),  unit(4, 0, 1.0))]).await.unwrap();
        let (docs, _) = stats().await;
        assert_eq!(docs, 1);
        let results = search(unit(4, 0, 1.0), 1).await.unwrap();
        assert_eq!(results.documents[0], "updated");
    }

    #[tokio::test]
    async fn search_scores_are_clamped_non_negative() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        add(vec![(make_doc("neg", "content"), unit(4, 0, 1.0))]).await.unwrap();
        let results = search(unit(4, 1, 1.0), 1).await.unwrap();
        for score in &results.scores {
            assert!(*score >= 0.0, "score should be non-negative, got {score}");
        }
    }
}
