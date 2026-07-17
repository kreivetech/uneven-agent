use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_PROP_MISSING: Lazy<Regex> = Lazy::new(|| Regex::new(r"Property '(.+?)' does not exist").unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_ts = code_str.starts_with("TS") 
        || ext == "ts" 
        || ext == "tsx" 
        || error.message.contains("TS2339") 
        || error.message.contains("TS1005") 
        || error.message.contains("TS2345");

    if !is_ts {
        return None;
    }

    // 1. Missing Property (TS2339)
    if code_str == "TS2339" || error.message.contains("TS2339") {
        if let Some(caps) = RE_PROP_MISSING.captures(&error.message) {
            let prop = caps.get(1).map(|m| m.as_str()).unwrap_or("property");
            
            let lines: Vec<&str> = content.lines().collect();
            let line_idx = error.line.saturating_sub(1);
            if let Some(&original_line) = lines.get(line_idx) {
                if let Ok(re_obj) = Regex::new(&format!(r"(\w+)\.{}", regex::escape(prop))) {
                    if re_obj.is_match(original_line) {
                        let after_line = re_obj.replace(original_line, format!("($1 as any).{}", prop));
                        return Some(FixSuggestion {
                            file: error.file.clone(),
                            line: error.line,
                            before: original_line.to_string(),
                            after: after_line.to_string(),


                            confidence: 0.9,
            append_eof: false,
            top_of_file: false,
                            explanation: format!("TypeScript: Property '{}' is not defined on this type", prop),
                            suggested_fix: format!("Cast the object to 'any' before accessing '{}'", prop),
                        });
                    }
                }
            }
        }
    }

    // 2. Missing Brace (TS1005)
    if (code_str == "TS1005" || error.message.contains("TS1005")) && error.message.contains("'}' expected") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "\n}\n".to_string(),


            confidence: 0.85,
            append_eof: true,
            top_of_file: false,
            explanation: "TypeScript: Missing closing brace".to_string(),
            suggested_fix: "Appended missing closing brace '}' at the end of the file".to_string(),
        });
    }

    // 3. Type Mismatch in Argument (TS2345)
    if code_str == "TS2345" || error.message.contains("TS2345") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Convert argument to correct type".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: "TypeScript: Type mismatch in function argument".to_string(),
            suggested_fix: "Check argument type matches parameter type".to_string(),
        });
    }

    None
}
