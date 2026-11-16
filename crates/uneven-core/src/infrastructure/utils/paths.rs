use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn global_uneven_dir() -> PathBuf {
    home_dir().join(".uneven")
}

pub fn global_bin_dir() -> PathBuf {
    global_uneven_dir().join("bin")
}

pub fn binary_root_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        if parent.file_name().and_then(|n| n.to_str()) == Some("deps")
            && let Some(target_dir) = parent.parent()
        {
            return target_dir.to_path_buf();
        }
        return parent.to_path_buf();
    }
    global_bin_dir()
}

pub fn global_stats_path() -> PathBuf {
    let bin_dir = binary_root_dir();
    let stats_candidate = bin_dir.join("stats.json");
    if stats_candidate.exists() {
        return stats_candidate;
    }
    if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(&stats_candidate) {
        drop(f);
        return stats_candidate;
    }
    // Fallback to global ~/.uneven/stats.json
    let fallback = global_uneven_dir().join("stats.json");
    if let Some(parent) = fallback.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    fallback
}

pub fn global_models_dir() -> PathBuf {
    let old_dir = global_bin_dir();
    let new_dir = old_dir.join("models");

    // One-time lazy migration of legacy models from global bin/ to global bin/models/
    if old_dir.exists() && !new_dir.exists() {
        let has_old_models = old_dir.join("embeddings").exists()
            || std::fs::read_dir(&old_dir)
                .map(|rd| {
                    rd.filter_map(Result::ok)
                        .any(|entry| entry.path().extension().is_some_and(|ext| ext == "gguf"))
                })
                .unwrap_or(false);

        if has_old_models {
            let _ = std::fs::create_dir_all(&new_dir);

            // Move embeddings folder
            let old_emb = old_dir.join("embeddings");
            let new_emb = new_dir.join("embeddings");
            if old_emb.exists() {
                let _ = std::fs::rename(&old_emb, &new_emb);
            }

            // Move .gguf and tokenizer files
            if let Ok(entries) = std::fs::read_dir(&old_dir) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.is_file() {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        if name.ends_with(".gguf") || name.contains("tokenizer.json") {
                            let _ = std::fs::rename(&path, new_dir.join(&name));
                        }
                    }
                }
            }
        }
    }

    new_dir
}

pub fn project_run_dir() -> PathBuf {
    global_bin_dir().join("run")
}

pub fn project_hash<P: AsRef<Path>>(project_root: P) -> String {
    let path_ref = project_root.as_ref();
    let abs_path = path_ref
        .canonicalize()
        .unwrap_or_else(|_| path_ref.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(abs_path.to_string_lossy().as_bytes());
    let hex: String = hasher.finalize().into_iter().map(|b| format!("{:02x}", b)).collect();
    hex[..16].to_string()
}

pub fn project_pid_file<P: AsRef<Path>>(project_root: P, daemon_name: &str) -> PathBuf {
    let hash = project_hash(project_root);
    project_run_dir().join(format!("{}_{}.pid", hash, daemon_name))
}

pub fn project_lock_file<P: AsRef<Path>>(project_root: P) -> PathBuf {
    let hash = project_hash(project_root);
    project_run_dir().join(format!("{}.lock", hash))
}

pub fn project_cache_dir<P: AsRef<Path>>(project_root: P) -> PathBuf {
    let hash = project_hash(project_root);
    global_bin_dir().join("cache").join(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_project_hash_uniqueness() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        let hash1 = project_hash(dir1.path());
        let hash2 = project_hash(dir2.path());
        assert_ne!(hash1, hash2);
        assert_eq!(hash1.len(), 16);
    }
}
