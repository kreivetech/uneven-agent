use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_RUNTIME_NULL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)null|undefined|NoneType").unwrap());
static RE_RUNTIME_BOUNDS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)index.*out.*of.*bounds|IndexError").unwrap());

pub fn suggest(error: &ErrorEvent) -> Option<FixSuggestion> {
    let error_type_str = error.error_type.map(|t| format!("{:?}", t)).unwrap_or_default().to_lowercase();
    let is_runtime = error_type_str == "runtime" || RE_RUNTIME_NULL.is_match(&error.message) || RE_RUNTIME_BOUNDS.is_match(&error.message);

    if !is_runtime {
        return None;
    }

    if RE_RUNTIME_NULL.is_match(&error.message) {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Add: if (obj != null) { ... } or use obj?.property".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,


            explanation: "Runtime: Accessing property on null/undefined value".to_string(),
            suggested_fix: "Add null check or optional chaining".to_string(),
        });
    }

    if RE_RUNTIME_BOUNDS.is_match(&error.message) {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Check: if (index >= 0 && index < arr.length) { ... }".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,


            explanation: "Runtime: Array index out of bounds".to_string(),
            suggested_fix: "Add bounds check before accessing array".to_string(),
        });
    }

    None
}
