use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebhookProvider {
    Slack,
    Discord,
    Generic,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Error,
    Fix,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPayload {
    pub r#type: Option<NotificationType>,
    pub error_message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub process_label: Option<String>,
    pub fix_applied: Option<String>,
}

pub trait WebhookFormatter: Send + Sync {
    fn provider(&self) -> WebhookProvider;
    fn format(&self, payload: &NotificationPayload) -> serde_json::Value;
}
