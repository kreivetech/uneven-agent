use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_cobol = ["cob", "cbl", "cpy"].contains(&ext.as_str()) || code_str == "COBOL_SYNTAX";

    if !is_cobol {
        return None;
    }

    let msg_lower = error.message.to_lowercase();

    // 1. Missing Period error
    if msg_lower.contains("missing period") || msg_lower.contains("expected period") || msg_lower.contains("period expected") {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let trimmed = original_line.trim_end();
            if !trimmed.ends_with('.') {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}.", trimmed),


                    confidence: 0.9,
            append_eof: false,
            top_of_file: false,
                    explanation: "COBOL: Missing period (.) terminator".to_string(),
                    suggested_fix: "Appended missing period '.' at the end of the line".to_string(),
                });
            }
        }
    }

    // 2. Division structure error
    if msg_lower.contains("division") || msg_lower.contains("section") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "* Verify standard division structure".to_string(),
            confidence: 0.3,
            append_eof: false,
            top_of_file: false,
            explanation: "COBOL: Invalid Division or Section declaration or sequence".to_string(),
            suggested_fix: "Verify the sequence: IDENTIFICATION DIVISION, ENVIRONMENT DIVISION, DATA DIVISION, PROCEDURE DIVISION".to_string(),
        });
    }

    // 3. Columns Alignment / Margin error
    if msg_lower.contains("margin") || msg_lower.contains("column") || msg_lower.contains("area a") || msg_lower.contains("area b") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "* Adjust spacing to align with COBOL fixed layout margins".to_string(),
            confidence: 0.3,
            append_eof: false,
            top_of_file: false,
            explanation: "COBOL: Area A/B Margin alignment error".to_string(),
            suggested_fix: "Ensure Area A declarations (columns 8-11) and Area B statements (columns 12-72) follow COBOL fixed-format rules".to_string(),
        });
    }

    // 4. Generic COBOL compile error
    Some(FixSuggestion {
        file: error.file.clone(),
        line: error.line,
        before: "".to_string(),
        after: format!("* Review COBOL syntax near line {}", error.line),
        confidence: 0.3,
            append_eof: false,
            top_of_file: false,
        explanation: format!("COBOL: Compilation error — {}", error.message),
        suggested_fix: format!("Review COBOL statements near line {}", error.line),
    })
}
