use catfive::{string, Validator};

/// Fixed protocol describing all unified inline directives available to reasoning stages.
/// Descriptions mirror `laws.rs` verbatim to prevent drift or hallucinations.
pub const DIRECTIVES_PROTOCOL: &str = r#"## Available System Directives
You may emit ONE or MORE of the following triggers in any stage when needed; resolved results are returned in the conversation before the next step:
- `[NEED_FILE: relative/path/to/file]` — Load the full content of a workspace file into context.
- `[NEED_DIR: group_id]` or `[NEED_DIR: group_id | page]` — Expand the paginated file list of a directory group from the workspace files map.
- `[WEB_SEARCH: query]` — Search external library docs, API references, or error messages on the web.
- `[WEB_URL: url]` — Fetch full documentation content from an `http(s)://` web page.
- `[USE_SKILL: skill_name]` — Dynamically load specialized operational guidelines for any skill listed under Available Skills below.
- `[EXEC_CMD: <single-line command>]` — Request execution of a single-line terminal command (no `]` inside the command). Requires user approval; when approved, its exit code and output are returned to context before the next step (or listed as final commands in synthesis).
- `[CREATE_POD: <Pod Title> | <Prime Specialist Role> | <Instructions> | <file1>, <file2>]` — Autonomously instantiate a domain-specialized Agent Pod when you identify an architectural subsystem that warrants isolated focus, specialized law, and domain boundaries (users cannot manually create Pods)."#;

/// Appends the fixed `DIRECTIVES_PROTOCOL` and the compact available skills list
/// to a stage-specific `base_law`.
pub fn with_directives_protocol(base_law: &str) -> String {
    let skills_list = crate::domain::skills::list_available_skills()
        .into_iter()
        .map(|(name, desc)| {
            if desc.is_empty() {
                format!("- `{}`", name)
            } else {
                format!("- `{}`: {}", name, desc)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{}\n\n{}\n\n### Available Skills for `[USE_SKILL: skill_name]`\n{}",
        base_law.trim_end(),
        DIRECTIVES_PROTOCOL,
        skills_list
    )
}

/// Unified inline directives emitted by AI agents across `think`, `ask`, and audit pipelines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentDirective {
    NeedFile(String),
    NeedDir {
        id: usize,
        page: usize,
    },
    UseSkill(String),
    ExecCmd(String),
    WebSearch(String),
    WebUrl(String),
    CreatePod {
        title: String,
        role: String,
        instructions: String,
        files: Vec<String>,
    },
}

const KNOWN_TAGS: &[&str] = &[
    "NEED_FILE",
    "NEED_DIR",
    "USE_SKILL",
    "EXEC_CMD",
    "WEB_SEARCH",
    "WEB_URL",
    "CREATE_POD",
];

fn clean_payload(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c: char| c == '`' || c == '"' || c == '\'')
        .trim()
        .to_string()
}

fn validate_non_empty(value: &str) -> bool {
    string().min(1).validate(value).is_ok()
}

fn validate_safe_rel_path(path: &str) -> bool {
    if !validate_non_empty(path) {
        return false;
    }
    let normalized = path.replace('\\', "/");
    !normalized.split('/').any(|seg| seg == "..")
}

fn validate_http_url(url: &str) -> bool {
    validate_non_empty(url) && (url.starts_with("http://") || url.starts_with("https://"))
}

fn build_directive(tag: &str, raw_payload: &str) -> Option<AgentDirective> {
    let payload = clean_payload(raw_payload);

    match tag {
        "NEED_FILE" => {
            if validate_safe_rel_path(&payload) {
                Some(AgentDirective::NeedFile(payload))
            } else {
                None
            }
        }
        "NEED_DIR" => {
            let parts: Vec<&str> = payload.split('|').map(|s| s.trim()).collect();
            if let Some(id_str) = parts.first()
                && validate_non_empty(id_str)
                && let Ok(id) = id_str.parse::<usize>()
            {
                let page = parts
                    .get(1)
                    .and_then(|p| p.parse::<usize>().ok())
                    .filter(|&p| p >= 1)
                    .unwrap_or(1);
                Some(AgentDirective::NeedDir { id, page })
            } else {
                None
            }
        }
        "USE_SKILL" => {
            let normalized = payload.to_lowercase();
            if validate_non_empty(&normalized)
                && normalized
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                Some(AgentDirective::UseSkill(normalized))
            } else {
                None
            }
        }
        "EXEC_CMD" => {
            if validate_non_empty(&payload) {
                Some(AgentDirective::ExecCmd(payload))
            } else {
                None
            }
        }
        "WEB_SEARCH" => {
            if validate_non_empty(&payload) {
                Some(AgentDirective::WebSearch(payload))
            } else {
                None
            }
        }
        "WEB_URL" => {
            if validate_http_url(&payload) {
                Some(AgentDirective::WebUrl(payload))
            } else {
                None
            }
        }
        "CREATE_POD" => {
            let parts: Vec<&str> = raw_payload.split('|').map(|s| s.trim()).collect();
            if parts.len() >= 3
                && validate_non_empty(parts[0])
                && validate_non_empty(parts[1])
                && validate_non_empty(parts[2])
            {
                let files = parts
                    .get(3)
                    .map(|f| {
                        f.split(',')
                            .map(|s| clean_payload(s))
                            .filter(|s| validate_safe_rel_path(s))
                            .collect()
                    })
                    .unwrap_or_default();
                Some(AgentDirective::CreatePod {
                    title: parts[0].to_string(),
                    role: parts[1].to_string(),
                    instructions: parts[2].to_string(),
                    files,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parses and validates all recognized `[TAG: payload]` directives in a single pass.
pub fn parse_directives(text: &str) -> Vec<AgentDirective> {
    let mut directives = Vec::new();
    let mut cursor = 0;

    while let Some(open_rel) = text[cursor..].find('[') {
        let open_abs = cursor + open_rel;
        let rest = &text[open_abs + 1..];

        let Some(close_rel) = rest.find(']') else {
            break;
        };

        let inner = &rest[..close_rel];
        if let Some((raw_tag, raw_payload)) = inner.split_once(':') {
            let tag = raw_tag.trim().to_ascii_uppercase();
            if KNOWN_TAGS.contains(&tag.as_str()) {
                if let Some(directive) = build_directive(&tag, raw_payload) {
                    directives.push(directive);
                }
                cursor = open_abs + 1 + close_rel + 1;
                continue;
            }
        }

        cursor = open_abs + 1;
    }

    directives
}

/// Strips recognized `[TAG: payload]` directive tags from displayed Markdown text
/// while preserving regular Markdown brackets such as `[link](url)` or `[0]`.
pub fn strip_directives(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;

    while let Some(open_rel) = text[cursor..].find('[') {
        let open_abs = cursor + open_rel;
        let rest = &text[open_abs + 1..];

        let Some(close_rel) = rest.find(']') else {
            break;
        };

        let inner = &rest[..close_rel];
        let is_known_tag = inner
            .split_once(':')
            .map(|(t, _)| KNOWN_TAGS.contains(&t.trim().to_ascii_uppercase().as_str()))
            .unwrap_or(false);

        if is_known_tag {
            out.push_str(&text[cursor..open_abs]);
            cursor = open_abs + 1 + close_rel + 1;
        } else {
            out.push_str(&text[cursor..open_abs + 1]);
            cursor = open_abs + 1;
        }
    }

    out.push_str(&text[cursor..]);
    out
}

/// Resolves operational instructions for `zeroday` or a global system skill bundle
/// requested via `AgentDirective::UseSkill`.
pub fn resolve_skill(name: &str) -> Option<String> {
    let clean = clean_payload(name).to_lowercase();
    if !validate_non_empty(&clean) {
        return None;
    }
    crate::domain::skills::load_skill_bundle(&clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mixed_directives() {
        let input = "\
            Inspect [NEED_FILE: src/main.rs], expand [NEED_DIR: 2 | 3] and load [USE_SKILL: explain].\n\
            Also check [WEB_SEARCH: rust tokio spawn_blocking] and [WEB_URL: https://docs.rs/tokio].\n\
            Run [EXEC_CMD: `cargo test`] and [CREATE_POD: Auth | Security Lead | Audit JWT | src/auth.rs, src/jwt.rs].";

        let parsed = parse_directives(input);
        assert_eq!(
            parsed,
            vec![
                AgentDirective::NeedFile("src/main.rs".to_string()),
                AgentDirective::NeedDir { id: 2, page: 3 },
                AgentDirective::UseSkill("explain".to_string()),
                AgentDirective::WebSearch("rust tokio spawn_blocking".to_string()),
                AgentDirective::WebUrl("https://docs.rs/tokio".to_string()),
                AgentDirective::ExecCmd("cargo test".to_string()),
                AgentDirective::CreatePod {
                    title: "Auth".to_string(),
                    role: "Security Lead".to_string(),
                    instructions: "Audit JWT".to_string(),
                    files: vec!["src/auth.rs".to_string(), "src/jwt.rs".to_string()],
                },
            ]
        );
    }

    #[test]
    fn test_catfive_rejects_invalid_directives() {
        let input = "\
            [NEED_FILE: ] \
            [NEED_FILE: ../../etc/passwd] \
            [USE_SKILL: invalid skill!] \
            [EXEC_CMD:   ] \
            [WEB_URL: ftp://example.com] \
            [CREATE_POD: OnlyTitle]";

        let parsed = parse_directives(input);
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_strip_directives_preserves_markdown() {
        let input = "See [docs](https://example.com) and [0]. [EXEC_CMD: cargo check] Done [USE_SKILL: docs].";
        let stripped = strip_directives(input);
        assert_eq!(stripped, "See [docs](https://example.com) and [0].  Done .");
    }

    #[test]
    fn test_resolve_skill() {
        let zeroday = resolve_skill("zeroday").unwrap();
        assert!(zeroday.contains("Zero-Day Vulnerability"));

        assert!(resolve_skill("explain").is_none());
        assert!(resolve_skill("docs").is_none());
        assert!(resolve_skill("../etc/passwd").is_none());
        assert!(resolve_skill("nonexistent").is_none());
    }

    #[test]
    fn test_with_directives_protocol_includes_skills() {
        let out = with_directives_protocol("Base law");
        assert!(out.contains("Available System Directives"));
        assert!(out.contains("Available Skills for `[USE_SKILL: skill_name]`"));
        assert!(out.contains("- `zeroday`:"));
    }
}
