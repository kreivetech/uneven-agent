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
    let is_nest_project = detector::has_npm_dependency(&proj_root, "@nestjs/core")
        || detector::has_npm_dependency(&proj_root, "@nestjs/common");

    let has_nest_signature = error.message.contains("Nest cannot resolve dependency")
        || error.message.contains("Circular dependency")
        || error.message.contains("NestJS")
        || error.message.contains("Injectable")
        || error.message.contains("Nest");

    if !is_nest_project && !has_nest_signature {
        return None;
    }

    // 1. Circular dependency
    if error.message.contains("Circular dependency")
        || (error.message.contains("Nest cannot resolve dependency") && error.message.to_lowercase().contains("circular"))
    {
        let re_inject = Regex::new(r"@Inject\(([\w\.]+)\)").unwrap();
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            if let Some(caps) = re_inject.captures(original_line) {
                let inject_target = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let target = format!("@Inject({})", inject_target);
                let replacement = format!("@Inject(forwardRef(() => {}))", inject_target);
                if original_line.contains(&target) {
                    return Some(FixSuggestion {
                        file: error.file.clone(),
                        line: error.line,
                        before: original_line.to_string(),
                        after: original_line.replace(&target, &replacement),


                        confidence: 0.9,
            append_eof: false,
            top_of_file: false,
                        explanation: "NestJS: Circular dependency detected in the injection tree.".to_string(),
                        suggested_fix: "Injected forwardRef() in constructor injection".to_string(),
                    });
                }
            }
        }
    }

    // 2. Missing @Injectable()
    if error.message.contains("is not a valid provider")
        || error.message.contains("must be decorated with @Injectable()")
        || error.message.contains("Injectable")
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
            if original_line.contains("class ") {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}@Injectable()\n{}", indent, original_line),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "NestJS: Provider class is not decorated with @Injectable().".to_string(),
                    suggested_fix: "Injected @Injectable() decorator above the class definition".to_string(),
                });
            }
        }
    }

    None
}
