//! History Context Optimizer
//!
//! Re-exports context optimization, tagging, and session primitives powered by `contextcraft`,
//! and provides uneven-specific integration for workspace file maps and patches.

pub use contextcraft::{
    delete_session, extract_content_between, list_sessions, load_session, optimize_context,
    save_session, update_content_between, BlockStrategy, ChatTurn, ContextBlock, ContextError,
    ContextOptimizer, HistoryItem, DEFAULT_LAW_END, DEFAULT_LAW_START, DEFAULT_MAP_END,
    DEFAULT_MAP_START,
};

pub const LAW_START: &str = DEFAULT_LAW_START;
pub const LAW_END: &str = DEFAULT_LAW_END;
pub const MAP_START: &str = DEFAULT_MAP_START;
pub const MAP_END: &str = DEFAULT_MAP_END;

use crate::domain::files_map::FilesMapIndex;
use crate::infrastructure::patch::FileObject;
use crate::UnevenConfig;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct BrainTurn {
    pub role: String,
    pub content: String,
}

impl HistoryItem for BrainTurn {
    fn get_role(&self) -> &str {
        &self.role
    }
    fn get_content(&self) -> &str {
        &self.content
    }
    fn set_content(&mut self, content: String) {
        self.content = content;
    }
}

/// Loads the cached `FilesMapIndex` from `.uneven/cache/files-map-groups.json` or builds and caches it.
pub fn load_or_build_files_map(root: &Path) -> FilesMapIndex {
    let cache_dir = crate::infrastructure::utils::paths::project_cache_dir(root);
    let cache_file = cache_dir.join("files-map-groups.json");

    if cache_file.is_file()
        && let Ok(raw) = std::fs::read_to_string(&cache_file)
        && let Ok(idx) = serde_json::from_str::<FilesMapIndex>(&raw)
        && idx.root == root
    {
        return idx;
    }

    let idx = FilesMapIndex::build(root, None);
    let _ = std::fs::create_dir_all(&cache_dir);
    if let Ok(json) = serde_json::to_string(&idx) {
        let _ = std::fs::write(&cache_file, json);
    }
    idx
}

/// Surgically refreshes only the 2-level directory groups affected by `rel_paths`.
pub fn refresh_files_map_paths(root: &Path, rel_paths: &[String]) {
    if rel_paths.is_empty() {
        return;
    }
    let mut idx = load_or_build_files_map(root);
    for rel in rel_paths {
        idx.refresh_path(rel);
    }
    let cache_dir = crate::infrastructure::utils::paths::project_cache_dir(root);
    let _ = std::fs::create_dir_all(&cache_dir);
    if let Ok(json) = serde_json::to_string(&idx) {
        let _ = std::fs::write(cache_dir.join("files-map-groups.json"), json);
    }
}

pub fn get_project_files_map(_config: &UnevenConfig, root: &Path) -> String {
    load_or_build_files_map(root).render_compact_index()
}

pub fn save_history_to_disk(
    history: &[BrainTurn],
    session_id: &str,
    root: &Path,
) -> std::io::Result<()> {
    let mut history_buf = history.to_vec();
    crate::application::ContextCompactorService::auto_compact(&mut history_buf);

    let history_cache_dir = crate::infrastructure::utils::paths::project_cache_dir(root);
    let history_file = history_cache_dir.join(format!("{}.json", session_id));
    save_session(&history_file, &history_buf).map_err(|e| match e {
        ContextError::Io(io_err) => io_err,
        other => std::io::Error::new(std::io::ErrorKind::Other, other),
    })
}

pub fn get_history_from_disk(session_id: &str, root: &Path) -> Option<Vec<BrainTurn>> {
    let history_cache_dir = crate::infrastructure::utils::paths::project_cache_dir(root);
    let history_file = history_cache_dir.join(format!("{}.json", session_id));
    load_session(&history_file).ok()
}

pub fn list_project_sessions(root: &Path) -> Vec<String> {
    let history_cache_dir = crate::infrastructure::utils::paths::project_cache_dir(root);
    list_sessions(&history_cache_dir).unwrap_or_default()
}

pub fn delete_history_from_disk(session_id: &str, root: &Path) -> std::io::Result<()> {
    let history_cache_dir = crate::infrastructure::utils::paths::project_cache_dir(root);
    let history_file = history_cache_dir.join(format!("{}.json", session_id));
    delete_session(&history_file).map_err(|e| match e {
        ContextError::Io(io_err) => io_err,
        other => std::io::Error::new(std::io::ErrorKind::Other, other),
    })
}

pub fn save_pending_patch_to_disk(
    patch: &FileObject,
    session_id: &str,
    root: &Path,
) -> std::io::Result<()> {
    let history_cache_dir = crate::infrastructure::utils::paths::project_cache_dir(root);
    let _ = std::fs::create_dir_all(&history_cache_dir);
    if let Ok(json) = serde_json::to_string_pretty(patch) {
        let patch_file = history_cache_dir.join(format!("{}.patch.json", session_id));
        std::fs::write(patch_file, json)?;
    }
    Ok(())
}

pub fn get_pending_patch_from_disk(session_id: &str, root: &Path) -> Option<FileObject> {
    let history_cache_dir = crate::infrastructure::utils::paths::project_cache_dir(root);
    let patch_file = history_cache_dir.join(format!("{}.patch.json", session_id));
    if let Ok(json) = std::fs::read_to_string(&patch_file) {
        serde_json::from_str(&json).ok()
    } else {
        None
    }
}

pub fn clear_pending_patch_from_disk(session_id: &str, root: &Path) -> std::io::Result<()> {
    let history_cache_dir = crate::infrastructure::utils::paths::project_cache_dir(root);
    let patch_file = history_cache_dir.join(format!("{}.patch.json", session_id));
    if patch_file.exists() {
        std::fs::remove_file(patch_file)?;
    }
    Ok(())
}
