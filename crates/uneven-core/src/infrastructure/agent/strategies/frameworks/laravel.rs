use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use crate::infrastructure::agent::strategies::detector;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_FIELD: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)Add\s+\[?([\w_]+)\]?\s+to"#).unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if ext != "php" {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_laravel_project = proj_root.join("artisan").exists() || proj_root.join("composer.json").exists();

    let has_laravel_signature = error.message.contains("Laravel")
        || error.message.contains("MassAssignmentException")
        || error.message.contains("fillable")
        || error.message.contains("CSRF");

    if !is_laravel_project && !has_laravel_signature {
        return None;
    }

    // 1. Mass Assignment Protection
    if error.message.contains("MassAssignmentException")
        || (error.message.contains("Add") && error.message.contains("fillable") && error.message.contains("mass assignment"))
    {
        let field_name = RE_FIELD.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("field_name");

        let target = "$fillable = [";
        let replacement = format!("$fillable = [\n        '{}',", field_name);
        if content.contains(target) {
            let lines: Vec<&str> = content.lines().collect();
            let line_idx = lines.iter().position(|&l| l.contains(target)).unwrap_or(0);
            if let Some(&original_line) = lines.get(line_idx) {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: line_idx + 1,
                    before: original_line.to_string(),
                    after: original_line.replace(target, &replacement),


                    confidence: 0.9,
            append_eof: false,
            top_of_file: false,
                    explanation: format!("Laravel: Mass assignment protection blocks inserting '{}' directly into the model database entry.", field_name),
                    suggested_fix: format!("Injected '{}' to the $fillable array in your Model class", field_name),
                });
            }
        }
    }

    // 2. CSRF Token Mismatch
    if error.message.contains("TokenMismatchException") || error.message.contains("CSRF token mismatch") {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let upper_line = original_line.to_uppercase();
            if upper_line.contains("<FORM") && upper_line.contains("POST") {
                let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}\n{}    @csrf", original_line, indent),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "Laravel: State-changing HTTP request failed due to missing CSRF token.".to_string(),
                    suggested_fix: "Injected @csrf directive inside the form block".to_string(),
                });
            }
        }
    }

    None
}
