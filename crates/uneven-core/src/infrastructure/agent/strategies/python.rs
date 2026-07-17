use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_PYTHON_NAME: Lazy<Regex> = Lazy::new(|| Regex::new(r#"name ['"](\w+)['"] is not defined"#).unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_python = ["SyntaxError", "IndentationError", "NameError", "TypeError", "ImportError"].contains(&code_str) 
        || ext == "py";

    if !is_python {
        return None;
    }

    if code_str == "SyntaxError" || error.message.contains("SyntaxError") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("# Review syntax near line {}", error.line),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Python: Syntax error — {}", error.message),
            suggested_fix: format!("Check for missing colons, mismatched brackets, or indentation issues near line {}", error.line),
        });
    }

    if code_str == "IndentationError" || error.message.contains("IndentationError") {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let leading_spaces = original_line.chars().take_while(|c| *c == ' ').count();
            let mut expected_spaces = ((leading_spaces as f32 / 4.0).round() as usize) * 4;
            if expected_spaces == 0 && leading_spaces > 0 {
                expected_spaces = 4;
            }
            let fixed_line = format!("{}{}", " ".repeat(expected_spaces), original_line.trim_start());
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: original_line.to_string(),
                after: fixed_line,


                confidence: 0.9,
            append_eof: false,
            top_of_file: false,
                explanation: "Python: Indentation error — mismatched whitespace".to_string(),
                suggested_fix: format!("Fixed indentation to {} spaces", expected_spaces),
            });
        }
    }

    if code_str == "NameError" || error.message.contains("NameError") || error.message.contains("is not defined") {
        let var_name = RE_PYTHON_NAME.captures(&error.message)
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
                after: format!("{}{} = None\n{}", indent, var_name, original_line),


                confidence: 0.8,
            append_eof: false,
            top_of_file: false,
                explanation: format!("Python: Variable '{}' is not defined", var_name),
                suggested_fix: format!("Injected default initialization for '{}'", var_name),
            });
        }
    }

    None
}
