use crate::domain::notifications::{WebhookFormatter, WebhookProvider, NotificationPayload, NotificationType};
use serde_json::{json, Value};
use chrono::Utc;

pub struct DiscordFormatter;

impl WebhookFormatter for DiscordFormatter {
    fn provider(&self) -> WebhookProvider {
        WebhookProvider::Discord
    }

    fn format(&self, payload: &NotificationPayload) -> Value {
        let is_fix = payload.r#type == Some(NotificationType::Fix);
        let title = if is_fix { "✓ Fix applied" } else { "⚠ Error detected" };
        let color = if is_fix { 0x00ff8c } else { 0xff4444 }; // green for fix, red for error

        let mut fields = Vec::new();
        if let Some(ref process_label) = payload.process_label {
            fields.push(json!({
                "name": "Process",
                "value": process_label,
                "inline": true
            }));
        }

        if let Some(ref file) = payload.file {
            let loc = if let Some(line) = payload.line {
                format!("{}:{}", file, line)
            } else {
                file.clone()
            };
            fields.push(json!({
                "name": "Location",
                "value": loc,
                "inline": true
            }));
        }

        // Limit error message to 1024 characters to respect Discord embeds constraints
        let error_val = if payload.error_message.len() > 1024 {
            &payload.error_message[..1024]
        } else {
            &payload.error_message
        };

        fields.push(json!({
            "name": "Error",
            "value": error_val,
            "inline": false
        }));

        if let Some(ref fix_applied) = payload.fix_applied {
            fields.push(json!({
                "name": "Fix",
                "value": fix_applied,
                "inline": false
            }));
        }

        json!({
            "embeds": [
                {
                    "title": title,
                    "color": color,
                    "fields": fields,
                    "footer": {
                        "text": "Uneven AI"
                    },
                    "timestamp": Utc::now().to_rfc3339()
                }
            ]
        })
    }
}
