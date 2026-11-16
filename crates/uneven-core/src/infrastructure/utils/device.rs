use std::fs;
use std::path::PathBuf;
use uuid::Uuid;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LicenseCache {
    tier: String,
    expires_at: i64,
    grace_until: i64,
}

pub fn get_or_create_device_id() -> String {
    let home_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    
    let uneven_dir = PathBuf::from(home_dir).join(".uneven");
    let device_file = uneven_dir.join("device_id");

    if device_file.exists()
        && let Ok(id) = fs::read_to_string(&device_file) {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

    let _ = fs::create_dir_all(&uneven_dir);
    let new_id = Uuid::new_v4().to_string();
    let _ = fs::write(&device_file, &new_id);
    new_id
}

pub fn get_license_info() -> (String, String) {
    let home_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    
    let license_file = PathBuf::from(home_dir).join(".uneven").join("license.json");
    if !license_file.exists() {
        return ("free".to_string(), "inactive".to_string());
    }

    let file_content = match fs::read_to_string(&license_file) {
        Ok(c) => c,
        Err(_) => return ("free".to_string(), "inactive".to_string()),
    };

    let cache: LicenseCache = match serde_json::from_str(&file_content) {
        Ok(c) => c,
        Err(_) => return ("free".to_string(), "inactive".to_string()),
    };

    let now = chrono::Utc::now().timestamp();
    let active = cache.expires_at == 0 || now < cache.expires_at || now < cache.grace_until;

    let status = if active { "active".to_string() } else { "expired".to_string() };
    (cache.tier, status)
}
