use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;

pub fn suggest(error: &ErrorEvent, _content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_markup = ["svelte", "vue", "astro", "jsx", "tsx"].contains(&ext.as_str()) || 
           code_str == "SVELTE_ERR" || code_str == "VUE_ERR" || code_str == "ASTRO_ERR" || 
           code_str == "TSX_ERR" || code_str == "JSX_ERR";

    if !is_markup {
        return None;
    }

    let unclosed_tag_regex = regex::Regex::new(r"(?:[Uu]nclosed element|[Ee]xpected) <(/?)(\w+)>?").unwrap();
    if error.message.contains("unclosed") || error.message.contains("Expected </") || error.message.contains("Unexpected EOF") {
        let tag = unclosed_tag_regex.captures(&error.message)
            .and_then(|c| c.get(2))
            .map(|m| m.as_str())
            .unwrap_or("div");
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("\n</{}>\n", tag),


            confidence: 0.85,
            append_eof: true,
            top_of_file: false,
            explanation: format!("Markup: Unclosed tag '{}'", tag),
            suggested_fix: format!("Appended missing closing tag </{}> at the end of the file", tag),
        });
    }

    if ext == "svelte" || code_str == "SVELTE_ERR" {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "<!-- Fix Svelte template syntax -->".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Svelte: Error during compilation — {}", error.message),
            suggested_fix: "Check Svelte specific syntax: unclosed {{#if}} blocks, invalid $: reactive statements, or unclosed HTML tags".to_string(),
        });
    }

    if ext == "vue" || code_str == "VUE_ERR" {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "<!-- Fix Vue template syntax -->".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Vue: Error during compilation — {}", error.message),
            suggested_fix: "Check Vue specific syntax: unclosed <template> tags, invalid v-bind/v-on directives, or Setup script errors".to_string(),
        });
    }

    if ext == "astro" || code_str == "ASTRO_ERR" {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Fix Astro component syntax".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Astro: Error during compilation — {}", error.message),
            suggested_fix: "Check Astro specific syntax: unclosed frontmatter (---), invalid JSX within HTML, or missing component imports".to_string(),
        });
    }

    if ext == "jsx" || ext == "tsx" || code_str == "JSX_ERR" || code_str == "TSX_ERR" {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "{/* Fix JSX syntax */}".to_string(),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("React/JSX: Error during compilation — {}", error.message),
            suggested_fix: "Check JSX specific syntax: unclosed tags, 'className' instead of 'class', or invalid inline styles".to_string(),
        });
    }

    None
}
