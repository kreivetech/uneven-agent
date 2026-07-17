use crate::infrastructure::reporting::MarkdownLogger;
use crate::infrastructure::pentester::dossier::{self, AttackerDossier};
use regex::Regex;

pub struct TerminalMonitor {
    logger: MarkdownLogger,
    dossier_regex: Regex,
}

impl TerminalMonitor {
    pub fn new(log_path: &str) -> Self {
        Self {
            logger: MarkdownLogger::new(log_path),
            dossier_regex: Regex::new(r"(?i)X-Uneven-Dossier[:' ]+([A-Za-z0-9+/=]+)").unwrap(),
        }
    }

    pub fn process_output(&self, text: &str) {
        if let Some(cap) = self.dossier_regex.captures(text) {
            let base64_payload = &cap[1];
            match dossier::decode_dossier(base64_payload) {
                Ok(dossier) => {
                    self.scream_in_log(&dossier);
                }
                Err(e) => {
                    self.logger.error(&format!("Failed to decode X-Uneven-Dossier payload: {}", e));
                }
            }
        }
    }

    fn scream_in_log(&self, dossier: &AttackerDossier) {
        let mut log_parts = vec![
            "SECURITY ALERT: Incoming pentest probe detected via Uneven AI!".to_string(),
            "Target received advanced attacker dossier bypassing VPN.".to_string(),
            "--- ATTACKER IDENTITY ---".to_string(),
            format!("Public IP: {}", dossier.ip.as_deref().unwrap_or("Unknown")),
            format!("Timezone: {}", dossier.timezone),
            format!("Local IPs: {}", dossier.local_ips.join(", ")),
            format!("Nearby Wi-Fi (BSSIDs for Geolocation): {}", 
                if dossier.wifi_bssids.is_empty() { "None detected".to_string() } else { dossier.wifi_bssids.join(", ") }),
        ];

        if let Some(claim) = &dossier.authorization_claim {
            log_parts.push("--- AUTHORIZATION CLAIM (UNVERIFIED) ---".to_string());
            log_parts.push(format!("Claimed Authorization: \"{}\"", claim.name));
            log_parts.push(format!("Company: \"{}\"", claim.company));
            log_parts.push(format!("Address: \"{}, {}\"", claim.address, claim.zip));
            log_parts.push(format!("Claimed Targets in Scope: {}", claim.targets.join(", ")));
            log_parts.push(format!("Scope Declared At: {}", claim.declared_at));
        }

        log_parts.push("-------------------------".to_string());
        log_parts.push(format!("Message: {}", dossier.message));

        // Scream!
        self.logger.critical(&log_parts.join("\n"));
    }
}
