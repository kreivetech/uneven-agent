use crate::UnevenError;
use notify::{Watcher, RecursiveMode, Config, EventKind};
use std::path::Path;


pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
}

impl FileWatcher {
    pub fn new<F>(on_change: F) -> Result<Self, UnevenError> 
    where 
        F: Fn(String) + Send + 'static 
    {
        let (tx, rx) = std::sync::mpsc::channel();

        let watcher = notify::RecommendedWatcher::new(tx, Config::default())
            .map_err(|e| UnevenError::WatcherError(format!("Failed to create watcher: {}", e)))?;

        std::thread::spawn(move || {
            for res in rx {
                match res {
                    Ok(event) => {
                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                                for path in event.paths {
                                    if let Some(path_str) = path.to_str() {
                                        on_change(path_str.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => tracing::error!("watch error: {:?}", e),
                }
            }
        });

        Ok(Self { watcher })
    }

    pub fn watch(&mut self, path: &str) -> Result<(), UnevenError> {
        let p = Path::new(path);
        self.watcher.watch(p, RecursiveMode::Recursive)
            .map_err(|e| UnevenError::WatcherError(format!("Failed to watch {}: {}", path, e)))?;
        Ok(())
    }
}
