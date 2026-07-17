use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use crate::infrastructure::agent::strategies::detector;
use regex::Regex;
use once_cell::sync::Lazy;

static RE_NAME: Lazy<Regex> = Lazy::new(|| Regex::new(r#"name\s+['"]?(\w+)['"]?"#).unwrap());

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if !["vue", "js", "jsx", "ts", "tsx"].contains(&ext.as_str()) {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_nuxt_project = detector::has_npm_dependency(&proj_root, "nuxt");

    let has_nuxt_signature = error.message.contains("useRuntimeConfig")
        || error.message.contains("useNuxtApp")
        || error.message.contains("Hydration node mismatch")
        || error.message.contains("definePageMeta")
        || error.message.contains("defineNuxtConfig")
        || error.message.contains("Text content did not match server-rendered HTML");

    if !is_nuxt_project && !has_nuxt_signature {
        return None;
    }

    // 1. SSR Hydration Mismatch
    if error.message.contains("Hydration")
        || error.message.contains("server-rendered HTML")
        || (error.message.contains("window is not defined") && ext == "vue")
        || (error.message.contains("document is not defined") && ext == "vue")
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let indent = original_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: original_line.to_string(),
                after: format!("{}<ClientOnly>\n{}\n{}</ClientOnly>", indent, original_line, indent),


                confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                explanation: "Nuxt: SSR Hydration mismatch detected. Browser-only API or element rendered mismatch between server and client.".to_string(),
                suggested_fix: "Wrapped the client-only block in a <ClientOnly> component".to_string(),
            });
        }
    }

    // 2. Auto-imports missing inside test/linter environment
    if error.message.contains("cannot find name")
        && (error.message.contains("useRuntimeConfig")
            || error.message.contains("useNuxtApp")
            || error.message.contains("useRoute")
            || error.message.contains("useRouter"))
    {
        let comp_name = RE_NAME.captures(&error.message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("useRuntimeConfig");

        return Some(FixSuggestion {
            file: error.file.clone(),
            line: 1,
            before: "".to_string(),
            after: format!("import {{ {} }} from '#imports'\n", comp_name),


            confidence: 0.85,
            append_eof: false,
            top_of_file: true,
            explanation: format!("Nuxt: Auto-imported composable '{}' is not resolved in this context.", comp_name),
            suggested_fix: format!("Injected explicit import for '{}' from '#imports'", comp_name),
        });
    }

    None
}
