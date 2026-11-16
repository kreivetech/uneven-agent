use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use crate::domain::error::UnevenError;

pub struct ProjectScanner {
    root: PathBuf,
}

impl ProjectScanner {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// Scans the provided directories (relative to root) and returns a list of relative paths.
    pub fn scan(&self, dirs: Vec<String>) -> Result<Vec<String>, UnevenError> {
        if dirs.is_empty() {
            return Ok(Vec::new());
        }

        let normalized_dirs: Vec<PathBuf> = dirs
            .iter()
            .map(|d| {
                let mut s = d.as_str();
                if s.starts_with("./") || s.starts_with(".\\") {
                    s = &s[2..];
                }
                PathBuf::from(s)
            })
            .collect();

        let mut files = Vec::new();
        let walker = WalkBuilder::new(&self.root)
            .hidden(false)
            .git_ignore(false)
            .filter_entry(|entry| {
                if entry.depth() == 0 {
                    return true;
                }
                if let Some(name) = entry.file_name().to_str() {
                    let s_lower = name.to_lowercase();
                    s_lower != ".git"
                        && s_lower != ".uneven"
                        && s_lower != "node_modules"
                        && s_lower != "target"
                        && s_lower != "dist"
                        && s_lower != "build"
                        && s_lower != "venv"
                        && s_lower != ".venv"
                } else {
                    true
                }
            })
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if entry.file_type().is_some_and(|ft| ft.is_file())
                        && let Ok(rel) = entry.path().strip_prefix(&self.root) {
                            // Filter out ignored folders (like node_modules, .git, .uneven, target, dist, build, venv, .venv)
                            let has_ignored_component = rel.components().any(|c| {
                                if let Some(s) = c.as_os_str().to_str() {
                                    let s_lower = s.to_lowercase();
                                    s_lower == ".git"
                                        || s_lower == "node_modules"
                                        || s_lower == ".uneven"
                                        || s_lower == "target"
                                        || s_lower == "dist"
                                        || s_lower == "build"
                                        || s_lower == "venv"
                                        || s_lower == ".venv"
                                } else {
                                    false
                                }
                            });

                            if has_ignored_component {
                                continue;
                            }

                            // Filter out sensitive/unnecessary files (like .env, .gitignore, lockfiles)
                            if let Some(file_name) = rel.file_name().and_then(|f| f.to_str()) {
                                let f_lower = file_name.to_lowercase();
                                if f_lower == ".env"
                                    || f_lower.starts_with(".env.")
                                    || f_lower == ".gitignore"
                                    || f_lower == "package-lock.json"
                                    || f_lower == "yarn.lock"
                                    || f_lower == "pnpm-lock.yaml"
                                    || f_lower == "cargo.lock"
                                {
                                    continue;
                                }
                            }

                            let rel_str = rel.to_string_lossy().to_string();
                            
                            // Check if this file is within one of the requested dirs
                            if normalized_dirs.iter().any(|d| {
                                d.to_str() == Some("") || d.to_str() == Some(".") || rel.starts_with(d)
                            }) {
                                files.push(rel_str);
                            }
                        }
                }
                Err(_e) => {
                    // Silent fail for individual file scan warnings to avoid information leakage.
                    // The overall scan will still return a Result.
                }
            }
        }

        files.sort();
        files.dedup();
        Ok(files)
    }

    /// Generates a compact string representation of the project structure for LLM context.
    pub fn generate_map(&self, dirs: Vec<String>, max_files: usize) -> Result<String, UnevenError> {
        let files = self.scan(dirs)?;
        if files.is_empty() {
            return Ok("(No files found in project root)".to_string());
        }

        let total = files.len();
        let list = if total > max_files {
            let mut s = files[..max_files].join("\n");
            s.push_str(&format!("\n... (and {} more files)", total - max_files));
            s
        } else {
            files.join("\n")
        };

        Ok(list)
    }
}

pub fn get_directory_map(root: &str, dirs: Vec<String>, max_files: usize) -> Result<String, UnevenError> {
    let scanner = ProjectScanner::new(root);
    scanner.generate_map(dirs, max_files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_scanner_basic() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("README.md"), "# Test").unwrap();
        
        let scanner = ProjectScanner::new(root);
        let files = scanner.scan(vec!["src".to_string(), "README.md".to_string()]).unwrap();
        
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"src/main.rs".to_string()));
        assert!(files.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_scanner_custom_ignores() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::write(root.join("node_modules/index.js"), "console.log(1)").unwrap();
        fs::write(root.join("app.log"), "error").unwrap();
        fs::write(root.join("app.rs"), "fn main() {}").unwrap();
        fs::write(root.join(".env"), "SECRET=123").unwrap();
        
        let scanner = ProjectScanner::new(root);
        let files = scanner.scan(vec![".".to_string()]).unwrap();
        
        assert!(files.contains(&"app.rs".to_string()));
        assert!(files.contains(&"app.log".to_string()));
        assert!(!files.contains(&"node_modules/index.js".to_string()));
        assert!(!files.contains(&".env".to_string()));
    }

    #[test]
    fn test_generate_map_truncation() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        for i in 1..=5 {
            fs::write(root.join(format!("file{}.txt", i)), "data").unwrap();
        }
        
        let scanner = ProjectScanner::new(root);
        let map = scanner.generate_map(vec![".".to_string()], 2).unwrap();
        
        assert!(map.contains("file1.txt"));
        assert!(map.contains("file2.txt"));
        assert!(map.contains("... (and 3 more files)"));
    }

    #[test]
    fn test_scanner_with_dot_slash() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        fs::create_dir(root.join("app")).unwrap();
        fs::write(root.join("app/main.ts"), "const a = 1;").unwrap();
        
        let scanner = ProjectScanner::new(root);
        let files = scanner.scan(vec!["./app".to_string()]).unwrap();
        
        assert_eq!(files.len(), 1);
        assert!(files.contains(&"app/main.ts".to_string()));
    }

    #[test]
    fn test_scanner_avoid_partial_matching() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        fs::create_dir(root.join("app")).unwrap();
        fs::write(root.join("app/main.ts"), "const a = 1;").unwrap();
        
        fs::create_dir(root.join("apple")).unwrap();
        fs::write(root.join("apple/main.ts"), "const a = 2;").unwrap();
        
        let scanner = ProjectScanner::new(root);
        let files = scanner.scan(vec!["app".to_string()]).unwrap();
        
        assert_eq!(files.len(), 1);
        assert!(files.contains(&"app/main.ts".to_string()));
        assert!(!files.contains(&"apple/main.ts".to_string()));
    }

    #[test]
    fn test_scanner_default_ignored_paths() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        fs::write(root.join("app.rs"), "fn main() {}").unwrap();
        fs::write(root.join(".env"), "SECRET=key").unwrap();
        fs::write(root.join(".env.local"), "SECRET=local").unwrap();
        fs::write(root.join("package-lock.json"), "{}").unwrap();
        
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), "some git config").unwrap();
        
        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join("target/debug"), "binary build").unwrap();
        
        let scanner = ProjectScanner::new(root);
        let files = scanner.scan(vec![".".to_string()]).unwrap();
        
        assert!(files.contains(&"app.rs".to_string()));
        assert!(!files.contains(&".env".to_string()));
        assert!(!files.contains(&".env.local".to_string()));
        assert!(!files.contains(&"package-lock.json".to_string()));
        assert!(!files.contains(&".git/config".to_string()));
        assert!(!files.contains(&"target/debug".to_string()));
    }
}
