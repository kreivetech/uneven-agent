use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".uneven",
    "node_modules",
    "target",
    "dist",
    "build",
    "venv",
    ".venv",
];

/// A deterministic 2-level directory group inside the workspace files map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileGroup {
    pub id: usize,
    /// `"."` for root files, or `"<dir>/*"` / `"<dir>/<subdir>/*"` (up to 2 levels from root).
    pub prefix: String,
    pub files: Vec<String>,
}

/// Compact, grouped workspace files index with surgical per-group refresh support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesMapIndex {
    pub root: PathBuf,
    pub groups: Vec<FileGroup>,
}

/// Computes the 2-level group key for a relative file path (e.g. `"."`, `"src/*"`, `"src/services/*"`).
pub fn group_prefix_for(rel_path: &str) -> String {
    let normalized = rel_path.replace('\\', "/");
    let parts: Vec<&str> = normalized
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();

    match parts.len() {
        0 | 1 => ".".to_string(),
        2 => format!("{}/*", parts[0]),
        _ => format!("{}/{}/*", parts[0], parts[1]),
    }
}

fn is_ignored_dir_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    IGNORED_DIRS.iter().any(|&d| d == lower)
}

fn should_skip_rel(rel: &Path) -> bool {
    rel.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(is_ignored_dir_name)
            .unwrap_or(false)
    })
}

fn collect_files_in(scan_root: &Path, base_root: &Path, max_depth: Option<usize>, max_files: Option<usize>) -> Vec<String> {
    let mut builder = WalkBuilder::new(scan_root);
    builder
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            if entry.file_type().is_some_and(|ft| ft.is_dir())
                && let Some(name) = entry.file_name().to_str()
            {
                return !is_ignored_dir_name(name);
            }
            true
        });

    if let Some(d) = max_depth {
        builder.max_depth(Some(d));
    }

    let mut files = Vec::new();
    for entry in builder.build().filter_map(|e| e.ok()) {
        if entry.file_type().is_some_and(|ft| ft.is_file())
            && let Ok(rel) = entry.path().strip_prefix(base_root)
        {
            if should_skip_rel(rel) {
                continue;
            }
            files.push(rel.to_string_lossy().replace('\\', "/"));
            if let Some(limit) = max_files
                && files.len() >= limit
            {
                break;
            }
        }
    }

    // Ensure existence-only visibility for `.gitignore` and `.env*` at `scan_root`
    // even if `.gitignore` lists `.env*` itself (content is never read here).
    if let Ok(entries) = std::fs::read_dir(scan_root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file()
                && let Some(name) = p.file_name().and_then(|n| n.to_str())
                && (name == ".gitignore" || name == ".env" || name.starts_with(".env."))
                && let Ok(rel) = p.strip_prefix(base_root)
            {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if !files.contains(&rel_str) {
                    files.push(rel_str);
                }
            }
        }
    }

    files.sort();
    files.dedup();
    files
}

impl FilesMapIndex {
    /// Builds a grouped `FilesMapIndex` across `root` using `ignore::WalkBuilder`,
    /// preserving `.gitignore` and `.env*` file visibility.
    pub fn build(root: &Path, max_files: Option<usize>) -> Self {
        let all_files = collect_files_in(root, root, None, max_files);
        let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for rel in all_files {
            let prefix = group_prefix_for(&rel);
            grouped.entry(prefix).or_default().push(rel);
        }

        let mut groups = Vec::with_capacity(grouped.len());
        if let Some(root_files) = grouped.remove(".") {
            groups.push(FileGroup {
                id: 0,
                prefix: ".".to_string(),
                files: root_files,
            });
        }

        for (prefix, files) in grouped {
            let id = groups.len();
            groups.push(FileGroup { id, prefix, files });
        }

        Self {
            root: root.to_path_buf(),
            groups,
        }
    }

    /// Renders the compact group index (`[id] prefix (N files)`) for the AI `MAP` block.
    pub fn render_compact_index(&self) -> String {
        self.groups
            .iter()
            .map(|g| format!("[{}] {} ({} files)", g.id, g.prefix, g.files.len()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Renders a paginated list of files belonging to group `id` (1-based `page`).
    pub fn render_group(&self, id: usize, page: usize, page_size: usize) -> Option<String> {
        let group = self.groups.iter().find(|g| g.id == id)?;
        let size = page_size.max(1);
        let total_pages = group.files.len().div_ceil(size).max(1);
        let current_page = page.max(1).min(total_pages);
        let start = (current_page - 1) * size;
        let end = (start + size).min(group.files.len());

        let mut out = format!(
            "[{}] {} (page {}/{}, {} files):\n",
            group.id,
            group.prefix,
            current_page,
            total_pages,
            group.files.len()
        );
        for file in &group.files[start..end] {
            out.push_str(file);
            out.push('\n');
        }
        Some(out.trim_end().to_string())
    }

    /// Surgically re-scans only the 2-level group containing `rel_path` (e.g. after `patchcraft` applies a change).
    pub fn refresh_path(&mut self, rel_path: &str) {
        let target_prefix = group_prefix_for(rel_path);
        let refreshed_files = if target_prefix == "." {
            collect_files_in(&self.root, &self.root, Some(1), None)
        } else {
            let dir_rel = target_prefix.trim_end_matches("/*");
            let target_dir = self.root.join(dir_rel);
            if target_dir.is_dir() {
                collect_files_in(&target_dir, &self.root, None, None)
            } else {
                Vec::new()
            }
        };

        if let Some(existing) = self.groups.iter_mut().find(|g| g.prefix == target_prefix) {
            existing.files = refreshed_files;
        } else if !refreshed_files.is_empty() {
            let next_id = self.groups.len();
            self.groups.push(FileGroup {
                id: next_id,
                prefix: target_prefix,
                files: refreshed_files,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_two_level_grouping_and_keeps_gitignore_and_env() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        fs::write(root.join(".gitignore"), "target/\n.env\n").unwrap();
        fs::write(root.join(".env"), "SECRET=123").unwrap();
        fs::write(root.join("Cargo.toml"), "[package]").unwrap();

        fs::create_dir_all(root.join("src/services/auth")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/services/user.rs"), "").unwrap();
        fs::write(root.join("src/services/auth/jwt.rs"), "").unwrap();

        // Ignored build folder should not appear
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/bin"), "").unwrap();

        let index = FilesMapIndex::build(root, None);
        let compact = index.render_compact_index();

        assert!(compact.contains("[0] . (3 files)"));
        assert!(compact.contains("src/* (1 files)"));
        assert!(compact.contains("src/services/* (2 files)"));
        assert!(!compact.contains("target"));

        let root_page = index.render_group(0, 1, 10).unwrap();
        assert!(root_page.contains(".gitignore"));
        assert!(root_page.contains(".env"));
        assert!(root_page.contains("Cargo.toml"));
    }

    #[test]
    fn test_refresh_path_updates_only_target_group() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        fs::create_dir_all(root.join("src/services")).unwrap();
        fs::write(root.join("src/services/a.rs"), "").unwrap();

        let mut index = FilesMapIndex::build(root, None);
        let svc_id = index
            .groups
            .iter()
            .find(|g| g.prefix == "src/services/*")
            .unwrap()
            .id;
        assert_eq!(index.groups[svc_id].files, vec!["src/services/a.rs"]);

        // Simulate patchcraft creating a new file in src/services/
        fs::write(root.join("src/services/b.rs"), "").unwrap();
        index.refresh_path("src/services/b.rs");

        assert_eq!(
            index.groups[svc_id].files,
            vec!["src/services/a.rs", "src/services/b.rs"]
        );
    }

    #[test]
    fn test_render_group_pagination() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("src/items")).unwrap();
        for i in 0..5 {
            fs::write(root.join(format!("src/items/f{}.rs", i)), "").unwrap();
        }

        let index = FilesMapIndex::build(root, None);
        let gid = index
            .groups
            .iter()
            .find(|g| g.prefix == "src/items/*")
            .unwrap()
            .id;

        let p1 = index.render_group(gid, 1, 2).unwrap();
        assert!(p1.contains("page 1/3, 5 files"));
        assert!(p1.contains("src/items/f0.rs"));
        assert!(p1.contains("src/items/f1.rs"));
        assert!(!p1.contains("src/items/f2.rs"));

        let p3 = index.render_group(gid, 3, 2).unwrap();
        assert!(p3.contains("page 3/3, 5 files"));
        assert!(p3.contains("src/items/f4.rs"));
    }
}
