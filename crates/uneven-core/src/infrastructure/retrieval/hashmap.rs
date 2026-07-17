use super::*;
use super::types::{StoredDocument, RetrievalResult};
use super::utils::cosine_sim;
use std::collections::{HashMap, BinaryHeap};
use tokio::sync::RwLock;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use crate::UnevenError;

/// In-memory state.
#[derive(Default, Serialize, Deserialize)]
pub struct VectorStoreState {
    pub embeddings: HashMap<String, Vec<f32>>,
    pub documents:  HashMap<String, StoredDocument>,
}

pub static STORE: Lazy<RwLock<VectorStoreState>> =
    Lazy::new(|| RwLock::new(VectorStoreState::default()));

pub async fn init() -> Result<(), UnevenError> {
    tracing::info!("Vector store (HashMap backend): initializing");

    std::fs::create_dir_all(get_store_path(STORE_DIR))
        .map_err(|e| UnevenError::RetrievalError(format!("create_dir_all failed: {e}")))?;

    let mut store = STORE.write().await;

    // Try binary format first
    let bin_path = get_store_path(STORE_BIN);
    if bin_path.exists() {
        match tokio::fs::read(&bin_path).await {
            Ok(bytes) => match bincode::deserialize::<VectorStoreState>(&bytes) {
                Ok(loaded) => {
                    let n = loaded.documents.len();
                    *store = loaded;
                    tracing::info!("Vector store loaded from binary ({n} documents)");
                    return Ok(());
                }
                Err(e) => tracing::warn!("Binary store corrupt — trying JSON migration: {e}"),
            },
            Err(e) => tracing::warn!("Failed to read binary store: {e}"),
        }
    }

    // Migrate legacy JSON
    let json_path = get_store_path(STORE_JSON);
    if json_path.exists() {
        tracing::info!("Migrating vector store from JSON to binary");
        if let Ok(json) = tokio::fs::read_to_string(&json_path).await {
            #[derive(Deserialize)]
            struct Legacy {
                embeddings: HashMap<String, Vec<f32>>,
                documents:  HashMap<String, StoredDocument>,
            }
            if let Ok(legacy) = serde_json::from_str::<Legacy>(&json) {
                let n = legacy.documents.len();
                store.embeddings = legacy.embeddings;
                store.documents  = legacy.documents;
                tracing::info!("Migrated {n} documents from JSON");
                if write_binary(&store).await.is_ok() {
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
    for (doc, emb) in docs {
        if emb.is_empty() { continue; }
        store.embeddings.insert(doc.id.clone(), emb);
        store.documents.insert(doc.id.clone(), doc);
    }
    Ok(())
}

pub async fn flush() -> Result<(), UnevenError> {
    let store = STORE.read().await;
    write_binary(&store).await
}

pub async fn search(query: Vec<f32>, top_k: usize) -> Result<RetrievalResult, UnevenError> {
    if query.is_empty() {
        return Err(UnevenError::RetrievalError("Empty query embedding".into()));
    }
    let store = STORE.read().await;
    if store.embeddings.is_empty() {
        return Ok(RetrievalResult { documents: vec![], scores: vec![] });
    }

    // O(n log k) min-heap keyed by f32::total_cmp — safe for NaN and -0.0
    #[derive(PartialEq)]
    struct Entry(f32, String);
    impl Eq for Entry {}
    impl Ord for Entry {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering { self.0.total_cmp(&o.0) }
    }
    impl PartialOrd for Entry {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) }
    }

    let mut heap: BinaryHeap<std::cmp::Reverse<Entry>> =
        BinaryHeap::with_capacity(top_k + 1);

    for (id, emb) in &store.embeddings {
        let score = cosine_sim(&query, emb).max(0.0);
        let entry = std::cmp::Reverse(Entry(score, id.clone()));
        if heap.len() < top_k {
            heap.push(entry);
        } else if let Some(worst) = heap.peek() {
            if score.total_cmp(&worst.0.0) == std::cmp::Ordering::Greater {
                heap.pop();
                heap.push(entry);
            }
        }
    }

    let mut top: Vec<Entry> = heap.into_iter().map(|r| r.0).collect();
    top.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));

    let mut documents = Vec::with_capacity(top.len());
    let mut scores    = Vec::with_capacity(top.len());
    for Entry(score, id) in top {
        if let Some(doc) = store.documents.get(&id) {
            documents.push(doc.content.clone());
            scores.push(score);
        }
    }
    Ok(RetrievalResult { documents, scores })
}

pub async fn clear() -> Result<(), UnevenError> {
    let mut store = STORE.write().await;
    store.embeddings.clear();
    store.documents.clear();
    for p in [STORE_BIN, STORE_JSON] {
        let _ = tokio::fs::remove_file(get_store_path(p)).await;
    }
    Ok(())
}

pub async fn stats() -> (usize, usize) {
    let s = STORE.read().await;
    (s.documents.len(), s.embeddings.len())
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
        // Three orthogonal unit vectors. Query aligned with idx=2 → doc-c is best.
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
        // Only one entry with the same id
        let (docs, _) = stats().await;
        assert_eq!(docs, 1);
        // Content reflects the latest write
        let results = search(unit(4, 0, 1.0), 1).await.unwrap();
        assert_eq!(results.documents[0], "updated");
    }

    #[tokio::test]
    async fn search_scores_are_clamped_non_negative() {
        let _g = STORE_MUTEX.lock().await;
        clear().await.unwrap();
        add(vec![(make_doc("neg", "content"), unit(4, 0, 1.0))]).await.unwrap();
        // Query on orthogonal axis — cosine similarity = 0, not negative
        let results = search(unit(4, 1, 1.0), 1).await.unwrap();
        for score in &results.scores {
            assert!(*score >= 0.0, "score should be non-negative, got {score}");
        }
    }
}

async fn write_binary(store: &VectorStoreState) -> Result<(), UnevenError> {
    let bytes = bincode::serialize(store)
        .map_err(|e| UnevenError::RetrievalError(format!("Bincode serialize: {e}")))?;
    let bin_path = get_store_path(STORE_BIN);
    let tmp = format!("{}.tmp", bin_path.display());
    tokio::fs::write(&tmp, &bytes).await
        .map_err(|e| UnevenError::RetrievalError(format!("Write tmp: {e}")))?;
    tokio::fs::rename(&tmp, &bin_path).await
        .map_err(|e| UnevenError::RetrievalError(format!("Rename: {e}")))?;
    tracing::debug!("Vector store flushed ({} docs, {} bytes)",
        store.documents.len(), bytes.len());
    Ok(())
}
