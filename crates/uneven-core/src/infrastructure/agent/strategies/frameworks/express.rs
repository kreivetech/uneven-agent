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

    if !["js", "ts"].contains(&ext.as_str()) {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_express_project = detector::has_npm_dependency(&proj_root, "express");

    let msg_lower = error.message.to_lowercase();
    let has_express_signature = error.message.contains("Cannot set headers after they are sent")
        || error.message.contains("HeadersAlreadySent")
        || error.message.contains("req, res, next")
        || msg_lower.contains("express");

    if !is_express_project && !has_express_signature {
        return None;
    }

    // 1. Headers already sent
    if error.message.contains("Cannot set headers") || error.message.contains("HeadersAlreadySent") {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx)
            && (original_line.contains("res.send") || original_line.contains("res.json") || original_line.contains("res.redirect") || original_line.contains("res.status"))
                && !original_line.trim().starts_with("return ") {
                let target_idx = original_line.find("res.").unwrap_or(0);
                let before = &original_line[..target_idx];
                let after = &original_line[target_idx..];
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}return {}", before, after),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "Express: Attempting to send headers/response after it has already been sent to the client.".to_string(),
                    suggested_fix: "Injected early return to prevent subsequent response executions.".to_string(),
                });
            }
    }

    // 2. Incorrect signature for error handling middleware
    if error.message.contains("error middleware")
        || (error.message.contains("next") && error.message.contains("signature") && error.message.contains("middleware"))
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let re_args = Regex::new(r"(\((req,?\s*res|res,?\s*req).*?\))").unwrap();
            if let Some(caps) = re_args.captures(original_line) {
                let target = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                if !target.is_empty() {
                    return Some(FixSuggestion {
                        file: error.file.clone(),
                        line: error.line,
                        before: original_line.to_string(),
                        after: original_line.replace(target, "(err, req, res, next)"),


                        confidence: 0.90,
            append_eof: false,
            top_of_file: false,
                        explanation: "Express: Error handling middleware must declare exactly 4 parameters to be registered correctly.".to_string(),
                        suggested_fix: "Injected (err, req, res, next) into the error middleware signature".to_string(),
                    });
                }
            }
        }
    }

    None
}
