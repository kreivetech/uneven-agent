use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_legacy = ["c", "cpp", "h", "hpp"].contains(&ext.as_str()) || 
           code_str == "C_ERR" || code_str == "CPP_ERR";

    if !is_legacy {
        return None;
    }

    if error.message.contains("expected ';'") || code_str == "C_ERR" {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        let target_idx = if line_idx > 0 && !lines.get(line_idx.saturating_sub(1)).unwrap_or(&"").trim().ends_with(';') {
            line_idx - 1
        } else {
            line_idx
        };
        
        if let Some(&original_line) = lines.get(target_idx) {
            let trimmed = original_line.trim();
            if !trimmed.is_empty() && !trimmed.ends_with(';') && !trimmed.ends_with('{') && !trimmed.ends_with('}') {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: target_idx + 1,
                    before: original_line.to_string(),
                    after: format!("{};", original_line),


                    confidence: 0.9,
            append_eof: false,
            top_of_file: false,
                    explanation: "C/C++: Missing semicolon".to_string(),
                    suggested_fix: "Appended missing semicolon".to_string(),
                });
            }
        }
    }

    None
}
