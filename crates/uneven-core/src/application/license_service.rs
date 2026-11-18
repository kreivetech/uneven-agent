use crate::UnevenError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use catfive::{number, schema, string, Validator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::Command as SysCommand;
use std::time::Duration;

const GRACE_EXTENSION_DAYS: i64 = 7;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LicenseCache {
    pub key: String,
    pub tier: String, // "pro" | "team"
    pub seats: usize,
    pub expires_at: i64,  // Unix timestamp, 0 = no expiry
    pub verified_at: i64, // Unix timestamp of last successful server check
    pub grace_until: i64, // Unix timestamp grace period deadline
    pub machine_id: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct KeyPayload {
    pub id: String,
    pub tier: String,
    pub seats: usize,
    pub issued: i64,
    pub expires: i64,
    pub eh: String,
}

#[derive(Debug, Deserialize)]
struct ServerActivationResult {
    valid: bool,
    tier: Option<String>,
    seats: Option<usize>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
    reason: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsageData {
    pub date: String,
    pub askf_count: usize,
    pub analyze_count: usize,
    #[serde(default)]
    pub session_timestamps: std::collections::HashMap<String, Vec<i64>>,
}

impl UsageData {
    pub fn new_empty(date: String) -> Self {
        Self {
            date,
            askf_count: 0,
            analyze_count: 0,
            session_timestamps: std::collections::HashMap::new(),
        }
    }
}

pub fn get_home_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Ok(path) = std::env::var("UNEVEN_TEST_HOME") {
            return Some(PathBuf::from(path));
        }
    }
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .map(PathBuf::from)
}

fn get_node_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

fn get_node_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    }
}

pub fn get_hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .or_else(|| {
            SysCommand::new("hostname")
                .output()
                .ok()
                .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown-host".to_string())
}

pub fn get_machine_id() -> String {
    let platform = get_node_platform();
    let arch = get_node_arch();
    let hostname = get_hostname();
    let home = get_home_dir()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(platform.as_bytes());
    hasher.update(arch.as_bytes());
    hasher.update(hostname.as_bytes());
    hasher.update(home.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn get_machine_label() -> String {
    format!(
        "{}-{} / {}",
        get_node_platform(),
        get_node_arch(),
        get_hostname()
    )
}

pub fn get_license_path() -> Option<PathBuf> {
    let mut path = get_home_dir()?;
    path.push(".config");
    path.push("uneven-ai");
    path.push("license.json");
    Some(path)
}

pub fn read_license() -> Option<LicenseCache> {
    let path = get_license_path()?;
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn write_license(data: &LicenseCache) -> Result<(), UnevenError> {
    let path = get_license_path()
        .ok_or_else(|| UnevenError::ConfigError("Home directory not resolved".to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    let content = serde_json::to_string_pretty(data)?;
    fs::write(&tmp, content)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn clear_license() {
    if let Some(path) = get_license_path() {
        let _ = fs::remove_file(path);
    }
}

pub fn parse_key(key: &str) -> Result<KeyPayload, &'static str> {
    if string().starts_with("UNVN-").validate(key).is_err() {
        return Err("malformed");
    }

    let parts: Vec<&str> = key.split('-').collect();
    if parts.len() < 4 {
        return Err("malformed");
    }

    let payload_b64 = parts[2..parts.len() - 1].join("-");

    let bytes = match URL_SAFE_NO_PAD.decode(&payload_b64) {
        Ok(b) => b,
        Err(_) => match base64::engine::general_purpose::URL_SAFE.decode(&payload_b64) {
            Ok(b) => b,
            Err(_) => return Err("malformed"),
        },
    };

    let payload_str = String::from_utf8(bytes).map_err(|_| "malformed")?;
    let payload: KeyPayload = serde_json::from_str(&payload_str).map_err(|_| "malformed")?;

    let payload_validator = schema::<KeyPayload>()
        .field("id", |p| &p.id, string().min(1))
        .field("tier", |p| &p.tier, string().in_list(&["pro", "team"]))
        .field("seats", |p| &p.seats, number().min(1));

    if let Err(errs) = payload_validator.validate(&payload) {
        let err_str = format!("{:?}", errs);
        if err_str.contains("tier") {
            return Err("unknown_tier");
        }
        return Err("malformed");
    }

    let now = chrono::Utc::now().timestamp();
    if payload.expires > 0 && payload.expires < now {
        return Err("expired");
    }

    Ok(payload)
}

pub fn get_usage_path() -> Option<PathBuf> {
    let mut path = get_home_dir()?;
    path.push(".config");
    path.push("uneven-ai");
    path.push("usage.json");
    Some(path)
}

pub fn read_usage() -> Option<UsageData> {
    let path = get_usage_path()?;
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn write_usage(data: &UsageData) -> Result<(), UnevenError> {
    let path = get_usage_path()
        .ok_or_else(|| UnevenError::ConfigError("Home directory not resolved".to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    let content = serde_json::to_string_pretty(data)?;
    fs::write(&tmp, content)?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// Centralized license & plan gating service shared by `uneven-core`, `uneven-cli`, and `uneven-ui`.
pub struct LicenseService;

impl LicenseService {
    /// Returns true if an active `pro` or `team` license is present.
    pub fn check_pro() -> bool {
        if let Some(license) = read_license() {
            let now = chrono::Utc::now().timestamp();
            let active =
                license.expires_at == 0 || now < license.expires_at || now < license.grace_until;
            active && (license.tier == "pro" || license.tier == "team")
        } else {
            false
        }
    }

    /// Requires an active `pro` (or `team`) license, returning `[UE-01000005]` otherwise.
    pub fn require_pro() -> Result<(), UnevenError> {
        if !Self::check_pro() {
            return Err(UnevenError::SecurityError(
                "[UE-01000005] This feature requires Uneven Pro. Upgrade at https://uneven.dev/pricing or activate your license key in Settings -> License.".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns true if an active `team` license is present.
    pub fn check_team() -> bool {
        if let Some(license) = read_license() {
            let now = chrono::Utc::now().timestamp();
            let active =
                license.expires_at == 0 || now < license.expires_at || now < license.grace_until;
            active && license.tier == "team"
        } else {
            false
        }
    }

    /// Requires an active `team` license, returning `[UE-01000006]` otherwise.
    pub fn require_team() -> Result<(), UnevenError> {
        if !Self::check_team() {
            return Err(UnevenError::SecurityError(
                "[UE-01000006] This feature requires Uneven Team. Upgrade at https://uneven.dev/pricing or activate your license key in Settings -> License.".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the active plan tier (`"free"`, `"pro"`, or `"team"`).
    pub fn active_tier() -> &'static str {
        if Self::check_team() {
            "team"
        } else if Self::check_pro() {
            "pro"
        } else {
            "free"
        }
    }

    /// Enforces sliding-window session limits for Free tier (`[UE-02000003]`).
    pub fn check_session_limit(
        command: &str,
        max_queries: usize,
        window_secs: i64,
    ) -> Result<(), UnevenError> {
        if Self::check_pro() {
            return Ok(());
        }

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut usage = read_usage().unwrap_or_else(|| UsageData::new_empty(today.clone()));

        if usage.date != today {
            usage.date = today;
            usage.askf_count = 0;
            usage.analyze_count = 0;
            usage.session_timestamps.clear();
        }

        let now = chrono::Utc::now().timestamp();
        let limit_boundary = now - window_secs;

        let timestamps = usage
            .session_timestamps
            .entry(command.to_string())
            .or_default();
        timestamps.retain(|&t| t > limit_boundary);

        if timestamps.len() >= max_queries {
            let hours = window_secs / 3600;
            return Err(UnevenError::SecurityError(format!(
                "[UE-02000003] Limit of {} scans/queries per {}-hour session reached for '{}' on Free license. Upgrade to Pro for unlimited use: https://uneven.dev/pricing",
                max_queries, hours, command
            )));
        }

        timestamps.push(now);
        write_usage(&usage)?;
        Ok(())
    }

    /// Enforces daily usage limits for Free tier (`[UE-02000003]`).
    pub fn check_and_increment_usage(command: &str) -> Result<usize, UnevenError> {
        if Self::check_pro() {
            return Ok(0);
        }

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut usage = read_usage().unwrap_or_else(|| UsageData::new_empty(today.clone()));

        if usage.date != today {
            usage.date = today;
            usage.askf_count = 0;
            usage.analyze_count = 0;
            usage.session_timestamps.clear();
        }

        let count = match command {
            "analyze" => {
                if usage.analyze_count >= 3 {
                    return Err(UnevenError::SecurityError(
                        "[UE-02000003] Daily limit of 3 queries for 'analyze' reached on Free license. Upgrade to Pro for unlimited use: https://uneven.dev/pricing".to_string(),
                    ));
                }
                usage.analyze_count += 1;
                usage.analyze_count
            }
            "askf" => 0,
            _ => {
                return Err(UnevenError::ConfigError(format!(
                    "Unknown command for limit check: {}",
                    command
                )));
            }
        };

        write_usage(&usage)?;
        Ok(count)
    }

    /// Activates a `UNVN-...` license key against the license server and persists `license.json`.
    pub async fn activate(key: &str) -> Result<LicenseCache, UnevenError> {
        let key = key.trim();
        let parsed = match parse_key(key) {
            Ok(p) => p,
            Err(err) => {
                let msg = match err {
                    "expired" => "This license has expired.",
                    "unknown_tier" => "Unknown license tier.",
                    _ => "Invalid key structure.",
                };
                return Err(UnevenError::ConfigError(format!(
                    "[UE-02000001] Key validation failed: {}",
                    msg
                )));
            }
        };

        let machine_id = get_machine_id();
        let machine_label = get_machine_label();

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| {
                UnevenError::ConfigError(
                    "[UE-03000001] Could not initialize HTTP client for license activation."
                        .to_string(),
                )
            })?;

        let res = client
            .post(format!(
                "{}/license/activate",
                obfstr::obfstr!("https://api.uneven.dev")
            ))
            .json(&serde_json::json!({
                "key": key,
                "machineId": machine_id,
                "machineLabel": machine_label,
            }))
            .send()
            .await;

        let response = match res {
            Ok(r) => r,
            Err(_) => {
                return Err(UnevenError::ConfigError(
                    "[UE-03000001] Could not reach license server. Check your connection."
                        .to_string(),
                ));
            }
        };

        let data: ServerActivationResult = response.json().await.map_err(|_| {
            UnevenError::ConfigError(
                "[UE-03000002] Activation failed: invalid server response.".to_string(),
            )
        })?;

        if !data.valid {
            let reason = data.reason.unwrap_or_default();
            let msg = match reason.as_str() {
                "not_found" => "Key not found. Check for typos or contact support.",
                "revoked" => "This license has been revoked.",
                "expired" => "This license has expired.",
                "seat_limit" => data
                    .message
                    .as_deref()
                    .unwrap_or("Machine limit reached. Deactivate another machine first."),
                _ => "Activation failed.",
            };
            return Err(UnevenError::ConfigError(format!("[UE-02000001] {}", msg)));
        }

        let tier = data.tier.ok_or_else(|| {
            UnevenError::ConfigError("Server did not return a license tier".to_string())
        })?;
        let seats = data.seats.unwrap_or(1);

        if tier != "pro" && tier != "team" {
            return Err(UnevenError::ConfigError(format!(
                "Activation failed: unexpected tier '{}' from server.",
                tier
            )));
        }

        let now = chrono::Utc::now().timestamp();
        let cache = LicenseCache {
            key: key.to_string(),
            tier,
            seats,
            expires_at: data.expires_at.unwrap_or(parsed.expires),
            verified_at: now,
            grace_until: now + GRACE_EXTENSION_DAYS * 86400,
            machine_id,
        };

        write_license(&cache)?;
        Ok(cache)
    }

    /// Deactivates the locally cached license on the server and clears `license.json`.
    pub async fn deactivate() -> Result<bool, UnevenError> {
        if let Some(cached) = read_license() {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .map_err(|_| {
                    UnevenError::ConfigError(
                        "[UE-03000001] Could not initialize HTTP client for license deactivation."
                            .to_string(),
                    )
                })?;

            let _ = client
                .post(format!(
                    "{}/license/deactivate",
                    obfstr::obfstr!("https://api.uneven.dev")
                ))
                .json(&serde_json::json!({
                    "key": cached.key,
                    "machineId": cached.machine_id,
                }))
                .send()
                .await;

            clear_license();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_license_rate_limiting_and_tiers() {
        let dir = tempdir().unwrap();
        let path_str = dir.path().to_str().unwrap().to_string();
        unsafe {
            std::env::set_var("UNEVEN_TEST_HOME", &path_str);
        }

        assert_eq!(LicenseService::active_tier(), "free");
        assert!(LicenseService::require_pro().is_err());
        assert!(LicenseService::require_team().is_err());

        for _ in 0..2 {
            assert!(LicenseService::check_session_limit("scan", 2, 6 * 3600).is_ok());
        }
        let err = LicenseService::check_session_limit("scan", 2, 6 * 3600).unwrap_err();
        assert!(err.to_string().contains("UE-02000003"));

        for _ in 0..3 {
            assert!(LicenseService::check_and_increment_usage("analyze").is_ok());
        }
        let err = LicenseService::check_and_increment_usage("analyze").unwrap_err();
        assert!(err.to_string().contains("UE-02000003"));

        unsafe {
            std::env::remove_var("UNEVEN_TEST_HOME");
        }
    }

    #[test]
    fn test_parse_key_catfive() {
        assert_eq!(parse_key("INVALID-KEY"), Err("malformed"));
        assert_eq!(parse_key("UNVN-onlytwo"), Err("malformed"));

        let payload_json = r#"{"id":"key_123","tier":"enterprise","seats":5,"issued":1700000000,"expires":0,"eh":"abc"}"#;
        let b64 = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let bad_tier_key = format!("UNVN-01-{}-SIG", b64);
        assert_eq!(parse_key(&bad_tier_key), Err("unknown_tier"));

        let valid_json = r#"{"id":"key_123","tier":"pro","seats":2,"issued":1700000000,"expires":0,"eh":"abc"}"#;
        let b64_valid = URL_SAFE_NO_PAD.encode(valid_json.as_bytes());
        let valid_key = format!("UNVN-01-{}-SIG", b64_valid);
        let parsed = parse_key(&valid_key).expect("Valid pro key should parse");
        assert_eq!(parsed.tier, "pro");
        assert_eq!(parsed.seats, 2);
    }
}
