use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_PHP_FN: Lazy<Regex> = Lazy::new(|| Regex::new(r"Call to undefined function (.+?)\(").unwrap());
static RE_PHP_CLASS: Lazy<Regex> = Lazy::new(|| Regex::new(r"Class '.+?' not found|Class '(.+?)' not found").unwrap());
static RE_PHP_VAR: Lazy<Regex> = Lazy::new(|| Regex::new(r"Undefined variable:?\s*\$?(\w+)").unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_php = code_str.starts_with("PHP") || ext == "php";

    if !is_php {
        return None;
    }

    if code_str == "PHP_UNDEF_FUNC" || error.message.contains("Call to undefined function") {
        let fn_name = RE_PHP_FN.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("unknown");
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("// Define: function {}() {{ ... }}", fn_name),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("PHP: Function '{}' is not defined", fn_name),
            suggested_fix: "Define the function, install the extension, or check the function name spelling".to_string(),
        });
    }

    if code_str == "PHP_UNDEF_CLASS" || error.message.contains("Class not found") || error.message.contains("not found") {
        let class_name = RE_PHP_CLASS.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("ClassName");
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("// Add: use Namespace\\{};", class_name),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: "PHP: Class not found — missing autoload or use statement".to_string(),
            suggested_fix: "Add use statement or ensure autoloader covers the class".to_string(),
        });
    }

    if code_str == "PHP_SYNTAX" || error.message.contains("Parse error") {
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
                    explanation: format!("PHP: Parse error — {}", error.message),
                    suggested_fix: "Appended missing semicolon".to_string(),
                });
            }
        }
    }

    if code_str == "PHP_UNDEF_VAR" || error.message.contains("Undefined variable") {
        let var_name = RE_PHP_VAR.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("x");
            
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: original_line.to_string(),
                after: format!("{}${} = null;\n{}", indent, var_name, original_line),


                confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                explanation: format!("PHP: Variable ${} is used before being defined", var_name),
                suggested_fix: format!("Injected default initialization for '${}'", var_name),
            });
        }
    }

    if code_str == "PHP_FATAL" || error.message.contains("Fatal error") {
        let length = std::cmp::min(60, error.message.len());
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("// Fix the underlying cause: {}", &error.message[..length]),
            confidence: 0.3,
            append_eof: false,
            top_of_file: false,
            explanation: format!("PHP: Fatal error — {}", error.message),
            suggested_fix: "Inspect the error message and resolve the root cause".to_string(),
        });
    }

    None
}
