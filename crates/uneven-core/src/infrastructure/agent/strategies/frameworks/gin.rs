use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use crate::infrastructure::agent::strategies::detector;
use regex::Regex;

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if ext != "go" {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_go_project = proj_root.join("go.mod").exists();

    let has_gin_signature = error.message.contains("gin")
        || error.message.contains("ShouldBindJSON")
        || error.message.contains("BindJSON")
        || error.message.contains("gin.Context");

    if !is_go_project && !has_gin_signature {
        return None;
    }

    // 1. JSON Binding Pointer Mismatch (ShouldBindJSON requires pointer)
    if (error.message.contains("ShouldBindJSON") || error.message.contains("BindJSON"))
        && (error.message.contains("pointer") || error.message.contains("non-pointer") || error.message.contains("invalid argument"))
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let re_bind = Regex::new(r"(ShouldBindJSON|BindJSON)\(([a-zA-Z0-9_]+)\)").unwrap();
            if let Some(caps) = re_bind.captures(original_line) {
                let func = caps.get(1).map(|m| m.as_str()).unwrap_or("ShouldBindJSON");
                let var = caps.get(2).map(|m| m.as_str()).unwrap_or("req");
                let target = format!("{}({})", func, var);
                let replacement = format!("{}(&{})", func, var);
                
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: original_line.replace(&target, &replacement),


                    confidence: 0.9,
            append_eof: false,
            top_of_file: false,
                    explanation: "Go/Gin: JSON binding function requires a pointer to write the parsed request body.".to_string(),
                    suggested_fix: "Injected ampersand (&) to pass the struct by reference".to_string(),
                });
            }
        }
    }

    // 2. Missing Context Parameter in Route Handler Signature
    if error.message.contains("cannot use")
        && error.message.contains("type func")
        && error.message.contains("as type gin.HandlerFunc")
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            if original_line.contains("func ") && original_line.contains("()") {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: original_line.replace("()", "(c *gin.Context)"),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "Go/Gin: Route handler function signature does not match gin.HandlerFunc.".to_string(),
                    suggested_fix: "Injected 'c *gin.Context' into function parameters".to_string(),
                });
            }
        }
    }

    None
}
