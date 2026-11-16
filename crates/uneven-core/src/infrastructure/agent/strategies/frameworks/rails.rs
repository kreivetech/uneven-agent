use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use crate::infrastructure::agent::strategies::detector;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_PARAM_1: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)missing\s+or\s+the\s+value\s+is\s+empty:\s+(\w+)"#).unwrap());
static RE_PARAM_2: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)key\s+not\s+found:\s+["':]?(\w+)"#).unwrap());
static RE_UNPERMITTED: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)unpermitted\s+parameter[s]?:\s+[:]?(\w+)"#).unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if ext != "rb" {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_rails_project = proj_root.join("Gemfile").exists() || proj_root.join("config").join("application.rb").exists();

    let has_rails_signature = error.message.contains("Rails")
        || error.message.contains("ActionController")
        || error.message.contains("ActiveRecord")
        || error.message.contains("ParameterMissing");

    if !is_rails_project && !has_rails_signature {
        return None;
    }

    // 1. Unpermitted parameters (Strong Parameters)
    if error.message.contains("Unpermitted parameters") || error.message.contains("unpermitted parameter") {
        let param_name = RE_UNPERMITTED.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("field_name");

        let lines: Vec<&str> = content.lines().collect();
        if let Some(line_idx) = lines.iter().position(|l| l.contains(".permit(")) {
            let original_line = lines[line_idx];
            let target = ".permit(";
            let replacement = format!(".permit(:{}, ", param_name);
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: line_idx + 1,
                before: original_line.to_string(),
                after: original_line.replace(target, &replacement),


                confidence: 0.9,
            append_eof: false,
            top_of_file: false,
                explanation: format!("Ruby on Rails: Strong Parameters blocked the unpermitted attribute '{}'.", param_name),
                suggested_fix: format!("Injected '{}' into the permit() list", param_name),
            });
        }
    }

    // 1.5. ParameterMissing (Strong Parameters)
    if error.message.contains("ParameterMissing")
        || (error.message.contains("param is missing or the value is empty") && error.message.contains("ActionController"))
    {
        let param_name = RE_PARAM_1.captures(&error.message)
            .or_else(|| RE_PARAM_2.captures(&error.message))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("model_name");

        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("# Controller Example:\n# params.require(:{}).permit(:field1, :field2)", param_name),


            confidence: 0.4,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Ruby on Rails: Strong Parameters check failed because the expected root payload key '{}' is missing.", param_name),
            suggested_fix: format!("Ensure your request payload wraps the input under a '{}' JSON key, or modify controller to permit flat parameters", param_name),
        });
    }

    // 2. ActiveRecord::RecordNotFound
    if error.message.contains("RecordNotFound") || error.message.contains("Couldn't find") {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx)
            && original_line.contains(".find(") {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: original_line.replace(".find(", ".find_by(id: "),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "Ruby on Rails: Querying database with find() failed to locate the record, raising a 404 exception.".to_string(),
                    suggested_fix: "Converted find() to find_by(id: ) to return nil instead of raising an exception".to_string(),
                });
            }
    }

    None
}
