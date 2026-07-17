use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use crate::infrastructure::agent::strategies::detector;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_PARAM: Lazy<Regex> = Lazy::new(|| Regex::new(r#"parameter\s+['"]?(\w+)['"]?\s+"#).unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if ext != "py" {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_fastapi_project = detector::has_python_dependency(&proj_root, "fastapi");

    let has_fastapi_signature = error.message.contains("fastapi")
        || error.message.contains("Pydantic")
        || error.message.contains("ValidationError")
        || error.message.contains("Path parameter");

    if !is_fastapi_project && !has_fastapi_signature {
        return None;
    }

    // 1. Path parameter mismatch
    if error.message.contains("Path parameter")
        && (error.message.contains("not defined") || error.message.contains("missing"))
    {
        let param_name = RE_PARAM.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("param");

        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            if original_line.contains("def ") && original_line.contains("(") {
                let replacement = original_line.replacen("(", &format!("({}, ", param_name), 1);
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: replacement,


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: format!("FastAPI: Route parameter '{}' defined in route decorator is missing from the function signature.", param_name),
                    suggested_fix: format!("Injected '{}' to your route handler function arguments", param_name),
                });
            }
        }
    }

    // 2. Missing async/await in path operation calling async code
    if error.message.contains("coroutine")
        && (error.message.contains("was never awaited") || error.message.contains("await"))
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let trimmed = original_line.trim_start();
            let indent = original_line[..original_line.len() - trimmed.len()].to_string();
            
            let after_line = if let Some(idx) = original_line.find("=") {
                let before = &original_line[..idx + 1];
                let after = original_line[idx + 1..].trim_start();
                format!("{} await {}", before, after)
            } else if let Some(stripped) = trimmed.strip_prefix("return ") {
                format!("{}return await {}", indent, stripped.trim_start())
            } else {
                format!("{}await {}", indent, trimmed)
            };

            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: original_line.to_string(),
                after: after_line,


                confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                explanation: "FastAPI: A coroutine was never awaited inside the route handler.".to_string(),
                suggested_fix: "Injected 'await' keyword before the coroutine invocation".to_string(),
            });
        }
    }

    None
}
