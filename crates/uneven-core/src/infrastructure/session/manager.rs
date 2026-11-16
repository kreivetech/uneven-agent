use std::path::{Path, PathBuf};
use crate::domain::session::{UnevenSession, PendingDiff, DiffStatus, KnowledgeSession, BrainSession, WatcherSessionState, PentesterSession, WriteLockSession};

pub struct SessionManager {
    pub session_path: PathBuf,
}

impl SessionManager {
    pub fn new(project_root: &Path) -> Self {
        Self {
            session_path: project_root.join(".uneven/session.json"),
        }
    }

    pub fn read(&self) -> Option<UnevenSession> {
        if !self.session_path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&self.session_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn write(&self, session: &UnevenSession) -> Result<(), std::io::Error> {
        let parent = self.session_path.parent().expect("Invalid session path");
        std::fs::create_dir_all(parent)?;
        
        let tmp_path = self.session_path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(session)?;
        std::fs::write(&tmp_path, content)?;
        
        std::fs::rename(&tmp_path, &self.session_path)?;
        Ok(())
    }

    pub fn add_pending_diff(&self, diff: PendingDiff) -> Result<(), std::io::Error> {
        let mut session = self.read().unwrap_or_else(|| self.default_session());
        session.pending_diffs.push(diff);
        self.write(&session)
    }

    pub fn resolve_diff(&self, id: &str, status: DiffStatus) -> Result<(), std::io::Error> {
        if let Some(mut session) = self.read()
            && let Some(diff) = session.pending_diffs.iter_mut().find(|d| d.id == id) {
                diff.status = status;
                self.write(&session)?;
            }
        Ok(())
    }

    pub fn clear_resolved_diffs(&self) -> Result<(), std::io::Error> {
        if let Some(mut session) = self.read() {
            session.pending_diffs.retain(|d| d.status == DiffStatus::Pending);
            self.write(&session)?;
        }
        Ok(())
    }

    pub fn get_pending_diffs(&self) -> Vec<PendingDiff> {
        let mut diffs = self.read().map(|s| s.pending_diffs).unwrap_or_default();
        diffs.reverse();
        diffs
    }

    pub fn default_session(&self) -> UnevenSession {
        let root_str = self.session_path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        UnevenSession {
            session_id: uuid::Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
            uneven_version: "1.12.0".to_string(),
            project_root: root_str,
            mode: "development".to_string(),
            auto_fix: true,
            confirm_before_fix: false,
            alert_only: false,
            knowledge: KnowledgeSession {
                last_indexed_at: None,
                indexed_dirs: vec![],
                indexed_files: vec![],
                indexed_urls: vec![],
                db_connected: false,
                staleness: "unknown".to_string(),
            },
            brain: BrainSession {
                provider: "local".to_string(),
                model: "llama3.2".to_string(),
                ready: false,
            },
            watcher: WatcherSessionState {
                active: false,
                terminal: None,
                pid: None,
            },
            pentester: PentesterSession {
                enabled: false,
                mode: None,
                target: None,
            },
            write_lock: WriteLockSession {
                active: false,
                locked_by: None,
                locked_at: None,
                lock_id: None,
                pid: None,
                timeout_ms: 10000,
                debounce_ms: 50,
            },
            pending_diffs: vec![],
        }
    }
}
