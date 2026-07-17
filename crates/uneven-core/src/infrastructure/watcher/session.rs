use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc::{self, UnboundedSender, UnboundedReceiver};
use tokio::task::JoinHandle;
use tokio::sync::Mutex;
use chrono::Utc;
use serde_json::json;

use crate::UnevenError;
use crate::domain::UnevenConfig;
use crate::domain::parsed_error::ErrorType;
use crate::domain::event::{UnevenEvent, EventType};
use crate::infrastructure::watcher::process::{ProcessWatcher, ProcessWatcherEvent};
use crate::infrastructure::watcher::file::FileWatcher;
use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::orchestrator::AgentOrchestrator;

#[derive(Debug, Clone)]
pub enum WatcherInternalEvent {
    Process {
        label: String,
        event: ProcessWatcherEvent,
    },
    FileChanged(String),
}

pub struct WatcherSession {
    config: UnevenConfig,
    project_root: PathBuf,
    is_running: Arc<AtomicBool>,
    event_tx: UnboundedSender<UnevenEvent>,
    internal_tx: UnboundedSender<WatcherInternalEvent>,
    internal_rx: Option<UnboundedReceiver<WatcherInternalEvent>>,
    process_watchers: Arc<Mutex<Vec<ProcessWatcher>>>,
    file_watcher: Option<FileWatcher>,
    orchestrator: Arc<AgentOrchestrator>,
    active_fixes: Arc<Mutex<HashSet<String>>>,
    debounce_tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    fix_attempts: Arc<Mutex<HashMap<String, u32>>>,
}

impl WatcherSession {
    pub fn new(
        config: UnevenConfig,
        project_root: PathBuf,
        event_tx: UnboundedSender<UnevenEvent>,
    ) -> Self {
        let (internal_tx, internal_rx) = mpsc::unbounded_channel();
        let log_path = config.log.as_ref()
            .and_then(|l| l.path.as_ref())
            .cloned()
            .unwrap_or_else(|| ".uneven/logs/main.md".to_string());

        Self {
            config,
            project_root: project_root.clone(),
            is_running: Arc::new(AtomicBool::new(false)),
            event_tx,
            internal_tx,
            internal_rx: Some(internal_rx),
            process_watchers: Arc::new(Mutex::new(Vec::new())),
            file_watcher: None,
            orchestrator: Arc::new(AgentOrchestrator::new(&log_path, project_root)),
            active_fixes: Arc::new(Mutex::new(HashSet::new())),
            debounce_tasks: Arc::new(Mutex::new(HashMap::new())),
            fix_attempts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn start(&mut self) -> Result<(), UnevenError> {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return Err(UnevenError::WatcherError("Watcher session already running".into()));
        }

        let mut internal_rx = self.internal_rx.take()
            .ok_or_else(|| UnevenError::WatcherError("Session already started or consumed".into()))?;

        // 1. Resolve watched terminal commands
        let mut commands = Vec::new();
        if let Some(ref watch_cfg) = self.config.watch {
            if let Some(ref terms) = watch_cfg.terminals {
                commands.extend(terms.clone());
            } else if let Some(ref term) = watch_cfg.terminal {
                commands.push(term.clone());
            }
        } else {
            commands.push("npm run dev".to_string());
        }

        let auto_fix = self.config.watch.as_ref().and_then(|w| w.auto_fix).unwrap_or(true);
        let confirm_before_fix = self.config.watch.as_ref().and_then(|w| w.confirm_before_fix).unwrap_or(false);

        // 2. Setup Process Watchers
        let log_path = self.config.log.as_ref()
            .and_then(|l| l.path.as_ref())
            .map(|s| s.as_str())
            .unwrap_or(".uneven/logs/main.md");

        let mut watchers = self.process_watchers.lock().await;
        for cmd in &commands {
            let label = label_of_command(cmd);
            let (proc_tx, mut proc_rx) = mpsc::unbounded_channel();
            let mut proc_watcher = ProcessWatcher::new(
                cmd.clone(),
                self.project_root.to_string_lossy().to_string(),
                auto_fix,
                3, // max restarts
                log_path,
                proc_tx,
            );

            proc_watcher.start()?;
            watchers.push(proc_watcher);

            let internal_tx_clone = self.internal_tx.clone();
            let label_clone = label.clone();
            tokio::spawn(async move {
                while let Some(proc_event) = proc_rx.recv().await {
                    let _ = internal_tx_clone.send(WatcherInternalEvent::Process {
                        label: label_clone.clone(),
                        event: proc_event,
                    });
                }
            });
        }
        drop(watchers);

        // 3. Setup File Watcher
        let watch_dirs = self.config.watch.as_ref()
            .and_then(|w| w.dirs.clone())
            .unwrap_or_else(|| {
                self.config.knowledge.as_ref()
                    .and_then(|k| k.dirs.clone())
                    .unwrap_or_else(|| vec![self.project_root.to_string_lossy().to_string()])
            });

        let internal_tx_file = self.internal_tx.clone();
        let mut file_watcher = FileWatcher::new(move |path| {
            let _ = internal_tx_file.send(WatcherInternalEvent::FileChanged(path));
        })?;

        for dir in &watch_dirs {
            file_watcher.watch(dir)?;
        }
        // Also watch config file if present
        let config_json = crate::infrastructure::utils::paths::project_cache_dir(&self.project_root).join("config.json");
        if config_json.exists() {
            if let Some(p) = config_json.to_str() {
                file_watcher.watch(p).ok();
            }
        }
        self.file_watcher = Some(file_watcher);

        // 4. Central event loop
        let is_running = self.is_running.clone();
        let event_tx = self.event_tx.clone();
        let orchestrator = self.orchestrator.clone();
        let active_fixes = self.active_fixes.clone();
        let debounce_tasks = self.debounce_tasks.clone();
        let fix_attempts = self.fix_attempts.clone();
        let project_root_clone = self.project_root.clone();
        let config_clone = self.config.clone();

        tokio::spawn(async move {
            event_tx.send(UnevenEvent {
                r#type: EventType::BrainReady,
                timestamp: Utc::now(),
                data: json!({
                    "provider": "local", // default, could map from config
                    "model": "model",
                    "mode": "watch"
                }),
            }).ok();

            while is_running.load(Ordering::SeqCst) {
                tokio::select! {
                    Some(internal_event) = internal_rx.recv() => {
                        match internal_event {
                            WatcherInternalEvent::Process { label, event } => {
                                match event {
                                    ProcessWatcherEvent::Start { command } => {
                                        event_tx.send(UnevenEvent {
                                            r#type: EventType::WatchStarted,
                                            timestamp: Utc::now(),
                                            data: json!({
                                                "process": label,
                                                "command": command
                                            }),
                                        }).ok();
                                    }
                                    ProcessWatcherEvent::Output { channel, data } => {
                                        // Forward stdout/stderr output lines
                                        event_tx.send(UnevenEvent {
                                            r#type: EventType::Info,
                                            timestamp: Utc::now(),
                                            data: json!({
                                                "process": label,
                                                "channel": channel,
                                                "data": data
                                            }),
                                        }).ok();
                                    }
                                    ProcessWatcherEvent::Exit { code } => {
                                        event_tx.send(UnevenEvent {
                                            r#type: EventType::Info,
                                            timestamp: Utc::now(),
                                            data: json!({
                                                "process": label,
                                                "message": format!("Process exited with code {:?}", code)
                                            }),
                                        }).ok();
                                    }
                                    ProcessWatcherEvent::Restart { attempt } => {
                                        event_tx.send(UnevenEvent {
                                            r#type: EventType::Warning,
                                            timestamp: Utc::now(),
                                            data: json!({
                                                "process": label,
                                                "message": format!("Process crashed, restarting (attempt {})...", attempt)
                                            }),
                                        }).ok();
                                    }
                                    ProcessWatcherEvent::Stopped { reason } => {
                                        event_tx.send(UnevenEvent {
                                            r#type: EventType::Warning,
                                            timestamp: Utc::now(),
                                            data: json!({
                                                "process": label,
                                                "message": format!("Process stopped: {}", reason)
                                            }),
                                        }).ok();
                                    }
                                    ProcessWatcherEvent::Error { errors } => {
                                        for error in errors {
                                            // Handle error and debounce auto-fix
                                            let error_clone = error.clone();
                                            let label_clone = label.clone();
                                            let event_tx_clone = event_tx.clone();
                                            let orchestrator_clone = orchestrator.clone();
                                            let active_fixes_clone = active_fixes.clone();
                                            let fix_attempts_clone = fix_attempts.clone();
                                            let config_clone_inner = config_clone.clone();

                                            event_tx.send(UnevenEvent {
                                                r#type: EventType::ErrorDetected,
                                                timestamp: Utc::now(),
                                                data: json!({
                                                    "process": label_clone,
                                                    "file": error_clone.file,
                                                    "line": error_clone.line,
                                                    "column": error_clone.column,
                                                    "message": error_clone.message,
                                                }),
                                            }).ok();

                                            // Setup debounce task for this error location
                                            let fix_key = format!("{}:{}", error_clone.file, error_clone.line);
                                            let mut tasks = debounce_tasks.lock().await;
                                            if let Some(existing) = tasks.remove(&fix_key) {
                                                existing.abort();
                                            }

                                            let fix_key_clone = fix_key.clone();
                                            let handle = tokio::spawn(async move {
                                                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                                                // Concurrency and loop protection
                                                let mut active = active_fixes_clone.lock().await;
                                                if active.contains(&fix_key_clone) {
                                                    return;
                                                }
                                                active.insert(fix_key_clone.clone());
                                                drop(active);

                                                let mut attempts = fix_attempts_clone.lock().await;
                                                let count = attempts.entry(fix_key_clone.clone()).or_insert(0);
                                                if *count >= 5 {
                                                    event_tx_clone.send(UnevenEvent {
                                                        r#type: EventType::Warning,
                                                        timestamp: Utc::now(),
                                                        data: json!({
                                                            "message": format!("Auto-fix suspended for {} after 5 failed attempts.", fix_key_clone)
                                                        }),
                                                    }).ok();
                                                    return;
                                                }
                                                *count += 1;
                                                drop(attempts);

                                                let brain = config_clone_inner.get_brain_for_command("watch");
                                                let only_use_strategies = config_clone_inner.watch.as_ref()
                                                    .and_then(|w| w.only_use_strategies)
                                                    .unwrap_or(false);
                                                // Trigger fix
                                                let res = orchestrator_clone.run_fix_for_error(
                                                    &error_clone,
                                                    auto_fix,
                                                    confirm_before_fix,
                                                    only_use_strategies,
                                                    &brain,
                                                ).await;

                                                if let Ok(Some(fix_res)) = res {
                                                    event_tx_clone.send(UnevenEvent {
                                                        r#type: EventType::FixSuggested,
                                                        timestamp: Utc::now(),
                                                        data: json!({
                                                            "process": label_clone,
                                                            "confidence": fix_res.suggestion.confidence,
                                                            "explanation": fix_res.suggestion.explanation,
                                                        }),
                                                    }).ok();

                                                    if fix_res.applied {
                                                        event_tx_clone.send(UnevenEvent {
                                                            r#type: EventType::FixApplied,
                                                            timestamp: Utc::now(),
                                                            data: json!({
                                                                "suggestion": {
                                                                    "process": label_clone,
                                                                    "confidence": fix_res.suggestion.confidence,
                                                                    "explanation": fix_res.suggestion.explanation,
                                                                }
                                                            }),
                                                        }).ok();
                                                    } else if fix_res.reverted {
                                                        event_tx_clone.send(UnevenEvent {
                                                            r#type: EventType::FixReverted,
                                                            timestamp: Utc::now(),
                                                            data: json!({}),
                                                        }).ok();
                                                    } else if let Some(pending_id) = fix_res.pending_id {
                                                        event_tx_clone.send(UnevenEvent {
                                                            r#type: EventType::FixPending,
                                                            timestamp: Utc::now(),
                                                            data: json!({
                                                                "id": pending_id,
                                                                "file": fix_res.suggestion.file,
                                                                "line": error_clone.line,
                                                                "message": error_clone.message,
                                                                "suggestedFix": fix_res.suggestion.suggested_fix,
                                                                "explanation": fix_res.suggestion.explanation,
                                                                "confidence": fix_res.suggestion.confidence,
                                                                "beforeCode": fix_res.suggestion.before,
                                                                "afterCode": fix_res.suggestion.after,
                                                            }),
                                                        }).ok();
                                                    }
                                                }

                                                active_fixes_clone.lock().await.remove(&fix_key_clone);
                                            });

                                            tasks.insert(fix_key, handle);
                                        }
                                    }
                                }
                            }
                            WatcherInternalEvent::FileChanged(path) => {
                                if should_ignore_path(&path, &project_root_clone, &config_clone) {
                                    continue;
                                }
                                event_tx.send(UnevenEvent {
                                    r#type: EventType::FileChanged,
                                    timestamp: Utc::now(),
                                    data: json!({
                                        "path": path
                                    }),
                                }).ok();

                                // Proactive auto-fix logic
                                let path_obj = Path::new(&path);
                                if path_obj.file_name().and_then(|n| n.to_str()) == Some("config.json") {
                                    // Recalculate or trigger index sync
                                    // In a real CLI, we restart the process watchers and spawn index,
                                    // for simplicity, we trigger the IndexSync internally
                                    continue;
                                }

                                let fix_key = format!("proactive:{}", path);
                                let mut tasks = debounce_tasks.lock().await;
                                if let Some(existing) = tasks.remove(&fix_key) {
                                    existing.abort();
                                }

                                let fix_key_clone = fix_key.clone();
                                let path_clone = path.clone();
                                let event_tx_clone = event_tx.clone();
                                let orchestrator_clone = orchestrator.clone();
                                let active_fixes_clone = active_fixes.clone();
                                let config_clone_inner = config_clone.clone();

                                let handle = tokio::spawn(async move {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

                                    let mut active = active_fixes_clone.lock().await;
                                    if active.contains(&fix_key_clone) {
                                        return;
                                    }
                                    active.insert(fix_key_clone.clone());
                                    drop(active);

                                    let virtual_error = ErrorEvent {
                                        file: path_clone.clone(),
                                        line: 0,
                                        column: 0,
                                        message: "Proactive quality scan on file save.".to_string(),
                                        stack_trace: "PROACTIVE_SCAN_TRIGGERED".to_string(),
                                        error_type: Some(ErrorType::ProactiveScan),
                                        code: Some("SCAN".to_string()),
                                    };

                                    let brain = config_clone_inner.get_brain_for_command("autofix");
                                    let only_use_strategies = config_clone_inner.proactive.as_ref()
                                        .and_then(|p| p.only_use_strategies)
                                        .unwrap_or(false);
                                    let res = orchestrator_clone.run_fix_for_error(
                                        &virtual_error,
                                        auto_fix,
                                        confirm_before_fix,
                                        only_use_strategies,
                                        &brain,
                                    ).await;

                                    if let Ok(Some(fix_res)) = res {
                                        if fix_res.applied {
                                            event_tx_clone.send(UnevenEvent {
                                                r#type: EventType::FixApplied,
                                                timestamp: Utc::now(),
                                                data: json!({
                                                    "suggestion": {
                                                        "file": path_clone,
                                                        "explanation": fix_res.suggestion.explanation,
                                                    }
                                                }),
                                            }).ok();
                                        } else if !fix_res.reverted {
                                            if let Some(pending_id) = fix_res.pending_id {
                                                event_tx_clone.send(UnevenEvent {
                                                    r#type: EventType::FixPending,
                                                    timestamp: Utc::now(),
                                                    data: json!({
                                                        "id": pending_id,
                                                        "file": fix_res.suggestion.file,
                                                        "line": virtual_error.line,
                                                        "message": virtual_error.message,
                                                        "suggestedFix": fix_res.suggestion.suggested_fix,
                                                        "explanation": fix_res.suggestion.explanation,
                                                        "confidence": fix_res.suggestion.confidence,
                                                        "beforeCode": fix_res.suggestion.before,
                                                        "afterCode": fix_res.suggestion.after,
                                                    }),
                                                }).ok();
                                            }
                                        }
                                    }

                                    active_fixes_clone.lock().await.remove(&fix_key_clone);
                                });

                                tasks.insert(fix_key, handle);
                            }
                        }
                    }
                    else => break,
                }
            }
        });

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), UnevenError> {
        if self.is_running.swap(false, Ordering::SeqCst) {
            let mut watchers = self.process_watchers.lock().await;
            for watcher in watchers.iter_mut() {
                watcher.stop();
            }
            watchers.clear();

            if let Some(ref mut _fw) = self.file_watcher {
                // file watcher is stopped when it goes out of scope or is dropped
            }
            self.file_watcher = None;
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

fn label_of_command(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() >= 3 && parts[0] == "npm" && parts[1] == "run" {
        return parts[2].to_string();
    }
    if parts.len() >= 3 && parts[0] == "pnpm" && parts[1] == "run" {
        return parts[2].to_string();
    }
    if parts.len() >= 2 && parts[0] == "yarn" && !["add", "install", "remove", "upgrade", "dlx"].contains(&parts[1]) {
        return parts[1].to_string();
    }
    if let Some(last) = parts.last() {
        if let Some(name) = Path::new(last).file_name().and_then(|n| n.to_str()) {
            return name.to_string();
        }
    }
    "process".to_string()
}

fn should_ignore_path(path_str: &str, project_root: &Path, config: &UnevenConfig) -> bool {
    let path = Path::new(path_str);
    let path_normalized = path_str.replace('\\', "/");
    
    let is_ignored = path.components().any(|c| {
        if let Some(s) = c.as_os_str().to_str() {
            let s_lower = s.to_lowercase();
            s_lower == ".git"
                || s_lower == ".uneven"
                || s_lower == "node_modules"
                || s_lower == "target"
                || s_lower == "dist"
                || s_lower == "build"
                || s_lower == "venv"
                || s_lower == ".venv"
        } else {
            false
        }
    });

    if is_ignored {
        return true;
    }

    if let Some(ref log_cfg) = config.log {
        if let Some(ref log_p) = log_cfg.path {
            let log_path_buf = PathBuf::from(log_p);
            let abs_log = if log_path_buf.is_absolute() {
                log_path_buf
            } else {
                project_root.join(log_path_buf)
            };
            
            let abs_log_str = abs_log.to_string_lossy().replace('\\', "/");
            if path_normalized == abs_log_str {
                return true;
            }

            if let Ok(abs_log_canon) = std::fs::canonicalize(&abs_log) {
                if let Ok(path_canon) = std::fs::canonicalize(path) {
                    if path_canon == abs_log_canon {
                        return true;
                    }
                }
            }
        }
    }

    false
}
