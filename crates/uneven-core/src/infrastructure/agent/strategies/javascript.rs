use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_READ_PROPS: Lazy<Regex> = Lazy::new(|| Regex::new(r"reading '(.+?)'").unwrap());
static RE_NOT_DEFINED: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?:name\s+)?['"]?(\w+)['"]?\s+is not defined"#).unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_js = ["js", "jsx", "mjs", "cjs"].contains(&ext.as_str()) || code_str == "JS_SYNTAX";

    if !is_js {
        return None;
    }

    // 1. Optional Chaining for Null/Undefined Property Access
    if error.message.contains("Cannot read properties of undefined") || error.message.contains("Cannot read properties of null") {
        let prop = RE_READ_PROPS.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("prop");
            
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let target = format!(".{}", prop);
            let replacement = format!("?.{}", prop);
            if original_line.contains(&target) {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: original_line.replace(&target, &replacement),


                    confidence: 0.95,
            append_eof: false,
            top_of_file: false,
                    explanation: format!("JavaScript: Cannot read property '{}' on null or undefined", prop),
                    suggested_fix: format!("Added optional chaining (?.) before accessing '{}'", prop),
                });
            }
        }
    }

    // 2. ReferenceError: Variable not defined
    if error.message.contains("is not defined") || code_str == "ReferenceError" {
        let var_name = RE_NOT_DEFINED.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("var");
            
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: original_line.to_string(),
                after: format!("{}let {} = null;\n{}", indent, var_name, original_line),


                confidence: 0.8,
            append_eof: false,
            top_of_file: false,
                explanation: format!("JavaScript: Variable '{}' is not defined", var_name),
                suggested_fix: format!("Injected initialization for '{}'", var_name),
            });
        }
    }

    // 3. Generic Syntax Error
    if error.message.contains("SyntaxError") || error.message.contains("Unexpected token") || code_str == "SyntaxError" {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("// Fix syntax near line {}", error.line),
            confidence: 0.3,
            append_eof: false,
            top_of_file: false,
            explanation: format!("JavaScript: Syntax error — {}", error.message),
            suggested_fix: format!("Check for unmatched parenthesis, brackets, or invalid syntax near line {}", error.line),
        });
    }

    None
}
