use crate::domain::UnevenError;
use std::fs;
use std::path::Path;

/// Migrates legacy project structures to the new unified and branded `.uneven` format.
///
/// Ports the behavior of TS migration.ts:
/// 1. Moves root `models/` directory to `.uneven/models/`
/// 2. Deletes legacy `.timoro/` directory
/// 3. Cleans up root pollution: moves files from `debug/` and `warn/` to `.uneven/logs/debug/` and `.uneven/logs/warn/`, then removes empty source directories.
pub fn migrate_legacy_structure<P: AsRef<Path>>(root: P) -> Result<(), UnevenError> {
    let root = root.as_ref();

    // 1. Move root models/ to .uneven/models/
    let old_models = root.join("models");
    let new_models = root.join(".uneven").join("models");
    if old_models.is_dir() && !new_models.exists() {
        if let Some(parent) = new_models.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Err(e) = fs::rename(&old_models, &new_models) {
            tracing::warn!("Failed to migrate legacy models/ directory: {}", e);
        } else {
            tracing::info!("Migrated: models/ moved to .uneven/models/");
        }
    }

    // 2. Remove legacy .timoro/
    let timoro_dir = root.join(".timoro");
    if timoro_dir.is_dir() {
        if let Err(e) = fs::remove_dir_all(&timoro_dir) {
            tracing::warn!("Failed to remove legacy .timoro/ folder: {}", e);
        } else {
            tracing::info!("Cleaned up: legacy .timoro/ folder removed");
        }
    }

    // 3. Clean up debug and warn folders in root
    let pollution = ["debug", "warn"];
    for dir in &pollution {
        let dir_path = root.join(dir);
        if !dir_path.is_dir() {
            continue;
        }

        // Read directory entries
        let mut is_empty = true;
        if let Ok(entries) = fs::read_dir(&dir_path) {
            let files: Vec<_> = entries.filter_map(Result::ok).collect();
            if !files.is_empty() {
                is_empty = false;
                let target_log_dir = root.join(".uneven").join("logs").join(dir);
                if let Err(e) = fs::create_dir_all(&target_log_dir) {
                    tracing::warn!("Failed to create target log directory {:?}: {}", target_log_dir, e);
                    continue;
                }

                let mut all_moved = true;
                for entry in files {
                    let file_path = entry.path();
                    if let Some(file_name) = file_path.file_name() {
                        let target_path = target_log_dir.join(file_name);
                        if let Err(e) = fs::rename(&file_path, &target_path) {
                            tracing::warn!("Failed to move file {:?} to {:?}: {}", file_path, target_path, e);
                            all_moved = false;
                        }
                    }
                }

                if all_moved {
                    if let Err(e) = fs::remove_dir(&dir_path) {
                        tracing::warn!("Failed to remove directory {:?}: {}", dir_path, e);
                    } else {
                        tracing::info!("Cleaned up: legacy {}/ folder moved to .uneven/logs/{}", dir, dir);
                    }
                }
            }
        }

        if is_empty
            && let Err(e) = fs::remove_dir(&dir_path) {
                tracing::warn!("Failed to remove empty directory {:?}: {}", dir_path, e);
            }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_migrate_models() {
        let root = tempdir().unwrap();
        let old_models = root.path().join("models");
        fs::create_dir(&old_models).unwrap();
        fs::write(old_models.join("model.bin"), b"model_data").unwrap();

        migrate_legacy_structure(root.path()).unwrap();

        assert!(!old_models.exists());
        let new_models = root.path().join(".uneven").join("models");
        assert!(new_models.exists());
        assert_eq!(fs::read(new_models.join("model.bin")).unwrap(), b"model_data");
    }

    #[test]
    fn test_remove_timoro() {
        let root = tempdir().unwrap();
        let timoro = root.path().join(".timoro");
        fs::create_dir(&timoro).unwrap();
        fs::write(timoro.join("data.json"), b"{}").unwrap();

        migrate_legacy_structure(root.path()).unwrap();

        assert!(!timoro.exists());
    }

    #[test]
    fn test_clean_pollution_empty() {
        let root = tempdir().unwrap();
        let debug_dir = root.path().join("debug");
        fs::create_dir(&debug_dir).unwrap();

        migrate_legacy_structure(root.path()).unwrap();

        assert!(!debug_dir.exists());
    }

    #[test]
    fn test_clean_pollution_with_files() {
        let root = tempdir().unwrap();
        let debug_dir = root.path().join("debug");
        fs::create_dir(&debug_dir).unwrap();
        fs::write(debug_dir.join("test.log"), b"log_content").unwrap();

        migrate_legacy_structure(root.path()).unwrap();

        assert!(!debug_dir.exists());
        let new_log = root.path().join(".uneven").join("logs").join("debug").join("test.log");
        assert!(new_log.exists());
        assert_eq!(fs::read(new_log).unwrap(), b"log_content");
    }
}
