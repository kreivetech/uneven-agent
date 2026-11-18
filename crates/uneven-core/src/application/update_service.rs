use crate::application::license_service::get_home_dir;
use crate::UnevenError;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command as SysCommand;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
}

#[derive(Debug, Deserialize)]
struct IdeVersionResponse {
    version: String,
}

fn parse_semver_tuple(v: &str) -> Option<(u64, u64, u64)> {
    let clean = v.trim().trim_start_matches('v');
    let mut parts = clean.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch_raw = parts.next().unwrap_or("0");
    let patch_digits: String = patch_raw.chars().take_while(|c| c.is_ascii_digit()).collect();
    let patch = patch_digits.parse::<u64>().ok().unwrap_or(0);
    Some((major, minor, patch))
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    match (parse_semver_tuple(latest), parse_semver_tuple(current)) {
        (Some(l), Some(c)) => l > c,
        _ => latest.trim() != current.trim() && !latest.trim().is_empty(),
    }
}

pub fn get_target_platform() -> Result<&'static str, UnevenError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("linux-x64"),
        ("linux", "aarch64") => Ok("linux-arm64"),
        ("macos", "aarch64") => Ok("darwin-arm64"),
        ("macos", "x86_64") => Ok("darwin-x64"),
        ("windows", "x86_64") => Ok("win32-x64"),
        (os, arch) => Err(UnevenError::ConfigError(format!(
            "[UE-02000001] Platform {}-{} is not supported for self-update.",
            os, arch
        ))),
    }
}

fn last_check_path() -> Option<PathBuf> {
    let mut p = get_home_dir()?;
    p.push(".config");
    p.push("uneven-ai");
    p.push("last_ide_update_check");
    Some(p)
}

/// Centralized IDE update service using obfuscated edge endpoints (`obfstr`).
pub struct UpdateService;

impl UpdateService {
    /// Checks `/api/ide/version` (rate-limited locally to once every 300s) and returns
    /// `Some(IdeUpdateInfo)` when a newer IDE version is available.
    pub async fn check_ide_update(current_version: &str) -> Result<Option<IdeUpdateInfo>, UnevenError> {
        let now = chrono::Utc::now().timestamp();
        if let Some(ref check_file) = last_check_path() {
            if check_file.exists()
                && let Ok(raw) = fs::read_to_string(check_file)
                && let Ok(last) = raw.trim().parse::<i64>()
                && (0..300).contains(&(now - last))
            {
                return Ok(None);
            }
            if let Some(parent) = check_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(check_file, now.to_string());
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|_| {
                UnevenError::ConfigError(
                    "[UE-03000001] Could not initialize update check client.".to_string(),
                )
            })?;

        let res = client
            .get(obfstr::obfstr!("https://edge.uneven.dev/api/ide/version"))
            .send()
            .await
            .map_err(|_| {
                UnevenError::ConfigError(
                    "[UE-03000001] Could not reach update server.".to_string(),
                )
            })?;

        if !res.status().is_success() {
            return Err(UnevenError::ConfigError(
                "[UE-03000001] Update server returned an error.".to_string(),
            ));
        }

        let data: IdeVersionResponse = res.json().await.map_err(|_| {
            UnevenError::ConfigError(
                "[UE-03000002] Invalid version response from update server.".to_string(),
            )
        })?;

        if is_newer_version(&data.version, current_version) {
            Ok(Some(IdeUpdateInfo {
                current_version: current_version.to_string(),
                latest_version: data.version,
            }))
        } else {
            Ok(None)
        }
    }

    /// Downloads the IDE binary from `/api/ide/download` and replaces the current executable
    /// (or downloads the OS installer to `~/.cache/uneven-ide/` if the binary path is read-only).
    pub async fn apply_ide_update() -> Result<String, UnevenError> {
        let target = get_target_platform()?;
        let current_exe = std::env::current_exe().map_err(|_| {
            UnevenError::ConfigError("[UE-03000001] Could not locate current executable.".to_string())
        })?;

        let exe_dir = current_exe.parent().ok_or_else(|| {
            UnevenError::ConfigError("[UE-03000001] Could not resolve executable directory.".to_string())
        })?;

        // Check if current executable directory is writable (standalone binary) vs system-managed (.deb/snap)
        let probe_path = exe_dir.join(".uneven_update_probe");
        let can_self_replace = fs::write(&probe_path, b"").is_ok();
        let _ = fs::remove_file(&probe_path);

        let download_url = if can_self_replace {
            format!(
                "{}?target={}&format=bin&reason=update",
                obfstr::obfstr!("https://edge.uneven.dev/api/ide/download"),
                target
            )
        } else {
            format!(
                "{}?target={}&reason=update",
                obfstr::obfstr!("https://edge.uneven.dev/api/ide/download"),
                target
            )
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|_| {
                UnevenError::ConfigError(
                    "[UE-03000001] Could not initialize update download client.".to_string(),
                )
            })?;

        let res = client
            .get(&download_url)
            .send()
            .await
            .map_err(|_| {
                UnevenError::ConfigError(
                    "[UE-03000001] Failed to download IDE update.".to_string(),
                )
            })?;

        if !res.status().is_success() {
            return Err(UnevenError::ConfigError(
                "[UE-03000001] Update download failed on server.".to_string(),
            ));
        }

        let filename = res
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split("filename=\"").nth(1))
            .and_then(|s| s.split('"').next())
            .unwrap_or("uneven-ide-update")
            .to_string();

        let bytes = res.bytes().await.map_err(|_| {
            UnevenError::ConfigError("[UE-03000001] Failed while reading update stream.".to_string())
        })?;

        if can_self_replace {
            let new_path = current_exe.with_extension("new");
            let old_path = current_exe.with_extension("old");
            fs::write(&new_path, &bytes)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&new_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&new_path, perms)?;
            }

            if old_path.exists() {
                let _ = fs::remove_file(&old_path);
            }
            fs::rename(&current_exe, &old_path)?;
            if let Err(e) = fs::rename(&new_path, &current_exe) {
                let _ = fs::rename(&old_path, &current_exe);
                return Err(UnevenError::IoError(e));
            }
            let _ = fs::remove_file(&old_path);

            let _ = SysCommand::new(&current_exe).spawn();
            Ok("Updated. Restarting...".to_string())
        } else {
            let mut cache_dir = get_home_dir().unwrap_or_else(|| PathBuf::from("."));
            cache_dir.push(".cache");
            cache_dir.push("uneven-ide");
            fs::create_dir_all(&cache_dir)?;
            let pkg_path = cache_dir.join(&filename);
            fs::write(&pkg_path, &bytes)?;

            #[cfg(target_os = "linux")]
            let _ = SysCommand::new("xdg-open").arg(&pkg_path).spawn();
            #[cfg(target_os = "macos")]
            let _ = SysCommand::new("open").arg(&pkg_path).spawn();
            #[cfg(target_os = "windows")]
            let _ = SysCommand::new("cmd")
                .args(["/C", "start", "", &pkg_path.to_string_lossy()])
                .spawn();

            Ok(format!("Installer downloaded to {}", pkg_path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.2.4", "0.2.3"));
        assert!(is_newer_version("0.3.0", "0.2.9"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.2.3", "0.2.3"));
        assert!(!is_newer_version("0.2.2", "0.2.3"));
    }
}
