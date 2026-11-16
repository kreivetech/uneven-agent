use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use catfive::{schema, string, number, Validator};
use console::style;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::Command as SysCommand;
use std::time::Duration;

// API_BASE is now obfuscated inline
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
#[derive(Debug, Deserialize, Clone)]
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

// ─── Fingerprint Utilities ───────────────────────────────────────────────────

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
    format!("{:x}", hasher.finalize())
}

pub fn get_machine_label() -> String {
    format!(
        "{}-{} / {}",
        get_node_platform(),
        get_node_arch(),
        get_hostname()
    )
}

// ─── Cache Storage ───────────────────────────────────────────────────────────

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

pub fn write_license(data: &LicenseCache) -> Result<()> {
    let path = get_license_path().ok_or_else(|| anyhow!("Home directory not resolved"))?;
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

// ─── Key Verification ────────────────────────────────────────────────────────

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

// ─── Gate / Requirements Checks ──────────────────────────────────────────────

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

pub fn require_pro() -> Result<()> {
    if !check_pro() {
        return Err(anyhow!(
            "\n  [UE-01000005] This feature requires Uneven Pro.\n\n  Upgrade at:  https://uneven.dev/pricing\n  Activate:    uneven license activate <your-key>\n"
        ));
    }
    Ok(())
}

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

pub fn require_team() -> Result<()> {
    if !check_team() {
        return Err(anyhow!(
            "\n  [UE-01000006] This feature requires Uneven Team.\n\n  Upgrade at:  https://uneven.dev/pricing\n  Activate:    uneven license activate <your-key>\n"
        ));
    }
    Ok(())
}

// ─── Command Runners ─────────────────────────────────────────────────────────

pub async fn run_activate(key: String) -> Result<()> {
    let key = key.trim();

    let parsed = match parse_key(key) {
        Ok(p) => p,
        Err(err) => {
            let msg = match err {
                "expired" => "This license has expired.",
                "unknown_tier" => "Unknown license tier.",
                _ => "Invalid key structure.",
            };
            return Err(anyhow!("[UE-02000001] Key validation failed: {}", msg));
        }
    };

    let machine_id = get_machine_id();
    let machine_label = get_machine_label();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

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
            return Err(anyhow!(
                "[UE-03000001] Could not reach license server. Check your connection."
            ));
        }
    };

    let data: ServerActivationResult = response
        .json()
        .await
        .map_err(|e| anyhow!("Activation failed: invalid response schema ({}).", e))?;

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
        return Err(anyhow!("[UE-02000001] {}", msg));
    }

    let tier = data
        .tier
        .ok_or_else(|| anyhow!("Server did not return a license tier"))?;
    let seats = data.seats.unwrap_or(1);

    if tier != "pro" && tier != "team" {
        return Err(anyhow!(
            "Activation failed: unexpected tier '{}' from server.",
            tier
        ));
    }

    let now = chrono::Utc::now().timestamp();
    let cache = LicenseCache {
        key: key.to_string(),
        tier: tier.clone(),
        seats,
        expires_at: data.expires_at.unwrap_or(parsed.expires),
        verified_at: now,
        grace_until: now + GRACE_EXTENSION_DAYS * 86400,
        machine_id,
    };

    write_license(&cache)?;

    println!(
        "\n  {} {}\n",
        style("✓").green().bold(),
        style(format!(
            "License activated. Tier: {} · {} machine(s).",
            tier.to_uppercase(),
            seats
        ))
        .bold()
    );

    Ok(())
}

pub async fn run_status() -> Result<()> {
    if let Some(license) = read_license() {
        let now = chrono::Utc::now().timestamp();
        let active =
            license.expires_at == 0 || now < license.expires_at || now < license.grace_until;

        let status_str = if active {
            style("ACTIVE").green().bold()
        } else {
            style("EXPIRED").red().bold()
        };

        println!();
        println!("  {}", style("Uneven License Status").bold());
        println!("  {}", style("─".repeat(40)).dim());
        println!("  Status     : {}", status_str);
        println!(
            "  Tier       : {}",
            style(license.tier.to_uppercase()).color256(208).bold()
        );
        println!("  Seats      : {}", license.seats);

        if license.expires_at == 0 {
            println!("  Expires    : Never");
        } else {
            let exp_date = chrono::DateTime::from_timestamp(license.expires_at, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "N/A".to_string());
            println!("  Expires    : {}", exp_date);
        }

        println!("  Machine ID : {}", style(license.machine_id).dim());
        println!();
    } else {
        println!(
            "\n  {} {}\n",
            style("ℹ").blue().bold(),
            style("No active license. Running on Free tier.").dim()
        );
    }
    Ok(())
}

pub async fn run_deactivate() -> Result<()> {
    if let Some(cached) = read_license() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()?;

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
        println!(
            "\n  {} {}\n",
            style("✓").green().bold(),
            style("License deactivated locally.").bold()
        );
    } else {
        println!("\n  No active license to deactivate.\n");
    }
    Ok(())
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

pub fn write_usage(data: &UsageData) -> Result<()> {
    let path = get_usage_path().ok_or_else(|| anyhow!("Home directory not resolved"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    let content = serde_json::to_string_pretty(data)?;
    fs::write(&tmp, content)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn check_session_limit(command: &str, max_queries: usize, window_secs: i64) -> Result<()> {
    if check_pro() {
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
        return Err(anyhow!(
            "\n  [UE-02000003] Limit of {} scans/queries per {}-hour session reached for '{}' on Free license.\n\n  Upgrade to Pro for unlimited use: https://uneven.dev/pricing\n",
            max_queries,
            hours,
            command
        ));
    }

    timestamps.push(now);
    write_usage(&usage)?;
    Ok(())
}

pub fn check_and_increment_usage(command: &str) -> Result<usize> {
    if check_pro() {
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
                return Err(anyhow!(
                    "\n  [UE-02000003] Daily limit of 3 queries for 'analyze' reached on Free license.\n\n  Upgrade to Pro for unlimited use: https://uneven.dev/pricing\n"
                ));
            }
            usage.analyze_count += 1;
            usage.analyze_count
        }
        "askf" => 0,
        _ => return Err(anyhow!("Unknown command for limit check: {}", command)),
    };

    write_usage(&usage)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_license_rate_limiting_all() {
        // Run test 1: usage limits
        {
            let temp = tempdir().unwrap();
            unsafe { std::env::set_var("UNEVEN_TEST_HOME", temp.path()) };

            assert!(read_usage().is_none());

            for _ in 1..=5 {
                let res = check_and_increment_usage("askf");
                assert!(res.is_ok());
                assert_eq!(res.unwrap(), 0);
            }

            for i in 1..=3 {
                let res = check_and_increment_usage("analyze");
                assert!(res.is_ok());
                assert_eq!(res.unwrap(), i);
            }

            let res_fail_an = check_and_increment_usage("analyze");
            assert!(res_fail_an.is_err());
            let err_msg_an = res_fail_an.unwrap_err().to_string();
            assert!(err_msg_an.contains("[UE-02000003]"));

            for _ in 1..=2 {
                let res = check_session_limit("pentest", 2, 6 * 3600);
                assert!(res.is_ok());
            }
            let res_fail_pt = check_session_limit("pentest", 2, 6 * 3600);
            assert!(res_fail_pt.is_err());

            for _ in 1..=2 {
                let res = check_session_limit("scan", 2, 6 * 3600);
                assert!(res.is_ok());
            }
            let res_fail_sc = check_session_limit("scan", 2, 6 * 3600);
            assert!(res_fail_sc.is_err());

            unsafe { std::env::remove_var("UNEVEN_TEST_HOME") };
        }

        // Run test 2: date reset
        {
            let temp = tempdir().unwrap();
            unsafe { std::env::set_var("UNEVEN_TEST_HOME", temp.path()) };

            let old_usage = UsageData {
                date: "1970-01-01".to_string(),
                askf_count: 15,
                analyze_count: 2,
                session_timestamps: std::collections::HashMap::new(),
            };
            assert!(write_usage(&old_usage).is_ok());

            let res = check_and_increment_usage("analyze");
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), 1);

            let current = read_usage().unwrap();
            assert_eq!(current.analyze_count, 1);
            assert_eq!(
                current.date,
                chrono::Local::now().format("%Y-%m-%d").to_string()
            );

            unsafe { std::env::remove_var("UNEVEN_TEST_HOME") };
        }
    }

    #[test]
    fn test_parse_key_catfive() {
        assert_eq!(parse_key("INVALID-KEY").unwrap_err(), "malformed");
        assert_eq!(parse_key("UNVN-PART1").unwrap_err(), "malformed");

        // Create a valid payload
        let valid_json = r#"{"id":"lic-123","tier":"pro","seats":2,"issued":1000,"expires":9999999999,"eh":"sig"}"#;
        let b64 = URL_SAFE_NO_PAD.encode(valid_json.as_bytes());
        let valid_key = format!("UNVN-v1-{}-SIG", b64);
        assert!(parse_key(&valid_key).is_ok());

        // Unknown tier rejected by Catfive schema
        let unknown_json = r#"{"id":"lic-123","tier":"unsupported","seats":2,"issued":1000,"expires":9999999999,"eh":"sig"}"#;
        let b64_unknown = URL_SAFE_NO_PAD.encode(unknown_json.as_bytes());
        let unknown_key = format!("UNVN-v1-{}-SIG", b64_unknown);
        assert_eq!(parse_key(&unknown_key).unwrap_err(), "unknown_tier");
    }
}
