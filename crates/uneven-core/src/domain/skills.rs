use catfive::{string, Validator};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/default_skills.rs"));

const SKILLS_VERSION_FILE: &str = ".version";
const CURRENT_SKILLS_VERSION: &str = env!("CARGO_PKG_VERSION");

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

/// Validates that a skill identifier is non-empty and free of path traversal or separators.
pub fn is_valid_skill_name(name: &str) -> bool {
    let clean = name.trim();
    string().min(1).validate(clean).is_ok()
        && !clean.contains("..")
        && clean
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Ensures the global `~/.uneven/config/skills/` directory is populated with the default skills.
pub fn ensure_default_skills() -> std::io::Result<()> {
    let root = crate::infrastructure::utils::paths::global_config_skills_dir();
    ensure_default_skills_in(&root)
}

/// Idempotently writes embedded default skills into `skills_root` when missing or outdated.
pub fn ensure_default_skills_in(skills_root: &Path) -> std::io::Result<()> {
    let version_path = skills_root.join(SKILLS_VERSION_FILE);
    if let Ok(existing) = fs::read_to_string(&version_path)
        && existing.trim() == CURRENT_SKILLS_VERSION
    {
        return Ok(());
    }

    fs::create_dir_all(skills_root)?;
    for (rel_path, content) in DEFAULT_SKILL_FILES {
        let target = skills_root.join(rel_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, content)?;
    }
    fs::write(&version_path, CURRENT_SKILLS_VERSION)?;
    Ok(())
}

/// Returns the operational instructions for a built-in skill by name (`zeroday` only).
pub fn get_builtin_skill(name: &str) -> Option<&'static str> {
    match name.trim().to_lowercase().as_str() {
        "zeroday" | "zeroday_audit" | "taint" | "vulnerability" => Some(SKILL_ZERODAY_AUDIT),
        _ => None,
    }
}

/// Lists built-in hardcoded skills (`zeroday` only).
pub fn list_builtin_skills() -> &'static [(&'static str, &'static str)] {
    &[(
        "zeroday",
        "Autonomous standards for deep multi-file taint tracing, sanitization bypass analysis, and zero-day vulnerability discovery.",
    )]
}

/// Extracts `(name, description)` from `SKILL.md` YAML frontmatter, falling back to `fallback_name`.
pub fn parse_skill_frontmatter(content: &str, fallback_name: &str) -> (String, String) {
    let mut name = fallback_name.to_string();
    let mut desc_lines: Vec<String> = Vec::new();

    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (name, String::new());
    }

    let mut in_description_block = false;
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if in_description_block {
            if line.starts_with(' ') || line.starts_with('\t') {
                if !trimmed.is_empty() {
                    desc_lines.push(trimmed.to_string());
                }
                continue;
            }
            in_description_block = false;
        }

        if let Some(rest) = trimmed.strip_prefix("name:") {
            let val = rest.trim().trim_matches(|c| c == '"' || c == '\'');
            if !val.is_empty() {
                name = val.to_string();
            }
        } else if let Some(rest) = trimmed.strip_prefix("description:") {
            let val = rest.trim();
            if val == ">-" || val == ">" || val == "|-" || val == "|" {
                in_description_block = true;
            } else {
                let clean = val.trim_matches(|c| c == '"' || c == '\'');
                if !clean.is_empty() {
                    desc_lines.push(clean.to_string());
                }
            }
        }
    }

    (name, desc_lines.join(" "))
}

/// Lists `zeroday` plus all skills available in `~/.uneven/config/skills/`.
pub fn list_available_skills() -> Vec<(String, String)> {
    let root = crate::infrastructure::utils::paths::global_config_skills_dir();
    list_available_skills_in(&root)
}

/// Lists `zeroday` plus all skills discovered in `skills_root`.
pub fn list_available_skills_in(skills_root: &Path) -> Vec<(String, String)> {
    let mut skills: Vec<(String, String)> = list_builtin_skills()
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();

    let Ok(entries) = fs::read_dir(skills_root) else {
        return skills;
    };

    let mut global_skills: Vec<(String, String)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if dir_name.starts_with('.') || !is_valid_skill_name(dir_name) {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if skill_md.is_file()
            && let Ok(content) = fs::read_to_string(&skill_md)
        {
            let (name, desc) = parse_skill_frontmatter(&content, dir_name);
            global_skills.push((name, desc));
        }
    }

    global_skills.sort_by(|a, b| a.0.cmp(&b.0));
    skills.extend(global_skills);
    skills
}

fn collect_auxiliary_files(dir: &Path, skill_root: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if file_name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_auxiliary_files(&path, skill_root, out);
        } else if path.is_file()
            && let Ok(rel) = path.strip_prefix(skill_root)
        {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str != "SKILL.md" {
                out.push((rel_str, path));
            }
        }
    }
}

/// Loads `zeroday` or a full skill bundle (`SKILL.md` + auxiliary files) from `~/.uneven/config/skills/<name>/`.
pub fn load_skill_bundle(name: &str) -> Option<String> {
    let root = crate::infrastructure::utils::paths::global_config_skills_dir();
    load_skill_bundle_in(name, &root)
}

/// Loads `zeroday` or a full skill bundle (`SKILL.md` + auxiliary files) from `skills_root/<name>/`.
pub fn load_skill_bundle_in(name: &str, skills_root: &Path) -> Option<String> {
    if let Some(builtin) = get_builtin_skill(name) {
        return Some(builtin.to_string());
    }

    let normalized = name.trim().to_lowercase();
    if !is_valid_skill_name(&normalized) {
        return None;
    }

    let canon_root = fs::canonicalize(skills_root).ok()?;
    let canon_skill_dir = fs::canonicalize(canon_root.join(&normalized)).ok()?;
    if !canon_skill_dir.starts_with(&canon_root) || !canon_skill_dir.is_dir() {
        return None;
    }

    let skill_md_path = canon_skill_dir.join("SKILL.md");
    let main_content = fs::read_to_string(&skill_md_path).ok()?;

    let mut bundle = main_content;
    let mut aux_files = Vec::new();
    collect_auxiliary_files(&canon_skill_dir, &canon_skill_dir, &mut aux_files);
    aux_files.sort_by(|a, b| a.0.cmp(&b.0));

    for (rel_path, abs_path) in aux_files {
        if let Ok(aux_content) = fs::read_to_string(&abs_path) {
            bundle.push_str(&format!(
                "\n\n### Auxiliary File: {}/{}\n{}",
                normalized, rel_path, aux_content
            ));
        }
    }

    Some(bundle)
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
            if is_valid_skill_name(&clean_name) {
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
    use tempfile::tempdir;

    #[test]
    fn test_get_builtin_skill_only_zeroday() {
        assert!(get_builtin_skill("zeroday").is_some());
        assert!(get_builtin_skill("taint").is_some());
        assert!(get_builtin_skill("explain").is_none());
        assert!(get_builtin_skill("docs").is_none());
        assert!(get_builtin_skill("unknown_skill").is_none());
    }

    #[test]
    fn test_is_valid_skill_name_blocks_traversal() {
        assert!(is_valid_skill_name("clean-code-engineering"));
        assert!(is_valid_skill_name("b2b_email"));
        assert!(!is_valid_skill_name(""));
        assert!(!is_valid_skill_name("../etc/passwd"));
        assert!(!is_valid_skill_name("foo/bar"));
        assert!(!is_valid_skill_name("foo\\bar"));
    }

    #[test]
    fn test_ensure_default_skills_populates_twelve_skills() {
        let temp = tempdir().unwrap();
        ensure_default_skills_in(temp.path()).unwrap();

        assert!(temp.path().join(".version").is_file());
        let listed = list_available_skills_in(temp.path());
        // 1 builtin (zeroday) + 12 embedded default skills
        assert_eq!(listed.len(), 13);
        assert!(listed.iter().any(|(n, _)| n == "zeroday"));
        assert!(listed.iter().any(|(n, _)| n == "clean-code-engineering"));
        assert!(listed.iter().any(|(n, _)| n == "b2b-email-prospecting"));
    }

    #[test]
    fn test_load_skill_bundle_includes_auxiliary_files() {
        let temp = tempdir().unwrap();
        ensure_default_skills_in(temp.path()).unwrap();

        let bundle = load_skill_bundle_in("b2b-email-prospecting", temp.path()).unwrap();
        assert!(bundle.contains("# B2B Email Prospecting"));
        assert!(bundle.contains("### Auxiliary File: b2b-email-prospecting/references/smtp.md"));

        assert!(load_skill_bundle_in("../secret", temp.path()).is_none());
        assert!(load_skill_bundle_in("zeroday", temp.path()).is_some());
    }

    #[test]
    fn test_parse_use_skill_tags() {
        let text = "I need [USE_SKILL: clean-code-engineering] and [USE_SKILL: ../bad] and [USE_SKILL: zeroday].";
        let tags = parse_use_skill_tags(text);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains("clean-code-engineering"));
        assert!(tags.contains("zeroday"));
        assert!(!tags.contains("../bad"));
    }

    #[test]
    fn test_strip_use_skill_tags() {
        let text = "Sure! [USE_SKILL: clean-code-engineering] Here is the result.";
        let stripped = strip_use_skill_tags(text);
        assert_eq!(stripped.trim(), "Sure!  Here is the result.");
    }
}
