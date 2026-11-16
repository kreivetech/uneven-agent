use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_GO_UNDEF: Lazy<Regex> = Lazy::new(|| Regex::new(r"undefined:\s+(\S+)").unwrap());
static RE_GO_UNUSED: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\S+) declared (and|but) not used").unwrap());
static RE_GO_IMPORT: Lazy<Regex> = Lazy::new(|| Regex::new(r#""(.+?)" imported and not used"#).unwrap());
static RE_GO_TYPE: Lazy<Regex> = Lazy::new(|| Regex::new(r"cannot use (.+?) \((?:variable of )?type (.+?)\) as (?:type )?(.+?)(?:\s|$)").unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_go = code_str.starts_with("GO") || ext == "go";

    if !is_go {
        return None;
    }

    if code_str == "GO_UNDEF" || error.message.contains("undefined:") || error.message.contains("declared and not used") || error.message.contains("imported and not used") {
        if let Some(caps) = RE_GO_UNDEF.captures(&error.message) {
            let name = caps.get(1).map(|m| m.as_str()).unwrap_or("var");
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: "".to_string(),
                after: format!("// Add import or define: var {} = ...", name),
                confidence: 0.2,
            append_eof: false,
            top_of_file: false,
                explanation: format!("Go: '{}' is not defined in this scope", name),
                suggested_fix: format!("Import the package or define '{}' before use", name),
            });
        }
        if let Some(caps) = RE_GO_UNUSED.captures(&error.message) {
            let name = caps.get(1).map(|m| m.as_str()).unwrap_or("var");
            let lines: Vec<&str> = content.lines().collect();
            let line_idx = error.line.saturating_sub(1);
            if let Some(&original_line) = lines.get(line_idx) {
                let target = format!("{} :=", name);
                let replacement = "_ =";
                if original_line.contains(&target) {
                    return Some(FixSuggestion {
                        file: error.file.clone(),
                        line: error.line,
                        before: original_line.to_string(),
                        after: original_line.replace(&target, replacement),


                        confidence: 0.95,
            append_eof: false,
            top_of_file: false,
                        explanation: format!("Go: Variable '{}' is declared but never used", name),
                        suggested_fix: "Replaced unused variable with blank identifier '_'".to_string(),
                    });
                }
            }
        }
        if let Some(caps) = RE_GO_IMPORT.captures(&error.message) {
            let pkg = caps.get(1).map(|m| m.as_str()).unwrap_or("pkg");
            let lines: Vec<&str> = content.lines().collect();
            let line_idx = error.line.saturating_sub(1);
            if let Some(&original_line) = lines.get(line_idx) {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: "".to_string(),


                    confidence: 0.95,
            append_eof: false,
            top_of_file: false,
                    explanation: format!("Go: Package '{}' is imported but not used", pkg),
                    suggested_fix: "Removed the unused import line".to_string(),
                });
            }
        }
    }

    if code_str == "GO_SYNTAX" || error.message.contains("syntax error") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("// Review syntax near line {}", error.line),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Go: Syntax error — {}", error.message),
            suggested_fix: format!("Check for missing brackets, braces, or semicolons near line {}", error.line),
        });
    }

    if (code_str == "GO_TYPE" || error.message.contains("cannot use"))
        && let Some(caps) = RE_GO_TYPE.captures(&error.message) {
            let expr = caps.get(1).map(|m| m.as_str()).unwrap_or("expr");
            let got_type = caps.get(2).map(|m| m.as_str()).unwrap_or("type");
            let exp_type = caps.get(3).map(|m| m.as_str()).unwrap_or("type");
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: "".to_string(),
                after: format!("// Use type conversion: {}({})", exp_type, expr),
                confidence: 0.4,
            append_eof: false,
            top_of_file: false,
                explanation: format!("Go: Type mismatch — got {}, expected {}", got_type, exp_type),
                suggested_fix: format!("Convert '{}' to {}, e.g. {}({})", expr, exp_type, exp_type, expr),
            });
        }

    None
}
