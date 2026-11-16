pub mod slack;
pub mod discord;
pub mod generic;

use crate::domain::notifications::{WebhookFormatter, WebhookProvider, NotificationPayload};
use crate::domain::NotificationsConfig;
use std::time::{Duration, Instant};
use std::sync::Mutex;

pub struct FormatterRegistry;

impl FormatterRegistry {
    pub fn format(url: &str, payload: &NotificationPayload) -> serde_json::Value {
        let provider = Self::detect_provider(url);
        match provider {
            WebhookProvider::Slack => slack::SlackFormatter.format(payload),
            WebhookProvider::Discord => discord::DiscordFormatter.format(payload),
            WebhookProvider::Generic => generic::GenericFormatter.format(payload),
        }
    }

    fn detect_provider(url: &str) -> WebhookProvider {
        if url.contains("discord.com/api/webhooks") || url.contains("discordapp.com") {
            WebhookProvider::Discord
        } else if url.contains("hooks.slack.com") {
            WebhookProvider::Slack
        } else {
            WebhookProvider::Generic
        }
    }
}

pub struct WebhookNotifier {
    config: NotificationsConfig,
    last_notify: Mutex<Option<Instant>>,
}

impl WebhookNotifier {
    pub fn new(config: NotificationsConfig) -> Self {
        Self {
            config,
            last_notify: Mutex::new(None),
        }
    }

    /// Checks if a URL is safe for Webhooks (SSRF Protection).
    /// Blocks loopback, private, and Link-Local IPs.
    pub fn is_safe_url(url: &str) -> bool {
        let parsed = match reqwest::Url::parse(url) {
            Ok(u) => u,
            Err(_) => return false,
        };

        let host = parsed.host_str().unwrap_or("").to_lowercase();
        if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]" {
            return false;
        }

        // Check private ranges
        if host.starts_with("10.") 
            || host.starts_with("192.168.") 
            || host.starts_with("169.254.") {
            return false;
        }

        if host.starts_with("172.") {
            let parts: Vec<&str> = host.split('.').collect();
            if parts.len() >= 2
                && let Ok(second) = parts[1].parse::<u8>()
                    && (16..=31).contains(&second) {
                        return false;
                    }
        }

        true
    }

    pub async fn notify(&self, payload: &NotificationPayload) -> Result<(), crate::UnevenError> {
        let url = match &self.config.webhook_url {
            Some(u) => u,
            None => return Ok(()),
        };

        if !Self::is_safe_url(url) {
            tracing::warn!("[SECURITY] Blocked webhook to unsafe URL: {} (SSRF Protection)", url);
            return Err(crate::UnevenError::SecurityError("SSRF protection: Blocked unsafe URL".to_string()));
        }

        // Rate limiting
        let rpm = self.config.rate_limit.unwrap_or(10);
        let min_interval = Duration::from_secs(60) / rpm;

        {
            let mut last = self.last_notify.lock().unwrap();
            let now = Instant::now();
            if let Some(last_time) = *last
                && now.duration_since(last_time) < min_interval {
                    tracing::info!("[SECURITY] Webhook rate limit throttled.");
                    return Ok(());
                }
            *last = Some(now);
        }

        let body = FormatterRegistry::format(url, payload);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|e| crate::UnevenError::Unknown(format!("Failed to build HTTP client: {}", e)))?;

        let res = client.post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::UnevenError::Unknown(format!("Webhook post failed: {}", e)))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            let truncated = if text.len() > 100 { &text[..100] } else { &text };
            return Err(crate::UnevenError::Unknown(format!("Webhook HTTP {}: {}", status, truncated)));
        }

        Ok(())
    }
}
