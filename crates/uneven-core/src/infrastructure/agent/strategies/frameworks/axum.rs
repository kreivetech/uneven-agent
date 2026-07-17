use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use crate::infrastructure::agent::strategies::detector;

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if ext != "rs" {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_rust_project = proj_root.join("Cargo.toml").exists();

    let has_axum_signature = error.message.contains("axum")
        || error.message.contains("Extractor")
        || error.message.contains("FromRequest")
        || error.message.contains("Handler");

    if !is_rust_project && !has_axum_signature {
        return None;
    }

    // 1. Axum Extractor Order Constraint
    if error.message.contains("cannot be used after")
        || (error.message.contains("axum") && error.message.contains("extractor") && error.message.contains("order"))
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            if original_line.contains("fn ") {
                let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}// FIXME: Extractors that consume the body (like Json/Form) must be the last argument.\n{}", indent, original_line),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "Rust/Axum: State or request extractors are declared in the incorrect order. Extractor consuming the request body must be last.".to_string(),
                    suggested_fix: "Injected FIXME comment indicating extractor order constraint".to_string(),
                });
            }
        }
    }

    // 2. Missing Send/Sync Bounds
    if error.message.contains("the trait bound")
        && (error.message.contains("Send") || error.message.contains("Sync"))
        && error.message.contains("axum")
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            if original_line.contains(".await") {
                let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}// FIXME: Ensure all non-Send types (like MutexGuard) are dropped before this await point.\n{}", indent, original_line),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "Rust/Axum: Route handler future is not Thread Safe (missing Send/Sync trait bounds), likely due to holding a non-Send type like MutexGuard across an await point.".to_string(),
                    suggested_fix: "Injected FIXME comment above the await point to warn about non-Send guards".to_string(),
                });
            }
        }
    }

    None
}
