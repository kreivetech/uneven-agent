use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use chrono::Utc;
use crate::UnevenError;

const STATE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileRecord {
    pub path: String,          // absolute path
    pub hash: String,          // SHA-256 of file content
    pub size: u64,             // bytes
    #[serde(rename = "indexedAt")]
    pub indexed_at: String,     // ISO 8601
    #[serde(rename = "chunkIds")]
    pub chunk_ids: Vec<String>,    // vector store chunk IDs for this file
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexState {
    pub version: u32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub files: HashMap<String, FileRecord>,   // keyed by absolute path
}

pub struct IncrementalIndex {
    project_root: PathBuf,
    state_path: PathBuf,
    pub state: IndexState,
}

impl IncrementalIndex {
    pub fn new<P: AsRef<Path>>(project_root: P) -> Self {
        let root_buf = project_root.as_ref().to_path_buf();
        let state_path = crate::infrastructure::utils::paths::project_cache_dir(&root_buf).join("index-state.json");
        let mut idx = Self {
            project_root: root_buf,
            state_path,
            state: IndexState {
                version: STATE_VERSION,
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
                files: HashMap::new(),
            },
        };
        idx.state = idx.load_state();
        idx
    }

    fn load_state(&self) -> IndexState {
        if !self.state_path.exists() {
            return self.empty_state();
        }
        match fs::read_to_string(&self.state_path) {
            Ok(raw) => {
                match serde_json::from_str::<IndexState>(&raw) {
                    Ok(parsed) => {
                        if parsed.version != STATE_VERSION {
                            self.empty_state()
                        } else {
                            parsed
                        }
                    }
                    Err(_) => self.empty_state()
                }
            }
            Err(_) => self.empty_state()
        }
    }

    fn empty_state(&self) -> IndexState {
        IndexState {
            version: STATE_VERSION,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            files: HashMap::new(),
        }
    }

    pub fn status(&self, file_path: &str) -> String {
        let abs = match fs::canonicalize(file_path) {
            Ok(p) => p,
            Err(_) => return "deleted".to_string(),
        };
        let abs_str = abs.to_string_lossy().to_string();

        let record = match self.state.files.get(&abs_str) {
            Some(r) => r,
            None => return "new".to_string(),
        };

        let current_hash = match self.hash_file(&abs) {
            Ok(h) => h,
            Err(_) => return "deleted".to_string(),
        };

        if current_hash == record.hash {
            "unchanged".to_string()
        } else {
            "modified".to_string()
        }
    }

    pub fn needs_indexing(&self, file_paths: &[String]) -> Vec<String> {
        let mut result = Vec::new();
        for fp in file_paths {
            let s = self.status(fp);
            if s == "new" || s == "modified" {
                result.push(fp.clone());
            }
        }
        result
    }

    pub fn mark_indexed_with_hash(&mut self, file_path: &str, hash: String, size: u64, chunk_ids: Vec<String>) {
        let abs = fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
        let abs_str = abs.to_string_lossy().to_string();

        self.state.files.insert(abs_str.clone(), FileRecord {
            path: abs_str,
            hash,
            size,
            indexed_at: Utc::now().to_rfc3339(),
            chunk_ids,
        });
        self.state.updated_at = Utc::now().to_rfc3339();
    }

    pub fn remove_file(&mut self, file_path: &str) -> Vec<String> {
        let abs = fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
        let abs_str = abs.to_string_lossy().to_string();

        if let Some(record) = self.state.files.remove(&abs_str) {
            self.state.updated_at = Utc::now().to_rfc3339();
            record.chunk_ids
        } else {
            Vec::new()
        }
    }

    pub fn prune_deleted_files(&mut self) -> Result<usize, UnevenError> {
        let mut pruned = 0;
        let mut to_remove = Vec::new();

        for abs_path in self.state.files.keys() {
            if !Path::new(abs_path).exists() {
                to_remove.push(abs_path.clone());
            }
        }

        for path in to_remove {
            self.remove_file(&path);
            pruned += 1;
        }

        if pruned > 0 {
            self.save()?;
        }

        Ok(pruned)
    }

    pub fn get_chunk_ids(&self, file_path: &str) -> Vec<String> {
        let abs = fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
        let abs_str = abs.to_string_lossy().to_string();
        self.state.files.get(&abs_str).map(|r| r.chunk_ids.clone()).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), UnevenError> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.state_path.with_extension("tmp");
        let content = serde_json::to_string_pretty(&self.state)?;
        fs::write(&tmp, content)?;
        fs::rename(tmp, &self.state_path)?;

        // Update file-list-cache.json to keep it in sync with the index state
        let cache_path = self.state_path.with_file_name("file-list-cache.json");
        let mut relative_paths: Vec<String> = self.state.files.keys()
            .filter_map(|abs_path| {
                Path::new(abs_path).strip_prefix(&self.project_root).ok()
                    .map(|p| p.to_string_lossy().to_string())
            })
            .collect();
        relative_paths.sort();
        relative_paths.dedup();
        if let Ok(json_content) = serde_json::to_string(&relative_paths) {
            let tmp_cache = cache_path.with_extension("tmp");
            if fs::write(&tmp_cache, json_content).is_ok() {
                let _ = fs::rename(tmp_cache, cache_path);
            }
        }

        Ok(())
    }

    pub fn clear(&mut self) {
        self.state = self.empty_state();
        let _ = fs::remove_file(&self.state_path);
    }

    fn hash_file(&self, path: &Path) -> Result<String, std::io::Error> {
        let content = fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        Ok(hasher.finalize().into_iter().map(|b| format!("{:02x}", b)).collect())
    }

    pub fn hash_buffer(buf: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(buf);
        hasher.finalize().into_iter().map(|b| format!("{:02x}", b)).collect()
    }
}
