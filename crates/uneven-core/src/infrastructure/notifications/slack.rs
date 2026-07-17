use crate::domain::notifications::{WebhookFormatter, WebhookProvider, NotificationPayload, NotificationType};
use serde_json::{json, Value};

pub struct SlackFormatter;

impl WebhookFormatter for SlackFormatter {
    fn provider(&self) -> WebhookProvider {
        WebhookProvider::Slack
    }

    fn format(&self, payload: &NotificationPayload) -> Value {
        let is_fix = payload.r#type == Some(NotificationType::Fix);
        let header = if is_fix { "✓ Fix applied" } else { "⚠ Error detected" };

        let mut blocks = vec![
            json!({
                "type": "header",
                "text": {
                    "type": "plain_text",
                    "text": header,
                    "emoji": true
                }
            })
        ];

        let mut fields = Vec::new();
        if let Some(ref process_label) = payload.process_label {
            fields.push(json!({
                "type": "mrkdwn",
                "text": format!("*Process:* {}", process_label)
            }));
        }

        if let Some(ref file) = payload.file {
            let loc = if let Some(line) = payload.line {
                format!("{}:{}", file, line)
            } else {
                file.clone()
            };
            fields.push(json!({
                "type": "mrkdwn",
                "text": format!("*Location:* {}", loc)
            }));
        }

        if !fields.is_empty() {
            blocks.push(json!({
                "type": "section",
                "fields": fields
            }));
        }

        blocks.push(json!({
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": payload.error_message
            }
        }));

        if let Some(ref fix_applied) = payload.fix_applied {
            blocks.push(json!({
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("*Fix applied:* {}", fix_applied)
                }
            }));
        }

        blocks.push(json!({
            "type": "divider"
        }));

        blocks.push(json!({
            "type": "context",
            "elements": [
                {
                    "type": "mrkdwn",
                    "text": "_Sent by Uneven AI_"
                }
            ]
        }));

        json!({
            "blocks": blocks
        })
    }
}
