//! Session cache and short-term memory module
//!
//! Stores recent interactions, embeddings, and file contents in memory
//! to speed up context assembly and avoid redundant I/O or LLM calls.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub data: T,
    pub expires_at: DateTime<Utc>,
}

pub struct SessionCache {
    /// File content cache: path -> content
    pub files: RwLock<HashMap<String, CacheEntry<String>>>,
    /// Embedding cache: text_hash -> embedding
    pub embeddings: RwLock<HashMap<u64, CacheEntry<Vec<f32>>>>,
    /// Recent context: turn_id -> context_string
    pub context_history: RwLock<HashMap<String, CacheEntry<String>>>,
}

pub static SESSION_CACHE: Lazy<Arc<SessionCache>> = Lazy::new(|| {
    Arc::new(SessionCache {
        files: RwLock::new(HashMap::new()),
        embeddings: RwLock::new(HashMap::new()),
        context_history: RwLock::new(HashMap::new()),
    })
});

impl SessionCache {
    pub fn get_file(&self, path: &str) -> Option<String> {
        let files = self.files.read().ok()?;
        let entry = files.get(path)?;
        if entry.expires_at > Utc::now() {
            Some(entry.data.clone())
        } else {
            None
        }
    }

    pub fn set_file(&self, path: String, content: String, ttl_secs: i64) {
        if let Ok(mut files) = self.files.write() {
            let now = Utc::now();
            files.retain(|_, entry| entry.expires_at > now);
            files.insert(path, CacheEntry {
                data: content,
                expires_at: now + chrono::Duration::seconds(ttl_secs),
            });
        }
    }

    pub fn evict_expired(&self) {
        let now = Utc::now();
        if let Ok(mut files) = self.files.write() { files.retain(|_, e| e.expires_at > now); }
        if let Ok(mut embs) = self.embeddings.write() { embs.retain(|_, e| e.expires_at > now); }
        if let Ok(mut ctx) = self.context_history.write() { ctx.retain(|_, e| e.expires_at > now); }
    }

    pub fn clear(&self) {
        if let Ok(mut files) = self.files.write() { files.clear(); }
        if let Ok(mut embs) = self.embeddings.write() { embs.clear(); }
        if let Ok(mut ctx) = self.context_history.write() { ctx.clear(); }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_file_cache_lifecycle() {
        let cache = SessionCache {
            files: RwLock::new(HashMap::new()),
            embeddings: RwLock::new(HashMap::new()),
            context_history: RwLock::new(HashMap::new()),
        };

        cache.set_file("test.txt".into(), "hello".into(), 1); // 1s TTL
        assert_eq!(cache.get_file("test.txt"), Some("hello".into()));

        thread::sleep(Duration::from_secs(2));
        assert_eq!(cache.get_file("test.txt"), None);
    }

    #[test]
    fn test_active_memory_eviction() {
        let cache = SessionCache {
            files: RwLock::new(HashMap::new()),
            embeddings: RwLock::new(HashMap::new()),
            context_history: RwLock::new(HashMap::new()),
        };

        cache.set_file("f1.txt".into(), "v1".into(), 1);
        cache.set_file("f2.txt".into(), "v2".into(), 10);
        
        assert_eq!(cache.files.read().unwrap().len(), 2);
        
        thread::sleep(Duration::from_secs(2));
        
        // Active prune via evict_expired
        cache.evict_expired();
        
        let files = cache.files.read().unwrap();
        assert_eq!(files.len(), 1);
        assert!(files.contains_key("f2.txt"));
    }
}

