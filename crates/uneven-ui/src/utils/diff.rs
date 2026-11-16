use similar::{ChangeTag, TextDiff};
use uneven_core::infrastructure::patch::PatchObj;

#[derive(Clone, Debug, PartialEq)]
pub enum DiffLineType {
    Add,
    Delete,
    Context,
    Header,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub old_num: Option<usize>,
    pub new_num: Option<usize>,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffStats {
    pub additions: usize,
    pub deletions: usize,
}

/// Computes structured diff lines and statistics for a given `PatchObj`.
pub fn compute_patch_diff(patch: &PatchObj) -> (Vec<DiffLine>, DiffStats) {
    let mode = patch.mode.trim().to_lowercase();
    let mut lines = Vec::new();
    let mut stats = DiffStats::default();

    match mode.as_str() {
        "edit" => {
            let diff = TextDiff::from_lines(&patch.search, &patch.content);
            let mut old_idx = 1;
            let mut new_idx = 1;

            for change in diff.iter_all_changes() {
                let text = change.value().trim_end_matches(['\r', '\n']).to_string();
                match change.tag() {
                    ChangeTag::Delete => {
                        stats.deletions += 1;
                        lines.push(DiffLine {
                            line_type: DiffLineType::Delete,
                            old_num: Some(old_idx),
                            new_num: None,
                            text,
                        });
                        old_idx += 1;
                    }
                    ChangeTag::Insert => {
                        stats.additions += 1;
                        lines.push(DiffLine {
                            line_type: DiffLineType::Add,
                            old_num: None,
                            new_num: Some(new_idx),
                            text,
                        });
                        new_idx += 1;
                    }
                    ChangeTag::Equal => {
                        lines.push(DiffLine {
                            line_type: DiffLineType::Context,
                            old_num: Some(old_idx),
                            new_num: Some(new_idx),
                            text,
                        });
                        old_idx += 1;
                        new_idx += 1;
                    }
                }
            }
        }
        "create" => {
            for (idx, line) in patch.content.lines().enumerate() {
                stats.additions += 1;
                lines.push(DiffLine {
                    line_type: DiffLineType::Add,
                    old_num: None,
                    new_num: Some(idx + 1),
                    text: line.to_string(),
                });
            }
        }
        "delete" => {
            lines.push(DiffLine {
                line_type: DiffLineType::Header,
                old_num: None,
                new_num: None,
                text: format!("File scheduled for permanent deletion: {}", patch.path),
            });
        }
        "rename" => {
            lines.push(DiffLine {
                line_type: DiffLineType::Header,
                old_num: None,
                new_num: None,
                text: format!("Rename: {} -> {}", patch.path, patch.rename_to),
            });
        }
        "append" => {
            if !patch.anchor.is_empty() {
                for (idx, line) in patch.anchor.lines().enumerate() {
                    lines.push(DiffLine {
                        line_type: DiffLineType::Context,
                        old_num: Some(idx + 1),
                        new_num: Some(idx + 1),
                        text: line.to_string(),
                    });
                }
            }
            let start_line = patch.anchor.lines().count() + 1;
            for (idx, line) in patch.content.lines().enumerate() {
                stats.additions += 1;
                lines.push(DiffLine {
                    line_type: DiffLineType::Add,
                    old_num: None,
                    new_num: Some(start_line + idx),
                    text: line.to_string(),
                });
            }
        }
        _ => {
            if !patch.search.is_empty() {
                let diff = TextDiff::from_lines(&patch.search, &patch.content);
                for change in diff.iter_all_changes() {
                    let text = change.value().trim_end_matches(['\r', '\n']).to_string();
                    match change.tag() {
                        ChangeTag::Delete => {
                            stats.deletions += 1;
                            lines.push(DiffLine {
                                line_type: DiffLineType::Delete,
                                old_num: None,
                                new_num: None,
                                text,
                            });
                        }
                        ChangeTag::Insert => {
                            stats.additions += 1;
                            lines.push(DiffLine {
                                line_type: DiffLineType::Add,
                                old_num: None,
                                new_num: None,
                                text,
                            });
                        }
                        ChangeTag::Equal => {
                            lines.push(DiffLine {
                                line_type: DiffLineType::Context,
                                old_num: None,
                                new_num: None,
                                text,
                            });
                        }
                    }
                }
            } else {
                for (idx, line) in patch.content.lines().enumerate() {
                    stats.additions += 1;
                    lines.push(DiffLine {
                        line_type: DiffLineType::Add,
                        old_num: None,
                        new_num: Some(idx + 1),
                        text: line.to_string(),
                    });
                }
            }
        }
    }

    (lines, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_create_mode() {
        let patch = PatchObj {
            path: "src/new.rs".to_string(),
            mode: "create".to_string(),
            anchor: String::new(),
            search: String::new(),
            content: "fn main() {\n    println!(\"hello\");\n}".to_string(),
            rename_to: String::new(),
        };

        let (lines, stats) = compute_patch_diff(&patch);
        assert_eq!(stats.additions, 3);
        assert_eq!(stats.deletions, 0);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].line_type, DiffLineType::Add);
        assert_eq!(lines[0].new_num, Some(1));
    }

    #[test]
    fn test_diff_edit_mode() {
        let patch = PatchObj {
            path: "src/main.rs".to_string(),
            mode: "edit".to_string(),
            anchor: String::new(),
            search: "let a = 1;\nlet b = 2;".to_string(),
            content: "let a = 1;\nlet b = 3;\nlet c = 4;".to_string(),
            rename_to: String::new(),
        };

        let (lines, stats) = compute_patch_diff(&patch);
        assert!(stats.deletions >= 1);
        assert!(stats.additions >= 2);
        assert!(lines.iter().any(|l| l.line_type == DiffLineType::Delete));
        assert!(lines.iter().any(|l| l.line_type == DiffLineType::Add));
        assert!(lines.iter().any(|l| l.line_type == DiffLineType::Context));
    }

    #[test]
    fn test_diff_delete_mode() {
        let patch = PatchObj {
            path: "temp.txt".to_string(),
            mode: "delete".to_string(),
            anchor: String::new(),
            search: String::new(),
            content: String::new(),
            rename_to: String::new(),
        };

        let (lines, _) = compute_patch_diff(&patch);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_type, DiffLineType::Header);
    }
}
