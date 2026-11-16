use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use crate::infrastructure::agent::strategies::detector;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_ROUTE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"Reverse for\s+['"]?([\w\-:]+)['"]?\s+"#).unwrap());

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
    let is_django_project = detector::has_python_dependency(&proj_root, "django");

    let msg_lower = error.message.to_lowercase();
    let has_django_signature = error.message.contains("NoReverseMatch")
        || error.message.contains("Reverse for")
        || msg_lower.contains("csrf")
        || msg_lower.contains("django");

    if !is_django_project && !has_django_signature {
        return None;
    }

    // 1. Reverse matching pattern missing / incorrect argument count
    if error.message.contains("NoReverseMatch") || error.message.contains("Reverse for") {
        let route_name = RE_ROUTE.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("url_name");

        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let target1 = format!("'{}'", route_name);
            let target2 = format!("\"{}\"", route_name);
            let replacement = format!("'{}', args=[id]", route_name);
            
            if original_line.contains("reverse(") && (original_line.contains(&target1) || original_line.contains(&target2)) {
                let actual_target = if original_line.contains(&target1) { &target1 } else { &target2 };
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: original_line.replace(actual_target, &replacement),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: format!("Django: Reverse URL matching failed for route '{}'.", route_name),
                    suggested_fix: format!("Injected args=[id] to reverse('{}') call", route_name),
                });
            }
        }
    }

    // 2. CSRF token missing / decorator issue
    if msg_lower.contains("csrf") {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx)
            && original_line.contains("def ") {
                let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}@csrf_exempt\n{}", indent, original_line),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "Django: CSRF verification failed or CSRF token is missing in the request.".to_string(),
                    suggested_fix: "Injected @csrf_exempt decorator above the view function".to_string(),
                });
            }
    }

    None
}
