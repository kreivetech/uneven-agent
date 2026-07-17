use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_JAVA_UNDEF: Lazy<Regex> = Lazy::new(|| Regex::new(r"symbol:\s+(?:class|method|variable)\s+(\S+)").unwrap());
static RE_JAVA_TYPE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?:incompatible types|cannot be converted).*?:\s*(.+)").unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_java = code_str.starts_with("JAVA") 
        || ["NullPointerException", "ArrayIndexOutOfBoundsException", "ClassNotFoundException"].contains(&code_str)
        || ext == "java";

    if !is_java {
        return None;
    }

    if code_str == "JAVA_UNDEF" || error.message.contains("cannot find symbol") {
        let sym = RE_JAVA_UNDEF.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("unknown");
            
        let lines: Vec<&str> = content.lines().collect();
        if sym.chars().next().is_some_and(|c| c.is_uppercase()) {
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: 1,
                before: "".to_string(),
                after: format!("import java.util.*; // Auto-injected for {}\n", sym),


                confidence: 0.85,
            append_eof: false,
            top_of_file: true,
                explanation: format!("Java: Cannot find symbol '{}'", sym),
                suggested_fix: "Injected wildcard import at the top of the file".to_string(),
            });
        } else {
            let line_idx = error.line.saturating_sub(1);
            if let Some(&original_line) = lines.get(line_idx) {
                let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}Object {} = null;\n{}", indent, sym, original_line),


                    confidence: 0.7,
            append_eof: false,
            top_of_file: false,
                    explanation: format!("Java: Cannot find symbol '{}'", sym),
                    suggested_fix: format!("Injected local variable '{}'", sym),
                });
            }
        }
    }

    if code_str == "JAVA_SYNTAX" {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("// Review syntax near line {}", error.line),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Java: Syntax error — {}", error.message),
            suggested_fix: format!("Check for missing semicolons, parentheses, or braces near line {}", error.line),
        });
    }

    if code_str == "JAVA_TYPE" {
        let details = RE_JAVA_TYPE.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or(&error.message);
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Use explicit cast or change declared type".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Java: Type incompatibility — {}", details),
            suggested_fix: "Add explicit cast or change the variable type to match".to_string(),
        });
    }

    if code_str == "JAVA_RETURN" || error.message.contains("missing return statement") {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: original_line.to_string(),
                after: format!("{}return null;\n{}", indent, original_line),


                confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                explanation: "Java: Missing return statement".to_string(),
                suggested_fix: "Injected 'return null;' before the closing brace".to_string(),
            });
        }
    }

    if code_str == "NullPointerException" || error.message.contains("NullPointerException") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Add: if (obj != null) { ... } or use Optional<T>".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: "Java: NullPointerException — accessing a method or field on a null reference".to_string(),
            suggested_fix: "Add null check before dereferencing the object".to_string(),
        });
    }

    if code_str == "ArrayIndexOutOfBoundsException" || error.message.contains("ArrayIndexOutOfBoundsException") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Add: if (i >= 0 && i < arr.length) { ... }".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: "Java: Array index out of bounds".to_string(),
            suggested_fix: "Validate array index before access: check index < array.length".to_string(),
        });
    }

    if code_str == "ClassNotFoundException" || error.message.contains("ClassNotFoundException") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Add dependency to pom.xml / build.gradle".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: "Java: Class not found — missing from classpath".to_string(),
            suggested_fix: "Add the required JAR to classpath or check the fully-qualified class name".to_string(),
        });
    }

    None
}
