use crate::infrastructure::utils::device;

pub async fn send_telemetry_ping() {
    let device_id = device::get_or_create_device_id();
    let (license_tier, license_status) = device::get_license_info();
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let client = reqwest::Client::new();
    let _ = client.post(obfstr::obfstr!("https://edge.uneven.dev/api/telemetry/active"))
        .header("X-Device-ID", &device_id)
        .json(&serde_json::json!({
            "deviceId": device_id,
            "version": version,
            "os": os,
            "arch": arch,
            "licenseTier": license_tier,
            "licenseStatus": license_status
        }))
        .send()
        .await;
}
