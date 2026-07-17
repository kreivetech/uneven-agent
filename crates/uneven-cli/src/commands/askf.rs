use crate::ui::truecolor;
use anyhow::Result;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::knowledge::incremental_index::IncrementalIndex;
use uneven_core::infrastructure::llm::{
    infer_with_params_streaming, init as init_llm, InferParams,
};
use uneven_core::infrastructure::security::SafetyGuard;

#[derive(Debug, Clone)]
pub enum PatchType {
    SearchReplace {
        search: String,
        replace: String,
    },
    InsertBefore {
        anchor_before: String,
        insert_before: String,
    },
    InsertAfter {
        anchor_after: String,
        insert_after: String,
    },
}

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub filename: String,
    pub content: String,
    pub patches: Vec<PatchType>, // Alterado de Vec<SearchReplacePatch> para Vec<PatchType>
    pub is_patch: bool,
    pub delete: bool,
    pub rename_to: Option<String>,
}

pub fn normalize_path(path: &Path) -> std::path::PathBuf {
    let components = path.components();
    let mut normalized = std::path::PathBuf::new();

    for component in components {
        match component {
            std::path::Component::Prefix(..) => {
                normalized.push(component.as_os_str());
            }
            std::path::Component::RootDir => {
                normalized.push(component.as_os_str());
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Normal(c) => {
                normalized.push(c);
            }
        }
    }
    normalized
}

pub fn is_safe_path_internal(name: &str, root: &Path, external_dir: Option<&Path>) -> bool {
    let path = Path::new(name);

    // Resolve relative and absolute paths
    let abs_target = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };

    let normalized = normalize_path(&abs_target);
    let norm_root = normalize_path(root);

    // Check if descendant of project root
    if normalized.starts_with(&norm_root) {
        return true;
    }

    // Check if descendant of external project root
    if let Some(ext) = external_dir {
        let norm_ext = normalize_path(ext);
        if normalized.starts_with(&norm_ext) {
            return true;
        }
    }

    false
}

pub fn is_safe_filename(name: &str) -> bool {
    let clean_name = name.replace('\\', "/");
    let root = loader::find_project_root(None);
    let config = loader::load_config(&root).ok();
    let ext_project = config
        .as_ref()
        .and_then(|c| c.knowledge.as_ref())
        .and_then(|k| k.external_project.as_ref())
        .map(std::path::PathBuf::from);

    is_safe_path_internal(&clean_name, &root, ext_project.as_deref())
}

pub fn build_prompt(user_request: &str, session_id: &str) -> String {
    format!(
        "Remember to strictly use the `file: {{}}` object format for all file actions. Use the unique session ID:
        {}\n\nUser request: {}",
        session_id, user_request
    )
}

pub fn parse_files_with_id(response: &str, session_id: &str) -> Vec<ParsedFile> {
    let mut files = Vec::new();

    let escaped_id = regex::escape(session_id);
    let pattern = format!(
        r#"(?i)FILE_{}:\s*(?:\n)?([^\n]+)\n([\s\S]*?)END_(?:FILE_)?{}"#,
        escaped_id, escaped_id
    );
    let re = match regex::Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => return files,
    };

    let patch_re =
        regex::Regex::new(r"(?s)<<<<<<< SEARCH\n(.*?)\n=======\n(.*?)\n>>>>>>> REPLACE").unwrap();
    let fence_re = regex::Regex::new(r"(?s)```[\w]*\n(.*?)```").unwrap();

    for cap in re.captures_iter(response) {
        let filename = cap[1].trim().to_string();
        if !is_safe_filename(&filename) {
            continue;
        }

        let raw_content = cap[2].trim();

        if raw_content == "DELETE" {
            files.push(ParsedFile {
                filename,
                content: String::new(),
                patches: Vec::new(),
                is_patch: false,
                delete: true,
                rename_to: None,
            });
        } else if let Some(stripped) = raw_content.strip_prefix("RENAME:") {
            let new_name = stripped.trim().to_string();
            if is_safe_filename(&new_name) {
                files.push(ParsedFile {
                    filename,
                    content: String::new(),
                    patches: Vec::new(),
                    is_patch: false,
                    delete: false,
                    rename_to: Some(new_name),
                });
            }
        } else if raw_content.contains("<<<<<<< SEARCH") && raw_content.contains(">>>>>>> REPLACE")
        {
            let mut patches = Vec::new();
            for pcap in patch_re.captures_iter(raw_content) {
                patches.push(PatchType::SearchReplace {
                    search: pcap[1].to_string(),
                    replace: pcap[2].to_string(),
                });
            }
            if !patches.is_empty() {
                files.push(ParsedFile {
                    filename,
                    content: String::new(),
                    patches,
                    is_patch: true,
                    delete: false,
                    rename_to: None,
                });
            }
        } else {
            let content = if let Some(fcap) = fence_re.captures(raw_content) {
                fcap[1].to_string()
            } else {
                raw_content.to_string()
            };
            files.push(ParsedFile {
                filename,
                content,
                patches: Vec::new(),
                is_patch: false,
                delete: false,
                rename_to: None,
            });
        }
    }
    files
}

pub fn parse_files(response: &str) -> Vec<ParsedFile> {
    let mut files = Vec::new();
    let patch_re =
        regex::Regex::new(r"(?s)<<<<<<< SEARCH\n(.*?)\n=======\n(.*?)\n>>>>>>> REPLACE").unwrap();
    let fence_re = regex::Regex::new(r"(?s)```[\w]*\n(.*?)```").unwrap();

    // Fallback for any FILE_<id>: <filename>\n([\s\S]*?)END_FILE_<id>
    if let Ok(re) = regex::Regex::new(
        r#"(?i)FILE_[a-zA-Z0-9]{5,6}:\s*(?:\n)?([^\n]+)\n([\s\S]*?)END_(?:FILE_)?[a-zA-Z0-9]{5,6}"#,
    ) {
        for cap in re.captures_iter(response) {
            let filename = cap[1].trim().to_string();
            if is_safe_filename(&filename) {
                let mut raw_content = cap[2].trim();
                if raw_content.starts_with("PATCH") {
                    raw_content = raw_content.strip_prefix("PATCH").unwrap().trim();
                }
                if raw_content == "DELETE" {
                    files.push(ParsedFile {
                        filename,
                        content: String::new(),
                        patches: Vec::new(),
                        is_patch: false,
                        delete: true,
                        rename_to: None,
                    });
                } else if let Some(stripped) = raw_content.strip_prefix("RENAME:") {
                    let new_name = stripped.trim().to_string();
                    if is_safe_filename(&new_name) {
                        files.push(ParsedFile {
                            filename,
                            content: String::new(),
                            patches: Vec::new(),
                            is_patch: false,
                            delete: false,
                            rename_to: Some(new_name),
                        });
                    }
                } else if raw_content.contains("<<<<<<< SEARCH")
                    && raw_content.contains(">>>>>>> REPLACE")
                {
                    let mut patches = Vec::new();
                    for pcap in patch_re.captures_iter(raw_content) {
                        patches.push(PatchType::SearchReplace {
                            search: pcap[1].to_string(),
                            replace: pcap[2].to_string(),
                        });
                    }
                    if !patches.is_empty() {
                        files.push(ParsedFile {
                            filename,
                            content: String::new(),
                            patches,
                            is_patch: true,
                            delete: false,
                            rename_to: None,
                        });
                    }
                } else {
                    let content = if let Some(fcap) = fence_re.captures(raw_content) {
                        fcap[1].to_string()
                    } else {
                        raw_content.to_string()
                    };
                    files.push(ParsedFile {
                        filename,
                        content,
                        patches: Vec::new(),
                        is_patch: false,
                        delete: false,
                        rename_to: None,
                    });
                }
            }
        }
    }
    if !files.is_empty() {
        return files;
    }

    // Fallback for regular FILE: <filename>\n([\s\S]*?)END_FILE
    if let Ok(re) = regex::Regex::new(r#"(?i)FILE:\s*(?:\n)?([^\n]+)\n([\s\S]*?)END_FILE"#) {
        for cap in re.captures_iter(response) {
            let filename = cap[1].trim().to_string();
            if is_safe_filename(&filename) {
                let raw_content = cap[2].trim();
                if raw_content == "DELETE" {
                    files.push(ParsedFile {
                        filename,
                        content: String::new(),
                        patches: Vec::new(),
                        is_patch: false,
                        delete: true,
                        rename_to: None,
                    });
                } else if let Some(stripped) = raw_content.strip_prefix("RENAME:") {
                    let new_name = stripped.trim().to_string();
                    if is_safe_filename(&new_name) {
                        files.push(ParsedFile {
                            filename,
                            content: String::new(),
                            patches: Vec::new(),
                            is_patch: false,
                            delete: false,
                            rename_to: Some(new_name),
                        });
                    }
                } else if raw_content.contains("<<<<<<< SEARCH")
                    && raw_content.contains(">>>>>>> REPLACE")
                {
                    let mut patches = Vec::new();
                    for pcap in patch_re.captures_iter(raw_content) {
                        patches.push(PatchType::SearchReplace {
                            search: pcap[1].to_string(),
                            replace: pcap[2].to_string(),
                        });
                    }
                    if !patches.is_empty() {
                        files.push(ParsedFile {
                            filename,
                            content: String::new(),
                            patches,
                            is_patch: true,
                            delete: false,
                            rename_to: None,
                        });
                    }
                } else {
                    let content = if let Some(fcap) = fence_re.captures(raw_content) {
                        fcap[1].to_string()
                    } else {
                        raw_content.to_string()
                    };
                    files.push(ParsedFile {
                        filename,
                        content,
                        patches: Vec::new(),
                        is_patch: false,
                        delete: false,
                        rename_to: None,
                    });
                }
            }
        }
    }
    if !files.is_empty() {
        return files;
    }

    // Code block hints: ```lang\n// path/to/file.ext\n...\n```
    if let Ok(re) = regex::Regex::new(
        r"```(?:\w+)?\n(?:\s*(?://|#|<!--)\s*([a-zA-Z0-9_./\- ]+(?:\.[a-zA-Z0-9]+)?)\s*(?:-->)?\n)([\s\S]*?)```",
    ) {
        for cap in re.captures_iter(response) {
            let filename = cap[1].trim().to_string();
            if is_safe_filename(&filename) {
                let raw_content = &cap[2];
                if raw_content.contains("<<<<<<< SEARCH") && raw_content.contains(">>>>>>> REPLACE")
                {
                    let mut patches = Vec::new();
                    for pcap in patch_re.captures_iter(raw_content) {
                        patches.push(PatchType::SearchReplace {
                            search: pcap[1].to_string(),
                            replace: pcap[2].to_string(),
                        });
                    }
                    if !patches.is_empty() {
                        files.push(ParsedFile {
                            filename,
                            content: String::new(),
                            patches,
                            is_patch: true,
                            delete: false,
                            rename_to: None,
                        });
                    }
                } else {
                    files.push(ParsedFile {
                        filename,
                        content: raw_content.to_string(),
                        patches: Vec::new(),
                        is_patch: false,
                        delete: false,
                        rename_to: None,
                    });
                }
            }
        }
    }
    files
}

/// Parse the new object-based file format: file: { session_id: ..., filename: ..., ... }
pub fn parse_file_objects(response: &str, session_id: &str) -> Vec<ParsedFile> {
    let mut files = Vec::new();

    // Extract the main file: { ... } container blocks first using the balanced brace strategy
    let main_blocks = extract_brace_blocks(response);

    for main_block in main_blocks {
        // Extract session_id field and validate
        let obj_session = extract_field(&main_block, "session_id").unwrap_or_default();
        if !obj_session.is_empty() && obj_session != session_id {
            continue;
        }

        // Extract filename (required)
        let filename = match extract_field(&main_block, "filename") {
            Some(f) => f,
            None => continue,
        };

        if !is_safe_filename(&filename) {
            continue;
        }

        // Check for delete
        if let Some(del_val) = extract_field(&main_block, "delete") {
            if del_val == "true" {
                files.push(ParsedFile {
                    filename,
                    content: String::new(),
                    patches: Vec::new(),
                    is_patch: false,
                    delete: true,
                    rename_to: None,
                });
                continue;
            }
        }

        // Check for rename
        if let Some(new_name) = extract_field(&main_block, "rename") {
            if is_safe_filename(&new_name) {
                files.push(ParsedFile {
                    filename,
                    content: String::new(),
                    patches: Vec::new(),
                    is_patch: false,
                    delete: false,
                    rename_to: Some(new_name),
                });
            }
            continue;
        }

        // Check for patches (nested array of objects supporting multiple modification types)
        if let Some(patches_array_content) = extract_array_block(&main_block, "patches") {
            // ONLY extract inner brace blocks from the isolated patches array content context
            let patch_objects = extract_brace_blocks(&patches_array_content);
            let mut patches_vec = Vec::new();

            for patch_block in patch_objects {
                // Check if it is a patch of Insert Before
                let anchor_before = extract_multiline_field(&patch_block, "anchor_before");
                let insert_before = extract_multiline_field(&patch_block, "insert_before");
                if let (Some(anchor), Some(content)) = (anchor_before, insert_before) {
                    patches_vec.push(PatchType::InsertBefore {
                        anchor_before: anchor,
                        insert_before: content,
                    });
                    continue;
                }

                // Check if it is a patch of Insert After
                let anchor_after = extract_multiline_field(&patch_block, "anchor_after");
                let insert_after = extract_multiline_field(&patch_block, "insert_after");
                if let (Some(anchor), Some(content)) = (anchor_after, insert_after) {
                    patches_vec.push(PatchType::InsertAfter {
                        anchor_after: anchor,
                        insert_after: content,
                    });
                    continue;
                }

                // Fallback for Traditional Search and Replace
                let search = extract_multiline_field(&patch_block, "search").unwrap_or_default();
                let replace = extract_multiline_field(&patch_block, "replace").unwrap_or_default();
                if !search.is_empty() {
                    patches_vec.push(PatchType::SearchReplace { search, replace });
                }
            }

            if !patches_vec.is_empty() {
                files.push(ParsedFile {
                    filename,
                    content: String::new(),
                    patches: patches_vec,
                    is_patch: true,
                    delete: false,
                    rename_to: None,
                });
            }
            continue;
        }

        // Check for legacy single patch
        if let Some(patch_block) = extract_nested_block(&main_block, "patch") {
            let search = extract_multiline_field(&patch_block, "search").unwrap_or_default();
            let replace = extract_multiline_field(&patch_block, "replace").unwrap_or_default();
            if !search.is_empty() {
                files.push(ParsedFile {
                    filename,
                    content: String::new(),
                    patches: vec![PatchType::SearchReplace { search, replace }],
                    is_patch: true,
                    delete: false,
                    rename_to: None,
                });
            }
            continue;
        }

        // Otherwise it's a file create — extract content
        if let Some(content) = extract_multiline_field(&main_block, "content") {
            files.push(ParsedFile {
                filename,
                content,
                patches: Vec::new(),
                is_patch: false,
                delete: false,
                rename_to: None,
            });
        }
    }

    files
}

/// Extract a simple single-line field value: `key: value`, `key: "value"` or `key: `value``
fn extract_field(block: &str, key: &str) -> Option<String> {
    for line in block.lines() {
        let trimmed = line.trim();
        // Match `key:` at the start of the line (but not `key_other:`)
        if let Some(rest) = trimmed.strip_prefix(key) {
            let rest = rest.trim_start();
            if let Some(value_part) = rest.strip_prefix(':') {
                let mut val = value_part.trim();
                // Strip trailing comma first
                val = val.strip_suffix(',').unwrap_or(val).trim();
                // Strip double quotes
                val = val.strip_prefix('"').unwrap_or(val);
                val = val.strip_suffix('"').unwrap_or(val);
                // Strip single quotes
                val = val.strip_prefix('\'').unwrap_or(val);
                val = val.strip_suffix('\'').unwrap_or(val);
                // NEW: Strip backticks if the AI used them here
                val = val.strip_prefix('`').unwrap_or(val);
                val = val.strip_suffix('`').unwrap_or(val);

                val = val.trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

/// Extract a nested block: `key: { ... }` — returns the inner content
fn extract_nested_block(block: &str, key: &str) -> Option<String> {
    let pattern = format!("{}:", key);
    let pos = block.find(&pattern)?;
    let after_key = &block[pos + pattern.len()..];
    let brace_pos = after_key.find('{')?;
    let brace_start = pos + pattern.len() + brace_pos;

    let mut depth = 1;
    let bytes = block.as_bytes();
    let mut i = brace_start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'`' => {
                // Skip everything inside backticks (quoted field values)
                i += 1;
                while i < bytes.len() && bytes[i] != b'`' {
                    i += 1;
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(block[brace_start + 1..i].to_string());
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Extracts content inside an array: `key: [ ... ]` — returns the inner content string
fn extract_array_block(block: &str, key: &str) -> Option<String> {
    let pattern = format!("{}:", key);
    let pos = block.find(&pattern)?;
    let after_key = &block[pos + pattern.len()..];
    let bracket_pos = after_key.find('[')?;
    let bracket_start = pos + pattern.len() + bracket_pos;

    let mut depth = 1;
    let bytes = block.as_bytes();
    let mut i = bracket_start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'`' => {
                // Skip everything inside backticks (quoted field values)
                i += 1;
                while i < bytes.len() && bytes[i] != b'`' {
                    i += 1;
                }
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(block[bracket_start + 1..i].to_string());
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Extracts each individual patch block object by balancing curly braces `{}`.
/// This completely unties the extraction from strict line-by-line start markers.
fn extract_brace_blocks(content: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i;
            let mut depth = 1;
            i += 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'`' => {
                        // Skip everything inside backticks (quoted field values)
                        i += 1;
                        while i < bytes.len() && bytes[i] != b'`' {
                            i += 1;
                        }
                    }
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            // Extracted the complete raw block safely between { and }
                            blocks.push(content[start + 1..i].to_string());
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        i += 1;
    }

    blocks
}

/// Extract a multiline field value that is strictly enclosed within single backticks (`...`).
fn extract_multiline_field(block: &str, key: &str) -> Option<String> {
    let pattern = format!("{}:", key);
    let pos = block.find(&pattern)?;
    let after_key = &block[pos + pattern.len()..];

    // Aggressively trim leading/trailing whitespace and newlines to expose the backtick
    let mut after_key = after_key.trim();

    // Remove raw external quotes if the LLM mistakenly wrapped the backticks inside them
    if after_key.starts_with('"') || after_key.starts_with('\'') {
        after_key = after_key[1..after_key.len() - 1].trim();
    }

    // Look for the starting backtick using the safer, idiomatic strip_prefix method
    if let Some(stripped) = after_key.strip_prefix('`') {
        // Find the matching closing backtick in the remaining string sequence
        if let Some(closing_pos) = stripped.find('`') {
            let content = &stripped[..closing_pos];
            return Some(content.to_string());
        }
    }

    // Fallback for fields not enclosed in backticks (legacy single-line support)
    let mut value_lines = Vec::new();
    let fields = [
        "session_id",
        "filename",
        "language",
        "content",
        "delete",
        "rename",
        "patch",
        "patches",
        "search",
        "replace",
        "anchor_before",
        "insert_before",
        "anchor_after",
        "insert_after",
    ];

    for line in after_key.lines() {
        let trimmed = line.trim();
        let mut is_new_field = false;
        for f in &fields {
            let f_pattern = format!("{}:", f);
            if trimmed.starts_with(&f_pattern) {
                is_new_field = true;
                break;
            }
        }
        if is_new_field {
            break;
        }
        value_lines.push(line);
    }

    // Join extracted lines and remove any trailing structural comma or whitespace
    let joined = value_lines.join("\n");
    let content = joined.trim().trim_end_matches(',').trim();
    if !content.is_empty() {
        let mut final_content = content;
        if final_content.starts_with('"') && final_content.ends_with('"') {
            final_content = &final_content[1..final_content.len() - 1];
        }
        Some(final_content.to_string())
    } else {
        None
    }
}

fn backup_to_obsolete(root: &Path, file_path: &Path, relative_filename: &str) {
    if !file_path.exists() || !file_path.is_file() {
        return;
    }
    let obsolete_dir = root.join(".uneven").join("obsolete");
    let safe_rel = relative_filename.replace('\\', "/");
    let target_path = obsolete_dir.join(&safe_rel);

    if let Some(parent) = target_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let date_str = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let file_name_with_timestamp = if let Some(ext) = target_path.extension() {
        if let Some(name) = target_path.file_stem() {
            format!(
                "{}_{}.{}",
                name.to_string_lossy(),
                date_str,
                ext.to_string_lossy()
            )
        } else {
            format!(
                "{}_{}",
                target_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                date_str
            )
        }
    } else {
        format!(
            "{}_{}",
            target_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            date_str
        )
    };

    let final_target_path = target_path.with_file_name(file_name_with_timestamp);
    let _ = fs::copy(file_path, &final_target_path);
}

pub fn write_parsed_files(root: &std::path::Path, files: Vec<ParsedFile>) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    println!();
    println!(
        "{} {}",
        truecolor("  › ", 168, 200, 255),
        truecolor(
            &format!(
                "Requested {} file modification{}...",
                files.len(),
                if files.len() > 1 { "s" } else { "" }
            ),
            107,
            122,
            150
        )
    );
    println!();

    let mut overwrite_all = false;
    let mut index = IncrementalIndex::new(root);
    let re_file_newlines = regex::Regex::new(r"\n{3,}").ok();

    for file in files {
        let safe_filename = file.filename.replace('\\', "/");
        let file_path = root.join(&safe_filename);
        let norm_file_path = normalize_path(&file_path);

        let config = loader::load_config(root).ok();
        let ext_project = config
            .as_ref()
            .and_then(|c| c.knowledge.as_ref())
            .and_then(|k| k.external_project.as_ref())
            .map(std::path::PathBuf::from);

        let mut path_is_safe = norm_file_path.starts_with(normalize_path(root));
        if !path_is_safe {
            if let Some(ref ext) = ext_project {
                if norm_file_path.starts_with(normalize_path(ext)) {
                    path_is_safe = true;
                }
            }
        }

        if !path_is_safe {
            println!(
                "{} {}",
                truecolor("  ⚠ ", 255, 107, 53),
                truecolor(
                    &format!("Skipping unsafe path: {}", file.filename),
                    107,
                    122,
                    150
                )
            );
            continue;
        }

        let exists = file_path.exists();

        if file.delete {
            if exists {
                backup_to_obsolete(root, &file_path, &file.filename);
                if let Err(e) = fs::remove_file(&file_path) {
                    println!(
                        "{} {}",
                        truecolor("  ⚠ ", 255, 107, 53),
                        truecolor(
                            &format!("Failed to delete {}: {}", file.filename, e),
                            107,
                            122,
                            150
                        )
                    );
                } else {
                    index.remove_file(file_path.to_str().unwrap_or(&file.filename));
                    println!(
                        "{} {} {}",
                        truecolor("  ✓ ", 0, 255, 140),
                        truecolor(&file.filename, 232, 237, 245),
                        truecolor("(Deleted successfully)", 107, 122, 150)
                    );
                }
            } else {
                println!(
                    "{} {}",
                    truecolor("  ⚠ ", 255, 107, 53),
                    truecolor(
                        &format!("File {} does not exist to be deleted", file.filename),
                        107,
                        122,
                        150
                    )
                );
            }
        } else if let Some(ref new_name) = file.rename_to {
            let new_path = root.join(new_name.replace('\\', "/"));
            let norm_new_path = normalize_path(&new_path);

            let config = loader::load_config(root).ok();
            let ext_project = config
                .as_ref()
                .and_then(|c| c.knowledge.as_ref())
                .and_then(|k| k.external_project.as_ref())
                .map(std::path::PathBuf::from);

            let mut new_path_is_safe = norm_new_path.starts_with(normalize_path(root));
            if !new_path_is_safe {
                if let Some(ref ext) = ext_project {
                    if norm_new_path.starts_with(normalize_path(ext)) {
                        new_path_is_safe = true;
                    }
                }
            }

            if !new_path_is_safe {
                println!(
                    "{} {}",
                    truecolor("  ⚠ ", 255, 107, 53),
                    truecolor(
                        &format!("Skipping unsafe rename target path: {}", new_name),
                        107,
                        122,
                        150
                    )
                );
                continue;
            }
            if exists {
                if let Some(parent) = new_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Err(e) = fs::rename(&file_path, &new_path) {
                    println!(
                        "{} {}",
                        truecolor("  ⚠ ", 255, 107, 53),
                        truecolor(
                            &format!("Failed to rename {} to {}: {}", file.filename, new_name, e),
                            107,
                            122,
                            150
                        )
                    );
                } else {
                    index.remove_file(file_path.to_str().unwrap_or(&file.filename));
                    let hash = fs::read(&new_path)
                        .ok()
                        .map(|b| IncrementalIndex::hash_buffer(&b))
                        .unwrap_or_default();
                    index.mark_indexed_with_hash(
                        new_path.to_str().unwrap_or(new_name),
                        hash,
                        fs::metadata(&new_path).map(|m| m.len()).unwrap_or(0),
                        Vec::new(),
                    );

                    println!(
                        "{} {} {} {}",
                        truecolor("  ✓ ", 0, 255, 140),
                        truecolor(&file.filename, 232, 237, 245),
                        truecolor("moved to", 107, 122, 150),
                        truecolor(new_name, 232, 237, 245)
                    );
                }
            } else {
                println!(
                    "{} {}",
                    truecolor("  ⚠ ", 255, 107, 53),
                    truecolor(
                        &format!("File {} does not exist to be moved", file.filename),
                        107,
                        122,
                        150
                    )
                );
            }
        } else if file.is_patch {
            if !exists {
                println!(
                    "{} {}",
                    truecolor("  ⚠ ", 255, 107, 53),
                    truecolor(
                        &format!("Cannot apply patch: File {} does not exist", file.filename),
                        107,
                        122,
                        150
                    )
                );
                continue;
            }

            let mut current_content = match fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    println!(
                        "{} {}",
                        truecolor("  ⚠ ", 255, 107, 53),
                        truecolor(
                            &format!("Failed to read {}: {}", file.filename, e),
                            107,
                            122,
                            150
                        )
                    );
                    continue;
                }
            };

            let mut all_applied = true;
            for patch_type in &file.patches {
                match patch_type {
                    PatchType::SearchReplace { search, replace } => {
                        if current_content.contains(search) {
                            current_content = current_content.replace(search, replace);
                        } else {
                            let search_lf = search.replace("\r\n", "\n");
                            let content_lf = current_content.replace("\r\n", "\n");
                            if content_lf.contains(&search_lf) {
                                current_content =
                                    content_lf.replace(&search_lf, &replace.replace("\r\n", "\n"));
                            } else {
                                println!(
                                    "{} Could not find search block in {}",
                                    truecolor("  ⚠ ", 255, 107, 53),
                                    file.filename
                                );
                                all_applied = false;
                                break;
                            }
                        }
                    }
                    PatchType::InsertBefore {
                        anchor_before,
                        insert_before,
                    } => {
                        if current_content.contains(anchor_before) {
                            current_content = current_content.replace(
                                anchor_before,
                                &format!("{}{}", insert_before, anchor_before),
                            );
                        } else {
                            let anchor_lf = anchor_before.replace("\r\n", "\n");
                            let current_lf = current_content.replace("\r\n", "\n");
                            if current_lf.contains(&anchor_lf) {
                                current_content = current_lf.replace(
                                    &anchor_lf,
                                    &format!(
                                        "{}{}",
                                        insert_before.replace("\r\n", "\n"),
                                        anchor_lf
                                    ),
                                );
                            } else {
                                println!(
                                    "{} Could not find anchor_before in {}",
                                    truecolor("  ⚠ ", 255, 107, 53),
                                    file.filename
                                );
                                all_applied = false;
                                break;
                            }
                        }
                    }
                    PatchType::InsertAfter {
                        anchor_after,
                        insert_after,
                    } => {
                        if current_content.contains(anchor_after) {
                            current_content = current_content.replace(
                                anchor_after,
                                &format!("{}{}", anchor_after, insert_after),
                            );
                        } else {
                            let anchor_lf = anchor_after.replace("\r\n", "\n");
                            let current_lf = current_content.replace("\r\n", "\n");
                            if current_lf.contains(&anchor_lf) {
                                current_content = current_lf.replace(
                                    &anchor_lf,
                                    &format!("{}{}", anchor_lf, insert_after.replace("\r\n", "\n")),
                                );
                            } else {
                                println!(
                                    "{} Could not find anchor_after in {}",
                                    truecolor("  ⚠ ", 255, 107, 53),
                                    file.filename
                                );
                                all_applied = false;
                                break;
                            }
                        }
                    }
                }
            }

            if all_applied {
                if let Some(re) = &re_file_newlines {
                    current_content = re.replace_all(&current_content, "\n\n").to_string();
                }

                current_content = current_content.trim_start_matches('\n').to_string();

                fs::write(&file_path, &current_content)?;

                let hash = IncrementalIndex::hash_buffer(current_content.as_bytes());
                if let Some(path_str) = file_path.to_str() {
                    index.mark_indexed_with_hash(
                        path_str,
                        hash,
                        current_content.len() as u64,
                        Vec::new(),
                    );
                }

                println!(
                    "{} {}{}",
                    truecolor("  ✓ ", 0, 255, 140),
                    truecolor(&file.filename, 232, 237, 245),
                    truecolor(" (Patched surgically)", 107, 122, 150)
                );
            }
        } else {
            if exists && !overwrite_all {
                print!(
                    "{} {}",
                    truecolor("  ? ", 255, 107, 53),
                    truecolor(
                        &format!("File {} exists. Overwrite? [y/n/a] ", file.filename),
                        107,
                        122,
                        150
                    )
                );
                std::io::stdout().flush()?;

                let mut choice = String::new();
                std::io::stdin().read_line(&mut choice)?;
                let choice = choice.trim().to_lowercase();

                if choice == "a" {
                    overwrite_all = true;
                } else if choice != "y" {
                    println!("     Skipping {}", file.filename);
                    continue;
                }
            }

            if exists {
                backup_to_obsolete(root, &file_path, &file.filename);
            }

            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&file_path, &file.content)?;

            let hash = IncrementalIndex::hash_buffer(file.content.as_bytes());
            if let Some(path_str) = file_path.to_str() {
                index.mark_indexed_with_hash(path_str, hash, file.content.len() as u64, Vec::new());
            }

            let size_kb = (file.content.len() as f64 / 1024.0).to_string();
            println!(
                "{} {}{}",
                truecolor("  ✓ ", 0, 255, 140),
                truecolor(&file.filename, 232, 237, 245),
                truecolor(
                    &format!(" ({:.1} KB)", size_kb.parse::<f64>().unwrap_or(0.0)),
                    107,
                    122,
                    150
                )
            );
        }
    }
    let _ = index.save();
    println!();
    Ok(())
}

pub async fn run(
    task: String,
    provider_opt: Option<String>,
    model_opt: Option<String>,
) -> Result<()> {
    // 1. Safety Guard Validation (Shield)
    let safety = SafetyGuard::new();
    if let Err(err) = safety.validate_intent(&task) {
        println!(
            "\n{} {}",
            crate::ui::truecolor_bold("⚠️ [SECURITY POLICY]:", 255, 107, 53),
            err
        );
        return Ok(());
    }

    let session_id = uuid::Uuid::new_v4().to_string()[..6].to_uppercase();

    // 2. Load config
    let root = loader::find_project_root(None);
    let config = loader::load_config(&root)?;
    let mut brain = config.get_brain_for_command("askf");
    if let Some(ref p) = provider_opt {
        brain.provider = p.clone();
    }
    if let Some(ref m) = model_opt {
        brain.model = m.clone();
    }

    // 3. Assemble Context
    let context = super::context_assembler::ContextAssembler::assemble(
        &task,
        &config,
        &brain,
        &root,
        None,
        &[],
    )
    .await
    .unwrap_or_else(|_| "(No relevant context found)\n\n".to_string());

    // 4. Initialize LLM
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    init_llm(&brain.provider, &brain.model, threads).await?;

    let thinking_frames = [
        format!("{} {}", style("⠋").cyan(), style("Thinking.  ").dim()),
        format!("{} {}", style("⠙").cyan(), style("Thinking.. ").dim()),
        format!("{} {}", style("⠹").cyan(), style("Thinking...").dim()),
        format!("{} {}", style("⠸").cyan(), style("Thinking.  ").dim()),
        format!("{} {}", style("⠼").cyan(), style("Thinking.. ").dim()),
        format!("{} {}", style("⠴").cyan(), style("Thinking...").dim()),
        format!("{} {}", style("⠦").cyan(), style("Thinking.  ").dim()),
        format!("{} {}", style("⠧").cyan(), style("Thinking.. ").dim()),
        format!("{} {}", style("⠇").cyan(), style("Thinking...").dim()),
        format!("{} {}", style("⠏").cyan(), style("Thinking.  ").dim()),
    ];
    let thinking_frames_ref: Vec<&str> = thinking_frames.iter().map(|s| s.as_str()).collect();

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&thinking_frames_ref)
            .template("  ↳ {spinner}")
            .unwrap(),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let user_content = format!("{}\nUser request: {}", context, task);
    let prompt = build_prompt(&user_content, &session_id);
    let params = InferParams {
        model: brain.model.clone(),
        threads,
        max_tokens: brain.max_tokens,
        temperature: brain.temperature as f64,
        repeat_penalty: 1.1,
        max_context: 4096,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: Some(
            uneven_core::infrastructure::security::laws::get_context("askf")
                .replace("{session_id}", &session_id),
        ),
        history: None,
    };

    let start_time = std::time::Instant::now();
    let spinner_clone = spinner.clone();
    let first_token = Arc::new(AtomicBool::new(true));
    let first_token_clone = first_token.clone();

    let res = match infer_with_params_streaming(&prompt, &params, move |_token| {
        if first_token_clone.swap(false, Ordering::Relaxed) {
            let generating_frames = [
                format!(
                    "{} {}",
                    style("⠋").cyan(),
                    style("Generating files.  ").dim()
                ),
                format!(
                    "{} {}",
                    style("⠙").cyan(),
                    style("Generating files.. ").dim()
                ),
                format!(
                    "{} {}",
                    style("⠹").cyan(),
                    style("Generating files...").dim()
                ),
                format!(
                    "{} {}",
                    style("⠸").cyan(),
                    style("Generating files.  ").dim()
                ),
                format!(
                    "{} {}",
                    style("⠼").cyan(),
                    style("Generating files.. ").dim()
                ),
                format!(
                    "{} {}",
                    style("⠴").cyan(),
                    style("Generating files...").dim()
                ),
                format!(
                    "{} {}",
                    style("⠦").cyan(),
                    style("Generating files.  ").dim()
                ),
                format!(
                    "{} {}",
                    style("⠧").cyan(),
                    style("Generating files.. ").dim()
                ),
                format!(
                    "{} {}",
                    style("⠇").cyan(),
                    style("Generating files...").dim()
                ),
                format!(
                    "{} {}",
                    style("⠏").cyan(),
                    style("Generating files.  ").dim()
                ),
            ];
            let generating_frames_ref: Vec<&str> =
                generating_frames.iter().map(|s| s.as_str()).collect();
            spinner_clone.set_style(
                ProgressStyle::default_spinner()
                    .tick_strings(&generating_frames_ref)
                    .template("  ↳ {spinner}")
                    .unwrap(),
            );
        }
    })
    .await
    {
        Ok(r) => {
            spinner.finish_and_clear();
            r
        }
        Err(e) => {
            spinner.finish_and_clear();
            return Err(e.into());
        }
    };
    let elapsed = start_time.elapsed();

    // Safety Response Censorship Check
    let sanitized_answer = safety.sanitize_response(&res.content);
    let censored = sanitized_answer != res.content;
    let answer = sanitized_answer;

    let mut files = parse_file_objects(&answer, &session_id);
    if files.is_empty() {
        files = parse_files_with_id(&answer, &session_id);
    }
    if files.is_empty() {
        files = parse_files(&answer);
    }

    let has_files = !files.is_empty();
    write_parsed_files(&root, files)?;
    if has_files {
        let mut explanation = answer.clone();
        // Strip new object-format file blocks safely using backtick protection
        {
            let marker = "file: {";
            let mut stripped = String::new();
            let mut last_end = 0;
            let mut search_from = 0;
            while let Some(pos) = explanation[search_from..].find(marker) {
                let abs_pos = search_from + pos;
                let brace_start = abs_pos + marker.len() - 1;
                let mut depth = 1;
                let bytes = explanation.as_bytes();
                let mut i = brace_start + 1;
                let mut found_end = None;
                while i < bytes.len() {
                    match bytes[i] {
                        b'`' => {
                            i += 1;
                            while i < bytes.len() && bytes[i] != b'`' {
                                i += 1;
                            }
                        }
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                found_end = Some(i);
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if let Some(end) = found_end {
                    stripped.push_str(&explanation[last_end..abs_pos]);
                    last_end = end + 1;
                    search_from = end + 1;
                } else {
                    search_from = abs_pos + marker.len();
                }
            }
            stripped.push_str(&explanation[last_end..]);
            explanation = stripped;
        }

        // Collapse multi-line empty spacing
        if let Ok(re_newlines) = regex::Regex::new(r"\n{3,}") {
            explanation = re_newlines.replace_all(&explanation, "\n\n").to_string();
        }

        // Strip legacy FILE blocks from the text sequence
        let strip_pattern = format!(
            r#"(?i)FILE_{}[\"\']?\s*[:=]?\s*[\"\']?[<\[\{{\(]?[^\s\]\[}}{{\"\',;<>\(\)]+[\"\']?\s*[>\]}})]?\s*\n\s*```[\s\S]*?```\s*END_(?:FILE_)?{}\n?"#,
            session_id, session_id
        );
        if let Ok(re) = regex::Regex::new(&strip_pattern) {
            explanation = re.replace_all(&explanation, "").to_string();
        }

        if let Ok(re) = regex::Regex::new(
            r#"(?i)FILE[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n```[\s\S]*?```\s*END_FILE\n?"#,
        ) {
            explanation = re.replace_all(&explanation, "").to_string();
        }
        if let Ok(re) = regex::Regex::new(
            r#"(?i)FILE_[A-Z0-9]+[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n\s*```[\s\S]*?```\s*END_(?:FILE_)?[A-Z0-9]+\n?"#,
        ) {
            explanation = re.replace_all(&explanation, "").to_string();
        }

        let explanation = explanation.trim().to_string();

        // 2. Render UI Outputs based on the action result
        if has_files {
            if !explanation.is_empty() {
                println!(
                    "{} {}",
                    truecolor("  › ", 168, 200, 255),
                    truecolor("Summary:", 107, 122, 150)
                );
                for line in explanation.lines() {
                    println!("    {}", truecolor(line, 107, 122, 150));
                }
                println!();
            }
        } else {
            // If no files were altered, render the LLM message description if available
            if !explanation.is_empty() {
                println!(
                    "{} {}",
                    truecolor("  › ", 168, 200, 255),
                    truecolor("Response:", 107, 122, 150)
                );
                for line in explanation.lines() {
                    println!("    {}", truecolor(line, 107, 122, 150));
                }
            } else {
                println!(
                    "{}",
                    truecolor("  › The LLM did not declare any files.", 107, 122, 150)
                );
                println!(
                    "{}",
                    truecolor(
                        "  › Try: \"create a file called index.html with a login form\"",
                        107,
                        122,
                        150
                    )
                );
            }
            println!();
        }

        if censored {
            println!("  {}", crate::ui::rust("⚠ [CENSORSHIP]: Some potentially unsafe contents in the output were blocked for security."));
        }

        let (cost, saved) = uneven_core::infrastructure::llm::pricing::calculate_cost_and_saved(
            &res.model,
            res.prompt_tokens,
            res.completion_tokens,
        );
        println!(
            "{}",
            truecolor(
                &format!(
                    "  [Tokens used: {} | Time: {} | Cost: ${:.5} | Saved: ${:.5}]",
                    res.tokens_used,
                    format_duration(elapsed),
                    cost,
                    saved
                ),
                107,
                122,
                150
            )
        );
        println!();
    } else {
        println!(
            "{}",
            truecolor("  › The LLM did not declare any files.", 107, 122, 150)
        );
        println!(
            "{}",
            truecolor(
                "  › Try: \"create a file called index.html with a login form\"",
                107,
                122,
                150
            )
        );
        println!();

        if censored {
            println!("  {}", crate::ui::rust("⚠ [CENSORSHIP]: Some potentially unsafe contents in the output were blocked for security."));
        }

        let (cost, saved) = uneven_core::infrastructure::llm::pricing::calculate_cost_and_saved(
            &res.model,
            res.prompt_tokens,
            res.completion_tokens,
        );
        println!(
            "{}",
            truecolor(
                &format!(
                    "  [Tokens used: {} | Time: {} | Cost: ${:.5} | Saved: ${:.5}]",
                    res.tokens_used,
                    format_duration(elapsed),
                    cost,
                    saved
                ),
                107,
                122,
                150
            )
        );
        println!();
    }

    Ok(())
}
fn format_duration(d: std::time::Duration) -> String {
    let secs_f = d.as_secs_f64();
    if secs_f < 1.0 {
        let ms = (secs_f * 1000.0).round() as u64;
        format!("{}ms", ms)
    } else if secs_f < 60.0 {
        let secs = secs_f.round() as u64;
        format!("{}s", secs)
    } else {
        let total_secs = secs_f.round() as u64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if secs == 0 {
            format!("{}m", mins)
        } else {
            format!("{}m {}s", mins, secs)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_objects_create() {
        let response = r#"Here is the file:

file: {
session_id: ABC123,
filename: src/index.js,
language: javascript,
content: `console.log('hello');
const x = 42;`
}

Done."#;
        let parsed = parse_file_objects(response, "ABC123");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].filename, "src/index.js");
        assert!(parsed[0].content.contains("console.log('hello');"));
        assert!(!parsed[0].is_patch);
        assert!(!parsed[0].delete);
    }

    #[test]
    fn test_parse_file_objects_patches() {
        let response = r#"file: {
        session_id: XYZ789,
        filename: src/main.rs,
        patches: [
          {
            search: old_code();,
            replace: new_code();
          }
        ]
        }"#;
        let parsed = parse_file_objects(response, "XYZ789");
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].is_patch);

        if let PatchType::SearchReplace { search, replace } = &parsed[0].patches[0] {
            assert_eq!(search, "old_code();");
            assert_eq!(replace, "new_code();");
        } else {
            panic!("Expected PatchType::SearchReplace");
        }
    }

    #[test]
    fn test_parse_file_objects_patch() {
        let response = r#"file: {
        session_id: XYZ789,
        filename: src/main.rs,
        patch: {
        search: old_code();,
        replace: new_code();
        }
        }"#;
        let parsed = parse_file_objects(response, "XYZ789");
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].is_patch);

        if let PatchType::SearchReplace { search, replace } = &parsed[0].patches[0] {
            assert_eq!(search, "old_code();");
            assert_eq!(replace, "new_code();");
        } else {
            panic!("Expected PatchType::SearchReplace");
        }
    }

    #[test]
    fn test_parse_file_objects_insert_before() {
        let response = r#"file: {
        session_id: INS123,
        filename: src/main.rs,
        patches: [
            {
            anchor_before: fn main() { },
            insert_before: // Initialize config
            }
        ]
        }"#;
        let parsed = parse_file_objects(response, "INS123");
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].is_patch);

        if let PatchType::InsertBefore {
            anchor_before,
            insert_before,
        } = &parsed[0].patches[0]
        {
            assert_eq!(anchor_before.trim(), "fn main() { }");
            assert_eq!(insert_before.trim(), "// Initialize config");
        } else {
            panic!("Expected PatchType::InsertBefore");
        }
    }

    #[test]
    fn test_parse_file_objects_insert_after() {
        let response = r#"file: {
        session_id: INS123,
        filename: src/main.rs,
        patches: [
            {
            anchor_after: fn main() { },
            insert_after: println!("Hello!");
            }
        ]
        }"#;
        let parsed = parse_file_objects(response, "INS123");
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].is_patch);

        if let PatchType::InsertAfter {
            anchor_after,
            insert_after,
        } = &parsed[0].patches[0]
        {
            assert_eq!(anchor_after.trim(), "fn main() { }");
            assert_eq!(insert_after.trim(), "println!(\"Hello!\");");
        } else {
            panic!("Expected PatchType::InsertAfter");
        }
    }

    #[test]
    fn test_parse_file_objects_delete() {
        let response = r#"file: {
            session_id: DEL001,
            filename: src/old.rs,
            delete: true
            }"#;
        let parsed = parse_file_objects(response, "DEL001");
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].delete);
    }

    #[test]
    fn test_parse_file_objects_rename() {
        let response = r#"file: {
            session_id: REN001,
            filename: src/old_name.rs,
            rename: src/new_name.rs
            }"#;
        let parsed = parse_file_objects(response, "REN001");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].rename_to.as_deref(), Some("src/new_name.rs"));
    }

    #[test]
    fn test_parse_file_objects_wrong_session() {
        let response = r#"file: {
            session_id: OTHER1,
            filename: src/main.rs,
            delete: true
        }"#;
        let parsed = parse_file_objects(response, "ABC123");
        assert_eq!(parsed.len(), 0);
    }

    #[test]
    fn test_parse_file_objects_braces_inside_backticks() {
        let response = r#"file: {
session_id: `80D5DF`,
filename: `TESTE.js`,
patches: [
{
    anchor_before: `function add(a, b) {`,
    insert_before: `/**
* Adds two numbers.
* @param {number} a
* @param {number} b
* @returns {number}
*/`
},
{
    anchor_before: `function multiply(a, b) {`,
    insert_before: `/**
* Multiplies two numbers.
* @param {number} a
* @param {number} b
* @returns {number}
*/`
}
]
}"#;
        let parsed = parse_file_objects(response, "80D5DF");
        assert_eq!(parsed.len(), 1, "Should parse exactly one file block");
        assert!(parsed[0].is_patch);
        assert_eq!(parsed[0].filename, "TESTE.js");
        assert_eq!(parsed[0].patches.len(), 2, "Should parse two patches");

        if let PatchType::InsertBefore {
            anchor_before,
            insert_before,
        } = &parsed[0].patches[0]
        {
            assert!(anchor_before.contains("function add(a, b) {"));
            assert!(insert_before.contains("Adds two numbers"));
        } else {
            panic!("Expected PatchType::InsertBefore for patch 0");
        }

        if let PatchType::InsertBefore {
            anchor_before,
            insert_before,
        } = &parsed[0].patches[1]
        {
            assert!(anchor_before.contains("function multiply(a, b) {"));
            assert!(insert_before.contains("Multiplies two numbers"));
        } else {
            panic!("Expected PatchType::InsertBefore for patch 1");
        }
    }

    #[test]
    fn test_parse_file_objects_multiple_actions() {
        let response = r#"Some explanation.

file: {
session_id: "ABC123",
filename: 'src/first.js',
language: "javascript",
content: `console.log('first');`
}

And here is the second file:

file: {
session_id: 'ABC123',
filename: "src/second.js",
delete: true,
}
"#;
        let parsed = parse_file_objects(response, "ABC123");
        assert_eq!(parsed.len(), 2);

        assert_eq!(parsed[0].filename, "src/first.js");
        assert!(parsed[0].content.contains("console.log('first');"));
        assert!(!parsed[0].delete);

        assert_eq!(parsed[1].filename, "src/second.js");
        assert!(parsed[1].delete);
    }

    #[test]
    fn test_is_safe_filename() {
        assert!(is_safe_filename("src/app.ts"));
        assert!(is_safe_filename("index.js"));
        assert!(is_safe_filename("Cargo.toml"));

        assert!(!is_safe_filename("/etc/passwd"));
        assert!(!is_safe_filename("../outside.ts"));
        assert!(!is_safe_filename("src/../../outside.ts"));
        assert!(!is_safe_filename(r"src\..\..\outside.ts"));
    }

    #[test]
    fn test_is_safe_path_internal() {
        let root = Path::new("/home/user/project");
        let external = Path::new("/home/user/other");

        assert!(is_safe_path_internal("src/main.rs", root, Some(external)));
        assert!(is_safe_path_internal(
            "/home/user/project/src/main.rs",
            root,
            Some(external)
        ));

        assert!(is_safe_path_internal(
            "../other/src/lib.rs",
            root,
            Some(external)
        ));
        assert!(is_safe_path_internal(
            "/home/user/other/src/lib.rs",
            root,
            Some(external)
        ));

        assert!(!is_safe_path_internal("/etc/passwd", root, Some(external)));
        assert!(!is_safe_path_internal(
            "../outside/src/lib.rs",
            root,
            Some(external)
        ));
        assert!(!is_safe_path_internal(
            "../../etc/passwd",
            root,
            Some(external)
        ));
    }

    #[test]
    fn test_parse_files_with_id() {
        let response = "Some explanation before.\n\n\
                        FILE_SESSION123:\n\
                        src/index.js\n\
                        ```javascript\n\
                        console.log('hello');\n\
                        ```\n\
                        END_FILE_SESSION123\n\n\
                        Some final explanation.";
        let parsed = parse_files_with_id(response, "SESSION123");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].filename, "src/index.js");
        assert_eq!(parsed[0].content.trim(), "console.log('hello');");
    }

    #[test]
    fn test_parse_files_fallback() {
        let response1 = "FILE: index.html\n```html\n<h1>Hello</h1>\n```\nEND_FILE";
        let parsed1 = parse_files(response1);
        assert_eq!(parsed1.len(), 1);
        assert_eq!(parsed1[0].filename, "index.html");
        assert_eq!(parsed1[0].content.trim(), "<h1>Hello</h1>");

        let response2 = "FILE: style.css\nbody { color: red; }\nEND_FILE";
        let parsed2 = parse_files(response2);
        assert_eq!(parsed2.len(), 1);
        assert_eq!(parsed2[0].filename, "style.css");
        assert_eq!(parsed2[0].content.trim(), "body { color: red; }");

        let response3 = "```typescript\n// src/utils.ts\nexport fn add(a: number, b: number) { return a + b; }\n```";
        let parsed3 = parse_files(response3);
        assert_eq!(parsed3.len(), 1);
        assert_eq!(parsed3[0].filename, "src/utils.ts");
        assert_eq!(
            parsed3[0].content.trim(),
            "export fn add(a: number, b: number) { return a + b; }"
        );
    }

    #[test]
    fn test_parse_files_with_patches() {
        let response = "Some text.\n\n\
                        FILE_SESSION123:\n\
                        src/index.js\n\
                        PATCH\n\
                        <<<<<<< SEARCH\n\
                        old_code();\n\
                        =======\n\
                        new_code();\n\
                        >>>>>>> REPLACE\n\
                        END_FILE_SESSION123\n\n\
                        Done.";
        let parsed = parse_files_with_id(response, "SESSION123");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].filename, "src/index.js");
        assert!(parsed[0].is_patch);
        assert_eq!(parsed[0].patches.len(), 1);

        if let PatchType::SearchReplace { search, replace } = &parsed[0].patches[0] {
            assert_eq!(search.trim(), "old_code();");
            assert_eq!(replace.trim(), "new_code();");
        } else {
            panic!("Expected PatchType::SearchReplace");
        }
    }

    #[test]
    fn test_parse_files_delete_and_rename() {
        let delete_response = "FILE_SESSION123:\n\
                               src/old_file.txt\n\
                               DELETE\n\
                               END_FILE_SESSION123";
        let parsed_del = parse_files_with_id(delete_response, "SESSION123");
        assert_eq!(parsed_del.len(), 1);
        assert_eq!(parsed_del[0].filename, "src/old_file.txt");
        assert!(parsed_del[0].delete);

        let rename_response = "FILE_SESSION123:\n\
                               src/old_name.txt\n\
                               RENAME: src/new_name.txt\n\
                               END_FILE_SESSION123";
        let parsed_ren = parse_files_with_id(rename_response, "SESSION123");
        assert_eq!(parsed_ren.len(), 1);
        assert_eq!(parsed_ren[0].filename, "src/old_name.txt");
        assert_eq!(parsed_ren[0].rename_to.as_deref(), Some("src/new_name.txt"));
    }

    #[test]
    fn test_backup_to_obsolete() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();

        let file_path = src_dir.join("main.rs");
        fs::write(&file_path, "fn main() { println!(\"hello\"); }").unwrap();

        backup_to_obsolete(root, &file_path, "src/main.rs");

        let obsolete_src = root.join(".uneven").join("obsolete").join("src");
        assert!(obsolete_src.exists());

        let mut entries = fs::read_dir(obsolete_src).unwrap();
        let entry = entries.next().unwrap().unwrap();
        let path = entry.path();

        let filename = path.file_name().unwrap().to_string_lossy();
        assert!(filename.starts_with("main_"));
        assert!(filename.ends_with(".rs"));

        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(contents, "fn main() { println!(\"hello\"); }");
    }

    #[test]
    fn test_backup_to_obsolete_no_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let file_path = root.join("LICENSE");
        fs::write(&file_path, "MIT License").unwrap();

        backup_to_obsolete(root, &file_path, "LICENSE");

        let obsolete_dir = root.join(".uneven").join("obsolete");
        assert!(obsolete_dir.exists());

        let mut entries = fs::read_dir(obsolete_dir).unwrap();
        let entry = entries.next().unwrap().unwrap();
        let path = entry.path();

        let filename = path.file_name().unwrap().to_string_lossy();
        assert!(filename.starts_with("LICENSE_"));

        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(contents, "MIT License");
    }
}
