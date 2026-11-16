use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_RUST_CODE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^E\d{4}$").unwrap());
static RE_CANNOT_FIND: Lazy<Regex> = Lazy::new(|| Regex::new(r"cannot find value `(.+?)` in this scope").unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_rust = RE_RUST_CODE.is_match(code_str) || ext == "rs";

    if !is_rust {
        return None;
    }

    let lines: Vec<&str> = content.lines().collect();

    // 1. Undefined value (E0425)
    if (code_str == "E0425" || error.message.contains("E0425"))
        && let Some(caps) = RE_CANNOT_FIND.captures(&error.message) {
            let var_name = caps.get(1).map(|m| m.as_str()).unwrap_or("var");
            let line_idx = error.line.saturating_sub(1);
            if let Some(&original_line) = lines.get(line_idx) {
                let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}let {} = Default::default();\n{}", indent, var_name, original_line),


                    confidence: 0.8,
            append_eof: false,
            top_of_file: false,
                    explanation: format!("Rust: Variable '{}' is not defined", var_name),
                    suggested_fix: format!("Injected default initialization for '{}'", var_name),
                });
            }
        }

    // 2. Missing Trait (E0277)
    if code_str == "E0277" || error.message.contains("E0277") {
        let mut struct_line_idx = error.line.saturating_sub(1);
        while struct_line_idx > 0 {
            if lines[struct_line_idx].contains("struct ") || lines[struct_line_idx].contains("enum ") {
                break;
            }
            struct_line_idx -= 1;
        }

        if let Some(&struct_line) = lines.get(struct_line_idx)
            && (struct_line.contains("struct ") || struct_line.contains("enum ")) {
                let indent = struct_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: struct_line_idx + 1,
                    before: struct_line.to_string(),
                    after: format!("{}#[derive(Debug, Clone, Default)]\n{}", indent, struct_line),


                    confidence: 0.75,
            append_eof: false,
            top_of_file: false,
                    explanation: "Rust: Type does not implement required trait".to_string(),
                    suggested_fix: "Added common derives to the closest struct/enum".to_string(),
                });
            }
    }

    None
}
