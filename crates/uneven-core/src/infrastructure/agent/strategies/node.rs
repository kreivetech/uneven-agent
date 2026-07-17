use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_node = ["TypeError", "ReferenceError", "SyntaxError", "Error"].contains(&code_str) && 
           (ext == "js" || ext == "cjs" || ext == "mjs");

    if !is_node {
        return None;
    }

    match code_str {
        "TypeError" => {
            if error.message.contains("is not a function") {
                let re_not_func = regex::Regex::new(r"(.+?) is not a function").unwrap();
                if let Some(caps) = re_not_func.captures(&error.message) {
                    let func_name = caps.get(1).map(|m| m.as_str()).unwrap_or("func");
                    let lines: Vec<&str> = content.lines().collect();
                    let line_idx = error.line.saturating_sub(1);
                    if let Some(&original_line) = lines.get(line_idx) {
                        let target = format!("{}()", func_name);
                        let replacement = format!("{}?.()", func_name);
                        if original_line.contains(&target) {
                            return Some(FixSuggestion {
                                file: error.file.clone(),
                                line: error.line,
                                before: original_line.to_string(),
                                after: original_line.replace(&target, &replacement),


                                confidence: 0.9,
            append_eof: false,
            top_of_file: false,
                                explanation: format!("Node.js: Type error — {}", error.message),
                                suggested_fix: "Added optional chaining before function call".to_string(),
                            });
                        }
                    }
                }
            }
            if error.message.contains("Cannot read properties of undefined") || error.message.contains("Cannot read properties of null") {
                let re_read_props = regex::Regex::new(r"reading '(.+?)'").unwrap();
                if let Some(caps) = re_read_props.captures(&error.message) {
                    let prop = caps.get(1).map(|m| m.as_str()).unwrap_or("prop");
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
                                explanation: "Node.js: Cannot read property on null or undefined".to_string(),
                                suggested_fix: format!("Added optional chaining (?.) before accessing '{}'", prop),
                            });
                        }
                    }
                }
            }
        }
        "ReferenceError" => {
            let re_not_defined = regex::Regex::new(r"(\w+) is not defined").unwrap();
            if let Some(caps) = re_not_defined.captures(&error.message) {
                let var_name = caps.get(1).map(|m| m.as_str()).unwrap_or("var");
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
                        explanation: format!("Node.js: Reference error — {}", error.message),
                        suggested_fix: format!("Injected default initialization for '{}'", var_name),
                    });
                }
            }
        }
        "SyntaxError" => {
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: "".to_string(),
                after: format!("// Review syntax near line {}", error.line),
                confidence: 0.3,
            append_eof: false,
            top_of_file: false,
                explanation: format!("Node.js: Syntax error — {}", error.message),
                suggested_fix: format!("Check for missing brackets, quotes, or invalid syntax near line {}", error.line),
            });
        }
        _ => {}
    }

    None
}
