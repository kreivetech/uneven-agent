use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_LINTER_UNUSED: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)unused|not used").unwrap());
static RE_LINTER_LONGLINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)line.*too long").unwrap());

pub fn suggest(error: &ErrorEvent) -> Option<FixSuggestion> {
    let error_type_str = error.error_type.map(|t| format!("{:?}", t)).unwrap_or_default().to_lowercase();
    let is_linter = error_type_str == "linter" || RE_LINTER_UNUSED.is_match(&error.message) || RE_LINTER_LONGLINE.is_match(&error.message);

    if !is_linter {
        return None;
    }

    if RE_LINTER_UNUSED.is_match(&error.message) {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Prefix with _: const _unused = ...;".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,


            explanation: "Linter: Variable is declared but not used".to_string(),
            suggested_fix: "Remove variable or prefix with underscore if intentional".to_string(),
        });
    }

    if RE_LINTER_LONGLINE.is_match(&error.message) {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Split into multiple lines".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,


            explanation: "Linter: Line exceeds maximum length".to_string(),
            suggested_fix: "Break line into multiple lines".to_string(),
        });
    }

    None
}
