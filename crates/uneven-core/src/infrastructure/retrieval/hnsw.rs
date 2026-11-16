use super::*;
use super::types::{StoredDocument, RetrievalResult};
use std::sync::Mutex as StdMutex;
use std::collections::HashMap;
use tokio::sync::RwLock;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};
use crate::UnevenError;

/// Document metadata and key mappings
#[derive(Default, Serialize, Deserialize)]
pub struct StoreMeta {
    pub key_to_id: HashMap<u64, String>,
    pub id_to_key: HashMap<String, u64>,
    pub documents: HashMap<String, StoredDocument>,
    pub next_key: u64,
}

pub struct HnswStore {
    pub index: StdMutex<Index>,
    pub meta:  StoreMeta,
}

fn lock_index(index: &StdMutex<Index>) -> std::sync::MutexGuard<'_, Index> {
    index.lock().unwrap_or_else(|e| {
        tracing::warn!("Retrieval: HNSW index lock poisoned — recovering");
        e.into_inner()
    })
}

const DIMS: usize = 384;

fn make_index() -> Result<Index, UnevenError> {
    let opts = IndexOptions {
        dimensions: DIMS,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        connectivity: 16,
        expansion_add: 128,
        ..Default::default()
    };
    Index::new(&opts)
        .map_err(|e| UnevenError::RetrievalError(format!("usearch Index::new: {e}")))
}

pub static STORE: Lazy<RwLock<HnswStore>> = Lazy::new(|| {
    let index = match make_index() {
        Ok(idx) => idx,
        Err(e) => {
            tracing::error!("HNSW index construction failed at startup: {e}");
            // Retry with bare-minimum options before giving up.
            let opts = IndexOptions {
                dimensions: DIMS,
                metric: MetricKind::Cos,
                quantization: ScalarKind::F32,
                ..Default::default()
            };
            Index::new(&opts).expect("fallback usearch Index::new also failed — cannot continue")
        }
    };
    RwLock::new(HnswStore {
        index: StdMutex::new(index),
        meta:  StoreMeta::default(),
    })
});

pub async fn init() -> Result<(), UnevenError> {
    tracing::info!("Vector store (HNSW backend): initializing");

    std::fs::create_dir_all(get_store_path(STORE_DIR))
        .map_err(|e| UnevenError::RetrievalError(format!("create_dir_all: {e}")))?;

    let mut store = STORE.write().await;

    let usearch_exists = get_store_path(STORE_USEARCH).exists();
    let meta_exists    = get_store_path(STORE_META_BIN).exists();

    if usearch_exists && meta_exists {
        match try_load_hnsw(&mut store).await {
            Ok(n) => {
                tracing::info!("HNSW store loaded ({n} documents)");
                return Ok(());
            }
            Err(e) => tracing::warn!("HNSW load failed — rebuilding: {e}"),
        }
    }

    // Migrate legacy JSON
    let json_path = get_store_path(STORE_JSON);
    if json_path.exists() {
        tracing::info!("Migrating vector store from JSON to HNSW");
        if let Ok(json) = tokio::fs::read_to_string(&json_path).await {
            #[derive(Deserialize)]
            struct Legacy {
                embeddings: HashMap<String, Vec<f32>>,
                documents:  HashMap<String, StoredDocument>,
            }
            if let Ok(legacy) = serde_json::from_str::<Legacy>(&json) {
                let n = legacy.documents.len();
                {
                    let HnswStore { index, meta } = &mut *store;
                    let mut idx = lock_index(index);
                    *idx = make_index()?;
                    idx.reserve(n.max(1024))
                        .map_err(|e| UnevenError::RetrievalError(format!("reserve: {e}")))?;
                    for (id, emb) in &legacy.embeddings {
                        let key = meta.next_key;
                        meta.next_key += 1;
                        meta.key_to_id.insert(key, id.clone());
                        meta.id_to_key.insert(id.clone(), key);
                        if let Err(e) = idx.add(key, emb) {
                            tracing::warn!("HNSW add failed for {id}: {e}");
                        }
                    }
                }
                store.meta.documents = legacy.documents;
                if write_hnsw(&store).await.is_ok() {
                    let _ = tokio::fs::remove_file(&json_path).await;
                    tracing::info!("Migration complete — legacy JSON removed");
                }
                return Ok(());
            }
        }
    }

    // Migrate from bincode
    let bin_path = get_store_path(STORE_BIN);
    if bin_path.exists() {
        tracing::info!("Migrating vector store from bincode to HNSW");
        if let Ok(bytes) = tokio::fs::read(&bin_path).await {
            #[derive(Deserialize)]
            struct BincodeStore {
                embeddings: HashMap<String, Vec<f32>>,
                documents:  HashMap<String, StoredDocument>,
            }
            if let Ok(old) = postcard::from_bytes::<BincodeStore>(&bytes) {
                let n = old.embeddings.len();
                {
                    let HnswStore { index, meta } = &mut *store;
                    let mut idx = lock_index(index);
                    *idx = make_index()?;
                    idx.reserve(n.max(1024))
                        .map_err(|e| UnevenError::RetrievalError(format!("reserve: {e}")))?;
                    for (id, emb) in &old.embeddings {
                        let key = meta.next_key;
                        meta.next_key += 1;
                        meta.key_to_id.insert(key, id.clone());
                        meta.id_to_key.insert(id.clone(), key);
                        if let Err(e) = idx.add(key, emb) {
                            tracing::warn!("HNSW add failed for {id}: {e}");
                        }
                    }
                }
                store.meta.documents = old.documents;
                if write_hnsw(&store).await.is_ok() {
                    let _ = tokio::fs::remove_file(&bin_path).await;
                    tracing::info!("Migration complete — bincode removed");
                }
                return Ok(());
            }
        }
    }

    tracing::info!("HNSW store initialized empty");
    Ok(())
}

pub async fn add(docs: Vec<(StoredDocument, Vec<f32>)>) -> Result<(), UnevenError> {
    if docs.is_empty() { return Ok(()); }
    let mut store = STORE.write().await;
    {
        let HnswStore { index, meta } = &mut *store;
        let idx = lock_index(index);
        let needed = meta.next_key as usize + docs.len();
        idx.reserve(needed)
            .map_err(|e| UnevenError::RetrievalError(format!("reserve: {e}")))?;
 
        for (doc, emb) in &docs {
            if emb.is_empty() { continue; }
            let key = if let Some(&k) = meta.id_to_key.get(&doc.id) {
                k
            } else {
                let k = meta.next_key;
                meta.next_key += 1;
                meta.key_to_id.insert(k, doc.id.clone());
                meta.id_to_key.insert(doc.id.clone(), k);
                k
            };
            if let Err(e) = idx.add(key, emb) {
                tracing::warn!("HNSW add failed for {}: {e}", doc.id);
            }
        }
    }
    for (doc, _) in docs {
        store.meta.documents.insert(doc.id.clone(), doc);
    }
    Ok(())
}

pub async fn flush() -> Result<(), UnevenError> {
    let store = STORE.read().await;
    write_hnsw(&store).await
}

pub async fn search(query: Vec<f32>, top_k: usize) -> Result<RetrievalResult, UnevenError> {
    if query.is_empty() {
        return Err(UnevenError::RetrievalError("Empty query embedding".into()));
    }
    let store = STORE.read().await;
    if store.meta.documents.is_empty() {
        return Ok(RetrievalResult { documents: vec![], scores: vec![] });
    }

    let results = {
        let idx = lock_index(&store.index);
        idx.search(&query, top_k)
            .map_err(|e| UnevenError::RetrievalError(format!("HNSW search: {e}")))?
    };

    let mut documents = Vec::with_capacity(results.keys.len());
    let mut scores    = Vec::with_capacity(results.keys.len());

    for (key, dist) in results.keys.iter().zip(results.distances.iter()) {
        if let Some(id) = store.meta.key_to_id.get(key)
            && let Some(doc) = store.meta.documents.get(id) {
                let similarity = (1.0 - dist).max(0.0_f32);
                documents.push(doc.content.clone());
                scores.push(similarity);
            }
    }

    Ok(RetrievalResult { documents, scores })
}

pub async fn clear() -> Result<(), UnevenError> {
    let mut store = STORE.write().await;
    store.meta = StoreMeta::default();
    {
        let mut idx = lock_index(&store.index);
        *idx = make_index()?;
    }
    for p in [STORE_USEARCH, STORE_USEARCH_TMP, STORE_META_BIN, STORE_JSON, STORE_BIN] {
        let _ = tokio::fs::remove_file(get_store_path(p)).await;
    }
    Ok(())
}

pub async fn stats() -> (usize, usize) {
    let s = STORE.read().await;
    let n = s.meta.documents.len();
    let indexed = lock_index(&s.index).size();
    (n, indexed)
}

async fn try_load_hnsw(store: &mut HnswStore) -> Result<usize, UnevenError> {
    let meta_path = get_store_path(STORE_META_BIN);
    let meta_bytes = tokio::fs::read(&meta_path).await
        .map_err(|e| UnevenError::RetrievalError(format!("read meta: {e}")))?;
    let meta = postcard::from_bytes::<StoreMeta>(&meta_bytes)
        .map_err(|e| UnevenError::RetrievalError(format!("deserialize meta: {e}")))?;
    let n = meta.documents.len();

    let usearch_path = get_store_path(STORE_USEARCH);
    let usearch_str = usearch_path.to_str()
        .ok_or_else(|| UnevenError::RetrievalError("Invalid usearch path".into()))?;
        
    let new_index = make_index()?;
    new_index.load(usearch_str)
        .map_err(|e| UnevenError::RetrievalError(format!("usearch load: {e}")))?;

    store.meta = meta;
    *lock_index(&store.index) = new_index;
    Ok(n)
}

async fn write_hnsw(store: &HnswStore) -> Result<(), UnevenError> {
    let usearch_path = get_store_path(STORE_USEARCH);
        
    let usearch_tmp = get_store_path(STORE_USEARCH_TMP);
    let usearch_tmp_str = usearch_tmp.to_str()
        .ok_or_else(|| UnevenError::RetrievalError("Invalid usearch tmp path".into()))?;
    
    lock_index(&store.index)
        .save(usearch_tmp_str)
        .map_err(|e| UnevenError::RetrievalError(format!("usearch save tmp: {e}")))?;
    tokio::fs::rename(&usearch_tmp, &usearch_path).await
        .map_err(|e| UnevenError::RetrievalError(format!("rename usearch: {e}")))?;

    let meta_bytes = postcard::to_stdvec(&store.meta)
        .map_err(|e| UnevenError::RetrievalError(format!("postcard serialize meta: {e}")))?;
    let meta_path = get_store_path(STORE_META_BIN);
    let tmp = format!("{}.tmp", meta_path.display());
    tokio::fs::write(&tmp, &meta_bytes).await
        .map_err(|e| UnevenError::RetrievalError(format!("write meta tmp: {e}")))?;
    tokio::fs::rename(&tmp, &meta_path).await
        .map_err(|e| UnevenError::RetrievalError(format!("rename meta: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    use once_cell::sync::Lazy;

    static TEST_MUTEX: Lazy<StdMutex<()>> = Lazy::new(|| StdMutex::new(()));

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_hnsw_lifecycle() {
        let _guard = TEST_MUTEX.lock().unwrap();
        // Start clean
        clear().await.unwrap();
        
        let doc = StoredDocument::new("test1", "test.ts", "const x = 1;", "code");
        
        let mut emb = vec![0.0f32; DIMS];
        emb[0] = 1.0; 

        add(vec![(doc, emb)]).await.unwrap();
        
        let (n, indexed) = stats().await;
        assert_eq!(n, 1);
        assert_eq!(indexed, 1);

        // Search with exact match
        let query = vec![1.0f32; DIMS]; // This will have high cosine similarity with [1, 0, ...]
        let res = search(query, 1).await.unwrap();
        assert_eq!(res.documents.len(), 1);
        assert_eq!(res.documents[0], "const x = 1;");
        
        // Search with empty query should fail
        assert!(search(vec![], 1).await.is_err());

        // Stats reflect clearing
        clear().await.unwrap();
        let (n2, indexed2) = stats().await;
        assert_eq!(n2, 0);
        assert_eq!(indexed2, 0);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_hnsw_multi_add() {
        let _guard = TEST_MUTEX.lock().unwrap();
        clear().await.unwrap();
        
        let docs = (0..5).map(|i| {
            (
                StoredDocument::new(format!("doc{}", i), "f.ts", format!("content {}", i), "test"),
                {
                    let mut v = vec![0.0f32; DIMS];
                    v[i] = 1.0;
                    v
                }
            )
        }).collect();

        add(docs).await.unwrap();
        let (n, _) = stats().await;
        assert_eq!(n, 5);
        
        clear().await.unwrap();
    }
}
