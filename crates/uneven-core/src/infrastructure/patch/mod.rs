use crate::infrastructure::config::loader;
use std::path::Path;

// Re-export core patchcraft models, parsers, options and helpers
pub use patchcraft::{
    clean_chat_proposal, clean_code_fence, convert_search_replace_to_unified_diff,
    extract_file_path_from_header, extract_rename_target, file_object, fuzzy_replace_search,
    is_safe_path, normalize_path, parse_anchor_block, parse_markdown_patches,
    parse_search_replace_block, FileObject, PatchActionKind, PatchActionResult, PatchObj,
    PatchOptions, ResponsePayload,
};

/// Uneven security boundary validator checking against the project root and configured external project.
pub fn is_safe_path_internal(name: &str, root: &Path, external_dir: Option<&Path>) -> bool {
    let allowed_vec: Vec<std::path::PathBuf> = match external_dir {
        Some(ext) => vec![ext.to_path_buf()],
        None => Vec::new(),
    };
    patchcraft::is_safe_path(name, root, &allowed_vec)
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

    let allowed: &[std::path::PathBuf] = match ext_project.as_ref() {
        Some(ext) => std::slice::from_ref(ext),
        None => &[],
    };

    patchcraft::is_safe_path(&clean_name, &root, allowed)
}

/// Backs up a file to the `.uneven/obsolete` directory with a timestamp.
pub fn backup_to_obsolete(root: &Path, file_path: &Path, relative_filename: &str) {
    let obsolete_dir = root.join(".uneven").join("obsolete");
    patchcraft::backup_file(root, file_path, relative_filename, Some(&obsolete_dir));
}

/// Applies all file actions to disk atomically with automatic backup to `.uneven/obsolete`.
pub fn apply_file_actions(root: &Path, file_obj: &FileObject) -> Vec<PatchActionResult> {
    let config = loader::load_config(root).ok();
    let mut options = PatchOptions::new(root)
        .with_backup_dir(root.join(".uneven").join("obsolete"));

    if let Some(ext) = config
        .as_ref()
        .and_then(|c| c.knowledge.as_ref())
        .and_then(|k| k.external_project.as_ref())
    {
        options = options.with_allowed_external_dir(ext);
    }

    patchcraft::apply_patches(&options, file_obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_clean_code_fence() {
        assert_eq!(clean_code_fence("```rust\nfn main() {}\n```"), "fn main() {}");
        assert_eq!(clean_code_fence("`foo.rs`"), "foo.rs");
        assert_eq!(clean_code_fence("plain text"), "plain text");
    }

    #[test]
    fn test_parse_markdown_patches_create_and_edit() {
        let text = r#"
### Summary: Added user model and updated routes

### File: src/models/user.rs
**Action**: `create`

```rust
pub struct User {
    pub id: u64,
}
```

### File: src/routes.rs
**Action**: `edit`

```rust
<<<<<<< SEARCH
// routes
=======
// new routes
>>>>>>> REPLACE
```
"#;
        let file_obj = parse_markdown_patches(text, "test_sess").expect("Should parse patches");
        assert_eq!(file_obj.patch.len(), 2);
        assert_eq!(file_obj.patch[0].path, "src/models/user.rs");
        assert_eq!(file_obj.patch[0].mode, "create");
        assert_eq!(file_obj.patch[1].path, "src/routes.rs");
        assert_eq!(file_obj.patch[1].mode, "edit");
        assert_eq!(file_obj.patch[1].search, "// routes");
        assert_eq!(file_obj.patch[1].content, "// new routes");
    }

    #[test]
    fn test_apply_file_actions_create_edit_delete() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let file_obj = FileObject {
            session_id: "test".to_string(),
            summary: Some("test actions".to_string()),
            patch: vec![
                PatchObj {
                    path: "hello.txt".to_string(),
                    mode: "create".to_string(),
                    anchor: String::new(),
                    search: String::new(),
                    content: "hello world".to_string(),
                    rename_to: String::new(),
                },
            ],
        };

        let results = apply_file_actions(root, &file_obj);
        assert_eq!(results.len(), 1);
        assert_eq!(std::fs::read_to_string(root.join("hello.txt")).unwrap(), "hello world");

        // Edit
        let edit_obj = FileObject {
            session_id: "test".to_string(),
            summary: None,
            patch: vec![
                PatchObj {
                    path: "hello.txt".to_string(),
                    mode: "edit".to_string(),
                    anchor: String::new(),
                    search: "world".to_string(),
                    content: "rust".to_string(),
                    rename_to: String::new(),
                },
            ],
        };
        let edit_results = apply_file_actions(root, &edit_obj);
        assert_eq!(edit_results.len(), 1);
        assert_eq!(std::fs::read_to_string(root.join("hello.txt")).unwrap(), "hello rust");

        // Delete
        let delete_obj = FileObject {
            session_id: "test".to_string(),
            summary: None,
            patch: vec![
                PatchObj {
                    path: "hello.txt".to_string(),
                    mode: "delete".to_string(),
                    anchor: String::new(),
                    search: String::new(),
                    content: String::new(),
                    rename_to: String::new(),
                },
            ],
        };
        let del_results = apply_file_actions(root, &delete_obj);
        assert_eq!(del_results.len(), 1);
        assert!(!root.join("hello.txt").exists());
    }

    #[test]
    fn test_fuzzy_replace_search_indentation_variance() {
        let original = "fn calculate() {\n    let a = 10;\n    let b = 20;\n    a + b\n}";
        let search = "  let a = 10;\n  let b = 20;";
        let replacement = "    let a = 100;\n    let b = 200;";

        let result = fuzzy_replace_search(original, search, replacement).expect("Should match fuzzy");
        assert!(result.contains("let a = 100;"));
        assert!(result.contains("let b = 200;"));
        assert!(!result.contains("let a = 10;"));
    }

    #[test]
    fn test_apply_file_actions_self_healing_missing_edit() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let edit_missing = FileObject {
            session_id: "test".to_string(),
            summary: None,
            patch: vec![PatchObj {
                path: "brand_new.rs".to_string(),
                mode: "edit".to_string(),
                anchor: String::new(),
                search: "some search".to_string(),
                content: "pub fn hello() {}".to_string(),
                rename_to: String::new(),
            }],
        };

        let results = apply_file_actions(root, &edit_missing);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].kind, PatchActionKind::Created(_)));
        assert_eq!(
            std::fs::read_to_string(root.join("brand_new.rs")).unwrap(),
            "pub fn hello() {}"
        );
    }
}
