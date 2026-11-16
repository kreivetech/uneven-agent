use anyhow::{Result, anyhow};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::path::PathBuf;
use crate::ui;

#[derive(serde::Deserialize)]
struct VersionResponse {
    version: String,
}

pub fn get_target_platform(os: &str, arch: &str) -> Result<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Ok("linux-x64"),
        ("linux", "aarch64") => Ok("linux-arm64"),
        ("macos", "aarch64") => Ok("darwin-arm64"),
        ("macos", "x86_64") => Ok("darwin-x64"),
        ("windows", "x86_64") => Ok("win32-x64"),
        _ => Err(anyhow!("Platform {}-{} is not officially supported for update", os, arch)),
    }
}

pub async fn run() -> Result<()> {
    // 1. Detect target platform key
    let target = get_target_platform(std::env::consts::OS, std::env::consts::ARCH)?;

    // Local Rate Limiting: 5 minutes (300 seconds) between checks
    let home_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let uneven_dir = std::path::Path::new(&home_dir).join(".uneven");
    let check_file = uneven_dir.join("last_update_check");
    let now = chrono::Utc::now().timestamp();

    if check_file.exists()
        && let Ok(content) = std::fs::read_to_string(&check_file)
            && let Ok(last_check) = content.trim().parse::<i64>() {
                let elapsed = now - last_check;
                if (0..300).contains(&elapsed) {
                    let wait_secs = 300 - elapsed;
                    ui::print_warning(&format!(
                        "You checked for updates recently. Please wait {} seconds before checking again.",
                        wait_secs
                    ));
                    return Ok(());
                }
            }

    println!("Checking for Uneven Agent updates...");

    // Save current check timestamp
    let _ = std::fs::create_dir_all(&uneven_dir);
    let _ = std::fs::write(&check_file, now.to_string());

    let client = reqwest::Client::new();
    let version_res = client.get(obfstr::obfstr!("https://edge.uneven.dev/api/version"))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to connect to version API: {}", e))?;

    if !version_res.status().is_success() {
        return Err(anyhow!("Version check API returned status code: {}", version_res.status()));
    }

    let data: VersionResponse = version_res.json()
        .await
        .map_err(|e| anyhow!("Failed to parse version response JSON: {}", e))?;

    let current_version = env!("CARGO_PKG_VERSION");
    let latest_version = data.version.clone();

    if current_version == latest_version {
        ui::print_success(&format!("Uneven Agent is already up to date (v{}).", current_version));
        return Ok(());
    }

    let current_exe = std::env::current_exe()
        .map_err(|e| anyhow!("Failed to determine current executable path: {}", e))?;

    // 2. Identify and delete companion native modules from legacy installations
    let exe_dir = current_exe.parent()
        .ok_or_else(|| anyhow!("Failed to get executable directory"))?;
    
    let native_module_paths = [
        exe_dir.join("uneven-core.uneven"),
        exe_dir.join(format!("uneven-core-{}.uneven", target)),
        exe_dir.join("uneven_core.node"),
        exe_dir.join("uneven.uneven"),
    ];
    
    for path in &native_module_paths {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }

    println!("Downloading update v{} for {}...", latest_version, target);

    // 3. Download binary asset
    let bin_url = format!("{}?target={}&asset=bin&reason=update", obfstr::obfstr!("https://edge.uneven.dev/api/download"), target);
    let tmp_exe = PathBuf::from(format!("{}.tmp", current_exe.to_string_lossy()));
    download_file(&client, &bin_url, &tmp_exe, "Binary").await?;

    // 4. Apply executable permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&tmp_exe, std::fs::Permissions::from_mode(0o755)) {
            warn_cleanup(&tmp_exe, &format!("Failed to set executable permissions: {}", e));
            return Err(anyhow!("Failed to set binary permissions: {}", e));
        }
    }

    // 5. Apply replacement atomically
    println!("Applying atomic update...");
    
    if cfg!(target_os = "windows") {
        let old_exe = PathBuf::from(format!("{}.old", current_exe.to_string_lossy()));
        if old_exe.exists() {
            let _ = std::fs::remove_file(&old_exe);
        }
        if let Err(e) = std::fs::rename(&current_exe, &old_exe) {
            warn_cleanup(&tmp_exe, &format!("Failed to rename running executable: {}", e));
            return Err(anyhow!("Failed to replace running binary: {}", e));
        }
        if let Err(e) = std::fs::rename(&tmp_exe, &current_exe) {
            // Rollback Windows rename
            let _ = std::fs::rename(&old_exe, &current_exe);
            warn_cleanup(&tmp_exe, &format!("Failed to move new executable: {}", e));
            return Err(anyhow!("Failed to apply update: {}", e));
        }
    } else {
        if let Err(e) = std::fs::rename(&tmp_exe, &current_exe) {
            warn_cleanup(&tmp_exe, &format!("Failed to replace binary: {}", e));
            return Err(anyhow!("Failed to apply update: {}", e));
        }
    }

    ui::print_success(&format!("Successfully updated Uneven Agent to v{}!", latest_version));
    Ok(())
}

async fn download_file(client: &reqwest::Client, url: &str, dest_path: &PathBuf, label: &str) -> Result<()> {
    let device_id = uneven_core::infrastructure::utils::device::get_or_create_device_id();
    let res = client.get(url)
        .header("X-Device-ID", &device_id)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to download {}: {}", label, e))?;

    if !res.status().is_success() {
        return Err(anyhow!("Failed to download {}, status code: {}", label, res.status()));
    }

    let total_size = res.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
            .progress_chars("#>-")
    );
    pb.set_message(format!("Downloading {}", label));

    let mut file = std::fs::File::create(dest_path)
        .map_err(|e| anyhow!("Failed to create temporary file {}: {}", dest_path.display(), e))?;
        
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = match item {
            Ok(c) => c,
            Err(e) => {
                pb.finish_and_clear();
                return Err(anyhow!("Error while downloading chunk: {}", e));
            }
        };
        if let Err(e) = file.write_all(&chunk) {
            pb.finish_and_clear();
            return Err(anyhow!("Error writing to temporary file: {}", e));
        }
        let new_pos = std::cmp::min(pb.position() + chunk.len() as u64, total_size);
        pb.set_position(new_pos);
    }

    pb.finish_with_message(format!("Downloaded {}", label));
    Ok(())
}

fn warn_cleanup(tmp_exe: &PathBuf, err: &str) {
    ui::print_warning(err);
    let _ = std::fs::remove_file(tmp_exe);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_target_platform() {
        assert_eq!(get_target_platform("linux", "x86_64").unwrap(), "linux-x64");
        assert_eq!(get_target_platform("linux", "aarch64").unwrap(), "linux-arm64");
        assert_eq!(get_target_platform("macos", "aarch64").unwrap(), "darwin-arm64");
        assert_eq!(get_target_platform("macos", "x86_64").unwrap(), "darwin-x64");
        assert_eq!(get_target_platform("windows", "x86_64").unwrap(), "win32-x64");
        assert!(get_target_platform("freebsd", "x86_64").is_err());
    }
}
