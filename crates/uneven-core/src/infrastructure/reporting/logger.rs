use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::Path;
use chrono::Utc;

pub struct MarkdownLogger {
    path: String,
}

impl MarkdownLogger {
    pub fn new(path: &str) -> Self {
        let log_path = path.to_string();
        let dir = Path::new(&log_path).parent().unwrap();
        let _ = create_dir_all(dir);

        if !Path::new(&log_path).exists() {
            if let Ok(mut file) = OpenOptions::new().create(true).write(true).truncate(true).open(&log_path) {
                let header = format!("# Uneven Log — {}\n\n", Utc::now().format("%Y-%m-%d"));
                let _ = file.write_all(header.as_bytes());
            }
        }

        Self { path: log_path }
    }

    fn get_timestamp(&self) -> String {
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    fn append(&self, entry: &str) {
        if let Ok(mut file) = OpenOptions::new().append(true).open(&self.path) {
            let _ = file.write_all(entry.as_bytes());
        }
    }

    pub fn info(&self, message: &str) {
        let entry = format!("## [{}] ℹ️ Info\n\n{}\n\n---\n\n", self.get_timestamp(), message);
        self.append(&entry);
    }

    pub fn warn(&self, message: &str) {
        let entry = format!("## [{}] ⚠️ Warning\n\n{}\n\n---\n\n", self.get_timestamp(), message);
        self.append(&entry);
    }

    pub fn error(&self, message: &str) {
        let msg = if message.starts_with("[UE-") {
            message.to_string()
        } else {
            "Internal technical failure. Please report the issue.".to_string()
        };
        let entry = format!("## [{}] ❌ Error\n\n{}\n\n---\n\n", self.get_timestamp(), msg);
        self.append(&entry);
    }

    pub fn critical(&self, message: &str) {
        let entry = format!("## [{}] 🚨 CRITICAL ALERT\n\n{}\n\n---\n\n", self.get_timestamp(), message);
        self.append(&entry);
    }

    pub fn fix_applied(&self, file: &str, before: &str, after: &str, explanation: &str) {
        let escape_fence = |s: &str| s.replace("```", "`\u{200B}``");
        let entry = format!(
            "## [{}] ✅ Fix Applied\n\n**File:** {}\n\n**Before:**\n```\n{}\n```\n\n**After:**\n```\n{}\n```\n\n**Explanation:** {}\n\n---\n\n",
            self.get_timestamp(),
            file,
            escape_fence(before),
            escape_fence(after),
            explanation
        );
        self.append(&entry);
    }
}
