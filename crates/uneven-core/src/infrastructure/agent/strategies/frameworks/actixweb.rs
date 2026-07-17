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

    if ext != "rs" {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_actix_project = detector::has_cargo_dependency(&proj_root, "actix-web");

    let msg_lower = error.message.to_lowercase();
    let has_actix_signature = error.message.contains("Responder")
        || error.message.contains("web::Path")
        || error.message.contains("web::Json")
        || msg_lower.contains("actix");

    if !is_actix_project && !has_actix_signature {
        return None;
    }

    // 1. Responder Trait Bounds not satisfied
    if error.message.contains("Responder")
        && (error.message.contains("not implemented") || error.message.contains("not satisfied"))
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let trimmed = original_line.trim();
            if !trimmed.is_empty() && !trimmed.ends_with(";") && !trimmed.starts_with("}") {
                let target = trimmed;
                let replacement = format!("HttpResponse::Ok().json({})", target);
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: original_line.replace(target, &replacement),


                    confidence: 0.80,
            append_eof: false,
            top_of_file: false,
                    explanation: "Actix-web: Handler function must return a type implementing the Responder trait.".to_string(),
                    suggested_fix: "Wrapped the return value in HttpResponse::Ok().json()".to_string(),
                });
            }
        }
    }

    // 2. Extractor signature mismatches
    if error.message.contains("Extractor")
        || (error.message.contains("Path") && error.message.contains("extractor"))
        || (error.message.contains("Json") && error.message.contains("extractor"))
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let is_path = error.message.contains("Path");
            let wrapper = if is_path { "web::Path" } else { "web::Json" };
            let re_arg = Regex::new(r"([a-zA-Z0-9_]+)\s*:\s*([a-zA-Z0-9_]+)").unwrap();
            
            if let Some(caps) = re_arg.captures(original_line) {
                let var_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let var_type = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let target1 = format!("{}: {}", var_name, var_type);
                let target2 = format!("{}:{}", var_name, var_type);
                
                let actual_target = if original_line.contains(&target1) { &target1 } else { &target2 };
                let replacement = format!("{}: {}<{}>", var_name, wrapper, var_type);
                
                if original_line.contains(actual_target) {
                    return Some(FixSuggestion {
                        file: error.file.clone(),
                        line: error.line,
                        before: original_line.to_string(),
                        after: original_line.replace(actual_target, &replacement),


                        confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                        explanation: "Actix-web: Route path/body arguments must be wrapped in extractors.".to_string(),
                        suggested_fix: format!("Wrapped the parameter type with {} in the route handler signature.", wrapper),
                    });
                }
            }
        }
    }

    None
}
