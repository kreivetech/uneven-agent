/// Pre-processing utility that sanitizes raw Markdown responses from the AI
/// EXCLUSIVELY for human presentation (in CLI terminal or visual IDE chat bubbles).
///
/// NOTE: The AI's internal memory/history turns MUST keep the raw response
/// so that subsequent turns and patch execution have 100% full-fidelity context.
pub fn clean_markdown_for_display(raw: &str) -> String {
    let mut cleaned_lines = Vec::new();
    let mut in_search_block = false;
    let mut in_code_block = false;

    for line in raw.lines() {
        let trimmed = line.trim();

        // 1. Filter out internal engine tags
        if trimmed.starts_with("[USE_SKILL:")
            || trimmed.starts_with("[NEED_FILE:")
            || trimmed.starts_with("[WEB_SEARCH:")
            || trimmed.starts_with("[WEB_URL:")
            || trimmed.starts_with("[CREATE_POD:")
            || trimmed.starts_with("[DISPATCH_POD:")
        {
            continue;
        }

        // Track code blocks
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            if !in_code_block {
                in_search_block = false;
            }
            cleaned_lines.push(line.to_string());
            continue;
        }

        // 2. Handle SEARCH/REPLACE diff markers (both inside and outside code blocks)
        if trimmed.starts_with("<<<<<<< SEARCH") || trimmed.starts_with("<<<<<<<") {
            in_search_block = true;
            continue;
        }

        if in_search_block && (trimmed.starts_with("=======") || trimmed == "=======") {
            in_search_block = false;
            continue;
        }

        if trimmed.starts_with(">>>>>>> REPLACE") || trimmed.starts_with(">>>>>>>") {
            in_search_block = false;
            continue;
        }

        // Also strip standalone conflict markers if any stray remains
        if trimmed.starts_with("=======") {
            continue;
        }

        // While inside the SEARCH section (old code to be replaced), skip it
        // so the user visually sees the clean replacement code rather than git conflicts
        if in_search_block {
            continue;
        }

        cleaned_lines.push(line.to_string());
    }

    let result = cleaned_lines.join("\n");
    // Strip trailing or excessive multiple empty lines
    collapse_excessive_newlines(&result)
}

fn collapse_excessive_newlines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut consecutive_newlines = 0;

    for c in text.chars() {
        if c == '\n' {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                out.push(c);
            }
        } else {
            consecutive_newlines = 0;
            out.push(c);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_search_replace_markers() {
        let raw = r#"Here is the proposed solution:

### File: src/main.rs
**Action**: `edit`
```rust
<<<<<<< SEARCH
fn old_func() {
    println!("old");
}
=======
fn new_func() {
    println!("new");
}
>>>>>>> REPLACE
```

Ready to apply."#;

        let cleaned = clean_markdown_for_display(raw);
        assert!(!cleaned.contains("<<<<<<< SEARCH"));
        assert!(!cleaned.contains("======="));
        assert!(!cleaned.contains(">>>>>>> REPLACE"));
        assert!(!cleaned.contains("fn old_func()"));
        assert!(cleaned.contains("fn new_func()"));
        assert!(cleaned.contains("Here is the proposed solution:"));
        assert!(cleaned.contains("Ready to apply."));
    }

    #[test]
    fn test_clean_search_replace_markers_outside_code_blocks() {
        let raw = "Before\n<<<<<<< SEARCH\nfoo\n=======\nbar\n>>>>>>> REPLACE\nAfter";
        let cleaned = clean_markdown_for_display(raw);
        assert!(!cleaned.contains("SEARCH"));
        assert!(!cleaned.contains("foo"));
        assert!(cleaned.contains("Before"));
        assert!(cleaned.contains("bar"));
        assert!(cleaned.contains("After"));
    }

    #[test]
    fn test_clean_internal_tags() {
        let raw = r#"[USE_SKILL: explain]
[NEED_FILE: src/lib.rs]
Explaining the module:
- Detail 1
- Detail 2"#;

        let cleaned = clean_markdown_for_display(raw);
        assert!(!cleaned.contains("[USE_SKILL:"));
        assert!(!cleaned.contains("[NEED_FILE:"));
        assert!(cleaned.contains("Explaining the module:"));
        assert!(cleaned.contains("- Detail 1"));
    }

    #[test]
    fn test_preserve_normal_code_blocks() {
        let raw = r#"Code example:
```json
{
  "name": "uneven",
  "version": "2.2.0"
}
```"#;

        let cleaned = clean_markdown_for_display(raw);
        assert!(cleaned.contains("```json"));
        assert!(cleaned.contains("\"version\": \"2.2.0\""));
    }
}
