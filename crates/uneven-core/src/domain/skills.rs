use std::collections::HashSet;

pub const SKILL_EXPLAIN: &str = r#"### Operational Standards — Architecture & Code Explanation
When explaining files, functions, or systems:
- Visual Flow: Diagram the data flow and component lifecycle using Mermaid (`flowchart TD` or `sequenceDiagram`).
- Invariants & Lifecycle: Detail pre-conditions, invariants, state transitions, and side-effects.
- Failure Modes & Edge Cases: Explicitly highlight potential error paths, concurrency considerations, and performance bottlenecks.
- Structure: Organize explanations cleanly with:
  1. Purpose & Responsibilities
  2. Data Flow (including Mermaid diagram)
  3. Key Invariants & Design Decisions
  4. Notable Considerations or Trade-offs
- Keep descriptions technical, concise, and actionable without conversational filler.
"#;

pub const SKILL_DOCS: &str = r#"### Operational Standards — Polyglot Documentation & In-Place Comments
When documenting code:
- Idiomatic Language Standards: Adhere strictly to the target language's standard conventions:
  * Rust (`.rs`): Use triple-slash docstrings (`///`) on public items. Include `# Arguments`, `# Returns`, `# Errors` (if `Result`), and `# Examples` where appropriate.
  * TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`): Use TSDoc/JSDoc block comments (`/** ... */`) with `@param {Type} name - description`, `@returns {Type}`, and `@throws`.
  * Python (`.py`): Use Google-style docstrings with `Args:`, `Returns:`, and `Raises:` sections.
  * Go (`.go`): Use standard GoDoc comments (`// SymbolName describes...`) starting with the exported symbol name.
- In-Place Patch Generation: When the goal is documenting existing files, generate the modifications as a unified patch (`### File: <path>`, `**Action**: Edit`) so they can be reviewed and applied directly via `patch_engine`.
- Preservation: Do NOT alter existing logic, implementation code, or function signatures. Only add or refine documentation comments.
"#;

pub const SKILL_ZERODAY_AUDIT: &str = r#"### Operational Standards — Autonomous Zero-Day Vulnerability & Taint Flow Audit
When conducting an autonomous zero-day and deep taint flow investigation:
- Source-to-Sink Taint Tracing: Methodically trace untrusted entry vectors (HTTP bodies/query, URL parameters, headers, WebSockets, CLI flags, file uploads, deserialization streams) down to critical execution sinks (`std::process::Command`, raw SQL queries, dynamic eval, reflection, unconstrained filesystem operations, IPC, memory transmutations).
- Multi-File Context Resolution: Use `[NEED_FILE: <path>]` whenever an input crosses component boundaries (controller -> service -> helper -> repository) to trace the end-to-end payload lifecycle. Never assume a helper or middleware sanitizes without inspecting its source code.
- Sanitization & Logic Bypass:
  * Check if regex sanitizers are anchored (`^...$`), handle multiline inputs, or allow bypass through encodings or null bytes.
  * Check for race conditions (TOCTOU) in file, database, or state management operations.
  * Check for prototype pollution, object injection, or unsafe deserialization.
  * Verify authorization and permission barriers on every state-mutating operation.
- Directive Output Protocol:
  When a definitive zero-day vulnerability or high-risk unvalidated flow is confirmed, emit a structured directive tag:
  `[ZERODAY_FINDING: <severity> | <finding_type> | <file:line> | <description> | <recommendation> | <taint_trail>]`
  * Allowed Severities: Critical, High, Medium, Low.
  * Taint Trail: Concise arrow-separated propagation path (e.g., `src/api.rs:45 (req.query) -> src/query.rs:12 (raw SQL)`).
- Defensive Remediation: For every confirmed flaw, provide actionable, minimal, safe code remediation guidance adhering to the principle of least privilege.
"#;

/// Returns the operational instructions for a built-in skill by name.
pub fn get_builtin_skill(name: &str) -> Option<&'static str> {
    match name.trim().to_lowercase().as_str() {
        "explain" | "architecture" | "flow" => Some(SKILL_EXPLAIN),
        "docs" | "doc" | "document" | "documentation" => Some(SKILL_DOCS),
        "zeroday" | "zeroday_audit" | "taint" | "vulnerability" => Some(SKILL_ZERODAY_AUDIT),
        _ => None,
    }
}

/// Lists all available built-in skills with their key and summary.
pub fn list_builtin_skills() -> &'static [(&'static str, &'static str)] {
    &[
        ("explain", "Standards for architectural decomposition, component lifecycles, and visual Mermaid diagrams."),
        ("docs", "Standards for idiomatic polyglot API documentation & in-place docstrings (Rustdoc, TSDoc, Python, Go) via unified patch."),
        ("zeroday", "Autonomous standards for deep multi-file taint tracing, sanitization bypass analysis, and zero-day vulnerability discovery."),
    ]
}

/// Parses all `[USE_SKILL: <skill_name>]` tags from an agent response.
pub fn parse_use_skill_tags(text: &str) -> HashSet<String> {
    let mut skills = HashSet::new();
    let mut start_idx = 0;
    while let Some(open_pos) = text[start_idx..].find("[USE_SKILL:") {
        let abs_open = start_idx + open_pos;
        if let Some(close_pos) = text[abs_open..].find(']') {
            let tag_content = &text[abs_open + 11..abs_open + close_pos];
            let clean_name = tag_content.trim().trim_matches('`').to_lowercase();
            if !clean_name.is_empty() {
                skills.insert(clean_name);
            }
            start_idx = abs_open + close_pos + 1;
        } else {
            break;
        }
    }
    skills
}

/// Strips all `[USE_SKILL: ...]` tags from text so they remain completely silent to the user.
pub fn strip_use_skill_tags(text: &str) -> String {
    let mut result = text.to_string();
    while let Some(open_pos) = result.find("[USE_SKILL:") {
        if let Some(close_pos) = result[open_pos..].find(']') {
            let end_idx = open_pos + close_pos + 1;
            result.replace_range(open_pos..end_idx, "");
        } else {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_builtin_skill() {
        assert!(get_builtin_skill("explain").is_some());
        assert!(get_builtin_skill("docs").is_some());
        assert!(get_builtin_skill("DOCS").is_some());
        assert!(get_builtin_skill("zeroday").is_some());
        assert!(get_builtin_skill("taint").is_some());
        assert!(get_builtin_skill("unknown_skill").is_none());
    }

    #[test]
    fn test_parse_use_skill_tags() {
        let text = "I need [USE_SKILL: docs] and [USE_SKILL: explain] to complete this.";
        let tags = parse_use_skill_tags(text);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains("docs"));
        assert!(tags.contains("explain"));
    }

    #[test]
    fn test_strip_use_skill_tags() {
        let text = "Sure! [USE_SKILL: docs] Here is the documentation.";
        let stripped = strip_use_skill_tags(text);
        assert_eq!(stripped.trim(), "Sure!  Here is the documentation.");
    }
}
