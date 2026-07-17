use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;

/// Traverses upwards from start_path to find the closest sub-project root containing package indicators.
pub fn find_project_root(start_path: &str) -> PathBuf {
    let mut dir = match PathBuf::from(start_path).canonicalize() {
        Ok(p) => {
            if p.is_file() {
                p.parent().map(|parent| parent.to_path_buf()).unwrap_or(p)
            } else {
                p
            }
        }
        Err(_) => {
            let p = PathBuf::from(start_path);
            if p.is_file() {
                p.parent().map(|parent| parent.to_path_buf()).unwrap_or(p)
            } else {
                p
            }
        }
    };

    loop {
        if dir.join("package.json").exists()
            || dir.join("requirements.txt").exists()
            || dir.join("pyproject.toml").exists()
            || dir.join("Cargo.toml").exists()
            || dir.join("go.mod").exists()
        {
            return dir;
        }
        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    // Default fallback to current working directory
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Checks if an NPM package exists in the dependencies/devDependencies of the closest package.json.
pub fn has_npm_dependency(project_root: &Path, dep_name: &str) -> bool {
    let pkg_path = project_root.join("package.json");
    if !pkg_path.exists() {
        return false;
    }

    let content = match fs::read_to_string(&pkg_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return false,
    };

    if let Some(deps) = json.get("dependencies").and_then(|d| d.as_object()) {
        if deps.contains_key(dep_name) {
            return true;
        }
    }

    if let Some(dev_deps) = json.get("devDependencies").and_then(|d| d.as_object()) {
        if dev_deps.contains_key(dep_name) {
            return true;
        }
    }

    false
}

/// Checks if a Python dependency exists in the closest requirements.txt or pyproject.toml.
pub fn has_python_dependency(project_root: &Path, dep_name: &str) -> bool {
    let req_path = project_root.join("requirements.txt");
    if req_path.exists() {
        if let Ok(content) = fs::read_to_string(&req_path) {
            let pattern = format!(r"(?:^|\s){}(?:\s|>=|<=|==|$)", regex::escape(dep_name));
            if let Ok(re) = Regex::new(&pattern) {
                if re.is_match(&content) {
                    return true;
                }
            }
        }
    }

    let pyproj_path = project_root.join("pyproject.toml");
    if pyproj_path.exists() {
        if let Ok(content) = fs::read_to_string(&pyproj_path) {
            if content.to_lowercase().contains(&dep_name.to_lowercase()) {
                return true;
            }
        }
    }

    false
}

/// Checks if a Rust dependency exists in the closest Cargo.toml.
pub fn has_cargo_dependency(project_root: &Path, dep_name: &str) -> bool {
    let cargo_path = project_root.join("Cargo.toml");
    if !cargo_path.exists() {
        return false;
    }

    if let Ok(content) = fs::read_to_string(&cargo_path) {
        let pattern = format!(r"(?:^|\s){}\s*=", regex::escape(dep_name));
        if let Ok(re) = Regex::new(&pattern) {
            return re.is_match(&content);
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_find_project_root() {
        let dir = tempdir().unwrap();
        let project_path = dir.path().join("my-project");
        fs::create_dir_all(&project_path).unwrap();
        
        let cargo_toml = project_path.join("Cargo.toml");
        File::create(&cargo_toml).unwrap();
        
        let nested_src = project_path.join("src").join("utils");
        fs::create_dir_all(&nested_src).unwrap();
        
        let root = find_project_root(&nested_src.to_string_lossy());
        assert_eq!(root.canonicalize().unwrap(), project_path.canonicalize().unwrap());
    }

    #[test]
    fn test_has_npm_dependency() {
        let dir = tempdir().unwrap();
        let pkg_path = dir.path().join("package.json");
        let mut file = File::create(&pkg_path).unwrap();
        file.write_all(br#"
        {
            "dependencies": {
                "express": "^4.18.2"
            },
            "devDependencies": {
                "typescript": "^5.0.0"
            }
        }
        "#).unwrap();

        assert!(has_npm_dependency(dir.path(), "express"));
        assert!(has_npm_dependency(dir.path(), "typescript"));
        assert!(!has_npm_dependency(dir.path(), "lodash"));
    }

    #[test]
    fn test_has_cargo_dependency() {
        let dir = tempdir().unwrap();
        let cargo_path = dir.path().join("Cargo.toml");
        let mut file = File::create(&cargo_path).unwrap();
        file.write_all(br#"
        [dependencies]
        tokio = { version = "1.0", features = ["full"] }
        serde = "1.0"
        "#).unwrap();

        assert!(has_cargo_dependency(dir.path(), "tokio"));
        assert!(has_cargo_dependency(dir.path(), "serde"));
        assert!(!has_cargo_dependency(dir.path(), "regex"));
    }
}
