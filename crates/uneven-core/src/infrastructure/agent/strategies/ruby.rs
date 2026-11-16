use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_RUBY_NAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"undefined local variable or method [`'](\w+)['`]").unwrap());
static RE_RUBY_METHOD: Lazy<Regex> = Lazy::new(|| Regex::new(r"undefined method [`'](\w+)['`] for (.+)").unwrap());
static RE_RUBY_LOAD: Lazy<Regex> = Lazy::new(|| Regex::new(r"cannot load such file -- (.+)").unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_ruby = ["NameError", "NoMethodError", "SyntaxError", "LoadError", "TypeError"].contains(&code_str) 
        || ext == "rb";

    if !is_ruby {
        return None;
    }

    if code_str == "NameError" || error.message.contains("undefined local variable") {
        let name = RE_RUBY_NAME.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("identifier");
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("# Define: {} = nil", name),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Ruby: '{}' is not defined in this scope", name),
            suggested_fix: "Define the variable or method before using it, or check for typos".to_string(),
        });
    }

    if (code_str == "NoMethodError" || error.message.contains("undefined method"))
        && let Some(caps) = RE_RUBY_METHOD.captures(&error.message) {
            let method = caps.get(1).map(|m| m.as_str()).unwrap_or("method");
            let receiver = caps.get(2).map(|m| m.as_str()).unwrap_or("object");
            let is_nil = receiver.contains("nil");
            
            let lines: Vec<&str> = content.lines().collect();
            let line_idx = error.line.saturating_sub(1);
            if let Some(&original_line) = lines.get(line_idx) {
                if is_nil {
                    let target = format!(".{}", method);
                    let replacement = format!("&.{}", method);
                    if original_line.contains(&target) {
                        return Some(FixSuggestion {
                            file: error.file.clone(),
                            line: error.line,
                            before: original_line.to_string(),
                            after: original_line.replace(&target, &replacement),


                            confidence: 0.95,
            append_eof: false,
            top_of_file: false,
                            explanation: format!("Ruby: Method '{}' called on nil", method),
                            suggested_fix: format!("Replaced '.{}' with safe navigation '&.{}'", method, method),
                        });
                    }
                } else {
                    let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                    return Some(FixSuggestion {
                        file: error.file.clone(),
                        line: error.line,
                        before: original_line.to_string(),
                        after: format!("{}def {}\n{}end\n{}", indent, method, indent, original_line),


                        confidence: 0.8,
            append_eof: false,
            top_of_file: false,
                        explanation: format!("Ruby: Method '{}' does not exist on {}", method, receiver),
                        suggested_fix: format!("Injected empty method definition for '{}'", method),
                    });
                }
            }
        }

    if code_str == "SyntaxError" || error.message.contains("SyntaxError") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("# Review syntax near line {}", error.line),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Ruby: Syntax error — {}", error.message),
            suggested_fix: format!("Check for missing end keywords, mismatched brackets, or unclosed strings near line {}", error.line),
        });
    }

    if (code_str == "LoadError" || error.message.contains("cannot load such file"))
        && let Some(caps) = RE_RUBY_LOAD.captures(&error.message) {
            let lib = caps.get(1).map(|m| m.as_str()).unwrap_or("library");
            let gem_name = lib.split('/').next().unwrap_or("gem-name");
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: "".to_string(),
                after: format!("# Add to Gemfile: gem '{}'", gem_name),
                confidence: 0.2,
                append_eof: false,
                top_of_file: false,
                explanation: format!("Ruby: Cannot load '{}' — gem missing or wrong path", lib),
                suggested_fix: format!("Run 'gem install {}' or add to Gemfile", gem_name),
            });
        }

    if code_str == "TypeError" || error.message.contains("TypeError") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "# Check: obj.is_a?(ExpectedClass)".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Ruby: Type error — {}", error.message),
            suggested_fix: "Check object types match expected types before the operation".to_string(),
        });
    }

    None
}
