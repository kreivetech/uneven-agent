use crate::domain::notifications::{WebhookFormatter, WebhookProvider, NotificationPayload, NotificationType};
use serde_json::{json, Value};
use chrono::Utc;

pub struct GenericFormatter;

impl WebhookFormatter for GenericFormatter {
    fn provider(&self) -> WebhookProvider {
        WebhookProvider::Generic
    }

    fn format(&self, payload: &NotificationPayload) -> Value {
        let is_fix = payload.r#type == Some(NotificationType::Fix);
        let event = if is_fix { "fix-applied" } else { "error-detected" };

        let mut res = json!({
            "source": "uneven-ai",
            "event": event,
            "timestamp": Utc::now().to_rfc3339(),
            "error": {
                "message": payload.error_message,
                "file": payload.file,
                "line": payload.line,
                "process": payload.process_label
            }
        });

        if let Some(ref fix_applied) = payload.fix_applied
            && let Some(obj) = res.as_object_mut() {
                obj.insert("fix".to_string(), json!(fix_applied));
            }

        res
    }
}
