use crate::domain::UnevenError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use chrono::{DateTime, Utc};


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LockData {
    pub pid: u32,
    pub command: String,
    pub started_at: String,
}

pub struct ProcessLock {
    lock_path: PathBuf,
    held: bool,
}

impl ProcessLock {
    pub fn new<P: AsRef<Path>>(project_root: P) -> Self {
        let lock_path = super::paths::project_lock_file(project_root);
        Self {
            lock_path,
            held: false,
        }
    }

    /// Tries to acquire the lock for the specified command.
    /// Returns Ok(None) on success.
    /// Returns Ok(Some(LockData)) if the lock already belongs to another active process.
    pub fn acquire(&mut self, command: &str) -> Result<Option<LockData>, UnevenError> {
        let parent = self.lock_path.parent().ok_or_else(|| {
            UnevenError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid lock path parent directory",
            ))
        })?;

        fs::create_dir_all(parent)?;

        // Check existing lock
        if self.lock_path.exists() {
            if let Some(existing) = self.read_lock() {
                if is_process_alive(existing.pid) {
                    return Ok(Some(existing));
                }
            }
            self.remove_lock();
        }

        let now: DateTime<Utc> = SystemTime::now().into();
        let data = LockData {
            pid: std::process::id(),
            command: command.to_string(),
            started_at: now.to_rfc3339(),
        };

        let content = serde_json::to_string_pretty(&data)?;

        // Atomic opening using create_new(true) (similar to O_CREAT | O_EXCL)
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.lock_path)
        {
            Ok(mut file) => {
                file.write_all(content.as_bytes())?;
                self.held = true;
                Ok(None)
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if let Some(existing) = self.read_lock() {
                    if is_process_alive(existing.pid) {
                        return Ok(Some(existing));
                    }
                }
                // Old owner inactive, try to remove and create again
                self.remove_lock();
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&self.lock_path)
                {
                    Ok(mut file) => {
                        file.write_all(content.as_bytes())?;
                        self.held = true;
                        Ok(None)
                    }
                    Err(err) => Err(UnevenError::IoError(err)),
                }
            }
            Err(e) => Err(UnevenError::IoError(e)),
        }
    }

    /// Releases the lock by removing the file.
    pub fn release(&mut self) {
        if self.held {
            self.held = false;
            self.remove_lock();
        }
    }

    fn read_lock(&self) -> Option<LockData> {
        let content = fs::read_to_string(&self.lock_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn remove_lock(&self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

impl Drop for ProcessLock {
    fn drop(&mut self) {
        self.release();
    }
}

/// Checks if a process is active in the system in a robust and cross-platform way.
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // On Unix/Linux/macOS, the command `kill -0 <pid>` checks for the process existence
        if let Ok(status) = std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
        {
            if status.success() {
                return true;
            }
        }
        // Fallback: check if the directory in /proc exists (works on the vast majority of Linux distros)
        Path::new(&format!("/proc/{}", pid)).exists()
    }

    #[cfg(windows)]
    {
        // On Windows, use `tasklist` with PID filtering
        if let Ok(output) = std::process::Command::new("tasklist")
            .arg("/FI")
            .arg(format!("PID eq {}", pid))
            .output()
        {
            if output.status.success() {
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                stdout_str.contains(&pid.to_string())
            } else {
                // If tasklist fails, assume active to be cautious
                true
            }
        } else {
            // If tasklist fails, assume active to be cautious
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_is_process_alive_with_current_pid() {
        let current_pid = std::process::id();
        assert!(is_process_alive(current_pid));
    }

    #[test]
    fn test_is_process_alive_with_dead_pid() {
        // PID very unlikely to be active
        let dead_pid = 999999;
        assert!(!is_process_alive(dead_pid));
    }

    #[test]
    fn test_acquire_and_release_lock() {
        let dir = tempdir().unwrap();
        let mut lock = ProcessLock::new(dir.path());

        // First acquisition succeeds
        let res = lock.acquire("test_command").unwrap();
        assert!(res.is_none());
        assert!(lock.lock_path.exists());

        // Second acquisition by same lock fails/detects itself because the PID is alive
        let mut another_lock = ProcessLock::new(dir.path());
        let res2 = another_lock.acquire("another_command").unwrap();
        assert!(res2.is_some());
        let owner = res2.unwrap();
        assert_eq!(owner.pid, std::process::id());
        assert_eq!(owner.command, "test_command");

        // Release first lock
        lock.release();
        assert!(!lock.lock_path.exists());

        // Now second acquisition succeeds
        let res3 = another_lock.acquire("another_command").unwrap();
        assert!(res3.is_none());
    }

    #[test]
    fn test_drop_cleanup_behavior() {
        let dir = tempdir().unwrap();
        
        {
            let mut lock = ProcessLock::new(dir.path());
            assert!(lock.acquire("test").unwrap().is_none());
            assert!(lock.lock_path.exists());
            // lock dropped here
        }

        // Lock file should be deleted automatically by Drop
        let path = dir.path().join(".uneven").join("uneven.lock");
        assert!(!path.exists());
    }

    #[test]
    fn test_stale_lock_recovery() {
        let dir = tempdir().unwrap();
        let lock_dir = dir.path().join(".uneven");
        fs::create_dir_all(&lock_dir).unwrap();
        
        // Write a stale lock file with a dead PID (e.g. 999999)
        let stale_data = LockData {
            pid: 999999,
            command: "stale_command".to_string(),
            started_at: "2026-05-23T00:00:00Z".to_string(),
        };
        let content = serde_json::to_string_pretty(&stale_data).unwrap();
        fs::write(lock_dir.join("uneven.lock"), content).unwrap();

        let mut lock = ProcessLock::new(dir.path());
        // Acquire should succeed and clean up the stale file
        let res = lock.acquire("new_command").unwrap();
        assert!(res.is_none());
        
        // Verifies the file has been updated with current PID
        let updated = lock.read_lock().unwrap();
        assert_eq!(updated.pid, std::process::id());
        assert_eq!(updated.command, "new_command");
    }
}

