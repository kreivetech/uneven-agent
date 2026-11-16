use std::path::{Path, PathBuf};
use std::fs;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::domain::error::UnevenError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub original_path: String,
    pub backup_path: String,
    pub reason: String,
    pub size: u64,
    pub checksum: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotManifest {
    pub snapshots: Vec<Snapshot>,
    pub max_snapshots: usize,
}

pub struct SnapshotManager {
    root: PathBuf,
    snapshots_dir: PathBuf,
    manifest_path: PathBuf,
}

impl SnapshotManager {
    pub fn new<P: AsRef<Path>>(project_root: P) -> Result<Self, UnevenError> {
        let root = project_root.as_ref().to_path_buf();
        let snapshots_dir = root.join(".uneven").join("snapshots");
        let manifest_path = snapshots_dir.join("manifest.json");

        if !snapshots_dir.exists() {
            fs::create_dir_all(&snapshots_dir)
                .map_err(|e| UnevenError::RetrievalError(format!("Failed to create snapshots dir: {e}")))?;
        }

        Ok(Self {
            root,
            snapshots_dir,
            manifest_path,
        })
    }

    pub fn take(&self, file_path: &str, reason: &str) -> Result<Snapshot, UnevenError> {
        let abs_path = if Path::new(file_path).is_absolute() {
            PathBuf::from(file_path)
        } else {
            self.root.join(file_path)
        };

        if !abs_path.exists() {
            return Err(UnevenError::RetrievalError(format!("File not found: {}", abs_path.display())));
        }

        let content = fs::read(&abs_path)
            .map_err(|e| UnevenError::RetrievalError(format!("Read failed: {e}")))?;

        // SHA256 Checksum
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let checksum: String = hasher.finalize().into_iter().map(|b| format!("{:02x}", b)).collect();

        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now();
        let filename = abs_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let short_hash = &checksum[..8];
        let backup_name = format!("{}-{}-{}.zst", timestamp.timestamp(), short_hash, filename);
        let backup_path = self.snapshots_dir.join(backup_name);

        // Compress and save
        let compressed = zstd::encode_all(&content[..], 3)
            .map_err(|e| UnevenError::RetrievalError(format!("Compression failed: {e}")))?;
        
        fs::write(&backup_path, compressed)
            .map_err(|e| UnevenError::RetrievalError(format!("Write failed: {e}")))?;

        let snapshot = Snapshot {
            id,
            created_at: timestamp,
            original_path: abs_path.to_string_lossy().to_string(),
            backup_path: backup_path.to_string_lossy().to_string(),
            reason: reason.to_string(),
            size: content.len() as u64,
            checksum,
        };

        // Update manifest
        let mut manifest = self.load_manifest()?;
        manifest.snapshots.push(snapshot.clone());
        self.prune(&mut manifest);
        self.save_manifest(&manifest)?;

        Ok(snapshot)
    }

    pub fn restore(&self, snapshot_id: &str) -> Result<Snapshot, UnevenError> {
        let manifest = self.load_manifest()?;
        let snapshot = manifest.snapshots.iter()
            .find(|s| s.id == snapshot_id)
            .ok_or_else(|| UnevenError::RetrievalError(format!("Snapshot not found: {snapshot_id}")))?;

        let backup_bytes = fs::read(&snapshot.backup_path)
            .map_err(|e| UnevenError::RetrievalError(format!("Read backup failed: {e}")))?;

        let content = zstd::decode_all(&backup_bytes[..])
            .map_err(|e| UnevenError::RetrievalError(format!("Decompression failed: {e}")))?;

        // Verify checksum
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let checksum: String = hasher.finalize().into_iter().map(|b| format!("{:02x}", b)).collect();

        if checksum != snapshot.checksum {
            return Err(UnevenError::RetrievalError("Checksum mismatch - corrupted backup".into()));
        }

        let target = Path::new(&snapshot.original_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).ok();
        }

        fs::write(target, content)
            .map_err(|e| UnevenError::RetrievalError(format!("Restore write failed: {e}")))?;

        Ok(snapshot.clone())
    }

    pub fn list(&self) -> Result<Vec<Snapshot>, UnevenError> {
        let manifest = self.load_manifest()?;
        let mut list = manifest.snapshots;
        list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(list)
    }

    pub fn delete(&self, snapshot_id: &str) -> Result<(), UnevenError> {
        let mut manifest = self.load_manifest()?;
        if let Some(idx) = manifest.snapshots.iter().position(|s| s.id == snapshot_id) {
            let s = &manifest.snapshots[idx];
            fs::remove_file(&s.backup_path).ok();
            manifest.snapshots.remove(idx);
            self.save_manifest(&manifest)?;
        }
        Ok(())
    }

    fn load_manifest(&self) -> Result<SnapshotManifest, UnevenError> {
        if !self.manifest_path.exists() {
            return Ok(SnapshotManifest {
                snapshots: Vec::new(),
                max_snapshots: 50,
            });
        }

        let content = fs::read_to_string(&self.manifest_path)
            .map_err(|e| UnevenError::RetrievalError(format!("Read manifest failed: {e}")))?;
        
        serde_json::from_str(&content)
            .map_err(|e| UnevenError::RetrievalError(format!("Parse manifest failed: {e}")))
    }

    fn save_manifest(&self, manifest: &SnapshotManifest) -> Result<(), UnevenError> {
        let content = serde_json::to_string_pretty(manifest)
            .map_err(|e| UnevenError::RetrievalError(format!("Serialize manifest failed: {e}")))?;
        
        let tmp = self.manifest_path.with_extension("tmp");
        fs::write(&tmp, content)
            .map_err(|e| UnevenError::RetrievalError(format!("Write manifest tmp failed: {e}")))?;
        
        fs::rename(tmp, &self.manifest_path)
            .map_err(|e| UnevenError::RetrievalError(format!("Atomic rename manifest failed: {e}")))
    }

    fn prune(&self, manifest: &mut SnapshotManifest) {
        if manifest.snapshots.len() <= manifest.max_snapshots {
            return;
        }

        manifest.snapshots.sort_by_key(|a| a.created_at);
        let count = manifest.snapshots.len() - manifest.max_snapshots;
        let to_remove: Vec<_> = manifest.snapshots.drain(..count).collect();

        for s in to_remove {
            fs::remove_file(s.backup_path).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_snapshot_take_and_restore() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let manager = SnapshotManager::new(root).unwrap();

        let file_path = root.join("test.txt");
        fs::write(&file_path, "hello snapshot").unwrap();

        // Take
        let snapshot = manager.take("test.txt", "unit test").unwrap();
        assert_eq!(snapshot.reason, "unit test");
        assert!(snapshot.backup_path.contains(".uneven/snapshots"));
        assert!(Path::new(&snapshot.backup_path).exists());

        // Modify original
        fs::write(&file_path, "modified content").unwrap();

        // Restore
        let restored = manager.restore(&snapshot.id).unwrap();
        assert_eq!(restored.id, snapshot.id);
        
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello snapshot");
    }

    #[test]
    fn test_snapshot_list_and_delete() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let manager = SnapshotManager::new(root).unwrap();

        let file_path = root.join("test.txt");
        fs::write(&file_path, "data").unwrap();

        let s1 = manager.take("test.txt", "first").unwrap();
        let s2 = manager.take("test.txt", "second").unwrap();

        let list = manager.list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, s2.id); // Newest first

        manager.delete(&s1.id).unwrap();
        let list_after = manager.list().unwrap();
        assert_eq!(list_after.len(), 1);
        assert_eq!(list_after[0].id, s2.id);
    }

    #[test]
    fn test_snapshot_pruning() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let manager = SnapshotManager::new(root).unwrap();
        
        // Setup low limit
        let mut manifest = manager.load_manifest().unwrap();
        manifest.max_snapshots = 2;
        manager.save_manifest(&manifest).unwrap();

        fs::write(root.join("f1.txt"), "1").unwrap();
        manager.take("f1.txt", "s1").unwrap();
        manager.take("f1.txt", "s2").unwrap();
        manager.take("f1.txt", "s3").unwrap(); // Should prune s1

        let list = manager.list().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|s| s.reason == "s2"));
        assert!(list.iter().any(|s| s.reason == "s3"));
        assert!(!list.iter().any(|s| s.reason == "s1"));
    }
}
