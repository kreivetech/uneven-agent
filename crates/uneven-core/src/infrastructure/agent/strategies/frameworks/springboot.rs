use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use crate::infrastructure::agent::strategies::detector;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_BEAN: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)for\s+bean\s+['"]?(\w+)['"]?"#).unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if ext != "java" && ext != "kt" {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_spring_project = proj_root.join("pom.xml").exists()
        || proj_root.join("build.gradle").exists()
        || proj_root.join("gradlew").exists();

    let has_spring_signature = error.message.contains("Spring")
        || error.message.contains("NoSuchBeanDefinitionException")
        || error.message.contains("CircularDependencyException")
        || error.message.contains("UnsatisfiedDependencyException")
        || error.message.contains("Autowired");

    if !is_spring_project && !has_spring_signature {
        return None;
    }

    // 1. NoSuchBeanDefinitionException (Autowired/Inject bean missing stereotype annotation)
    if error.message.contains("NoSuchBeanDefinitionException")
        || (error.message.contains("UnsatisfiedDependencyException") && error.message.contains("NoSuchBeanDefinition"))
    {
        let bean_name = RE_BEAN.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("Bean");

        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            if original_line.contains("class ") {
                let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: format!("{}@Service\n{}", indent, original_line),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: format!("Spring Boot: Dependency injection failed because the target bean '{}' is not registered.", bean_name),
                    suggested_fix: "Injected @Service annotation above the class definition".to_string(),
                });
            }
        }
    }

    // 2. Circular Dependency / Unsatisfied Dependency due to cycles
    if error.message.contains("CircularDependencyException")
        || (error.message.contains("BeanCurrentlyInCreationException") && error.message.contains("circular"))
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let re_params = Regex::new(r"(\(.*?\))").unwrap();
            if let Some(caps) = re_params.captures(original_line) {
                let params = caps.get(1).map(|m| m.as_str()).unwrap_or("()");
                if params.len() > 2 { // Not empty ()
                    let target = params;
                    let replacement = target.replacen("(", "(@Lazy ", 1);
                    return Some(FixSuggestion {
                        file: error.file.clone(),
                        line: error.line,
                        before: original_line.to_string(),
                        after: original_line.replace(target, &replacement),


                        confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                        explanation: "Spring Boot: Mutual circular dependency injection detected.".to_string(),
                        suggested_fix: "Injected @Lazy annotation in constructor/method parameter to break the cycle".to_string(),
                    });
                }
            }
        }
    }

    None
}
