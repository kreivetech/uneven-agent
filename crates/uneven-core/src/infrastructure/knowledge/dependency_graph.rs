//! Dependency graph analysis module
//!
//! Parses source files to extract import/export relationships and
//! builds a directed graph of project dependencies.


use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use regex::Regex;
use once_cell::sync::Lazy;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DependencyNode {
    pub path: String,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
}

#[derive(Default)]
pub struct DependencyGraph {
    pub nodes: HashMap<String, DependencyNode>,
}

static RE_TS_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:import|export)\s+.*?\s+from\s+['"](.*?)['"]"#).unwrap()
});

static RE_JS_REQUIRE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)require\s*\(\s*['"](.*?)['"]\s*\)"#).unwrap()
});

static RE_RS_USE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^use\s+crate::(.*?);"#).unwrap()
});


impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze a file and extract its dependencies
    pub fn analyze_file(&mut self, file_path: &Path, content: &str) {
        let path_str = file_path.to_string_lossy().to_string();
        let mut imports = Vec::new();

        // Extract TS/JS imports
        for cap in RE_TS_IMPORT.captures_iter(content) {
            imports.push(cap[1].to_string());
        }

        // Extract JS requirements
        for cap in RE_JS_REQUIRE.captures_iter(content) {
            imports.push(cap[1].to_string());
        }

        // Extract Rust use statements (crate-internal only for now)
        for cap in RE_RS_USE.captures_iter(content) {
            imports.push(cap[1].to_string());
        }

        self.nodes.insert(path_str.clone(), DependencyNode {
            path: path_str,
            imports,
            exports: Vec::new(), // TODO: Implement export analysis
        });
    }

    /// Resolve relative imports to absolute paths
    pub fn resolve_dependencies(&mut self) {
        let mut resolved_nodes = HashMap::new();

        for (path, node) in &self.nodes {
            let base_dir = Path::new(path).parent().unwrap_or_else(|| Path::new(""));
            let mut resolved_imports = Vec::new();

            for imp in &node.imports {
                if imp.starts_with('.') {
                    // Resolve relative path
                    let full_path = base_dir.join(imp);
                    
                    // Try common extensions
                    let mut resolved = false;
                    for ext in &["ts", "tsx", "js", "jsx", "rs"] {
                        let path_with_ext = full_path.with_extension(ext);
                        // Normalize: remove redundant ./ and resolve components
                        let normalized = path_with_ext.components()
                            .filter(|c| matches!(c, std::path::Component::Normal(_)) || matches!(c, std::path::Component::ParentDir))
                            .collect::<PathBuf>();
                        let normalized_str = normalized.to_string_lossy().to_string();

                        if self.nodes.contains_key(&normalized_str) {
                            resolved_imports.push(normalized_str);
                            resolved = true;
                            break;
                        }
                    }
                    if !resolved {
                        resolved_imports.push(imp.clone());
                    }
                } else {
                    resolved_imports.push(imp.clone());
                }
            }

            resolved_nodes.insert(path.clone(), DependencyNode {
                path: path.clone(),
                imports: resolved_imports,
                exports: node.exports.clone(),
            });
        }

        self.nodes = resolved_nodes;
    }

    /// Get related files (neighbors in the graph)
    pub fn get_related_files(&self, file_path: &str) -> HashSet<String> {
        let mut related = HashSet::new();
        if let Some(node) = self.nodes.get(file_path) {
            for imp in &node.imports {
                related.insert(imp.clone());
            }
        }
        
        // Also find files that import THIS file
        for (path, node) in &self.nodes {
            if node.imports.iter().any(|i| i == file_path) {
                related.insert(path.clone());
            }
        }

        related
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ts_imports() {
        let mut graph = DependencyGraph::new();
        let content = r#"
            import { foo } from './foo';
            export { bar } from '../bar';
            const x = require('./baz');
        "#;
        graph.analyze_file(Path::new("src/main.ts"), content);
        
        let node = graph.nodes.get("src/main.ts").unwrap();
        assert!(node.imports.contains(&"./foo".to_string()));
        assert!(node.imports.contains(&"../bar".to_string()));
        assert!(node.imports.contains(&"./baz".to_string()));
    }

    #[test]
    fn test_resolve_relative_paths() {
        let mut graph = DependencyGraph::new();
        graph.nodes.insert("src/foo.ts".to_string(), DependencyNode::default());
        
        let content = "import { foo } from './foo';";
        graph.analyze_file(Path::new("src/main.ts"), content);
        
        graph.resolve_dependencies();
        
        let node = graph.nodes.get("src/main.ts").unwrap();
        assert!(node.imports.contains(&"src/foo.ts".to_string()));
    }
}
