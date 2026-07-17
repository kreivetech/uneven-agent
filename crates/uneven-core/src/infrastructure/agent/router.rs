//! Context Router module (The Librarian)
//!
//! Decides which files should be included in the context based on
//! the user's query, project structure, and dependency relationships.

use crate::UnevenError;
use crate::infrastructure::knowledge::scanner::ProjectScanner;
use crate::infrastructure::knowledge::dependency_graph::DependencyGraph;
use crate::infrastructure::session::SESSION_CACHE;
use std::collections::HashMap;
use std::path::Path;

pub struct ContextRouter {
    pub project_root: String,
    pub graph: DependencyGraph,
}

impl ContextRouter {
    pub fn new(project_root: String) -> Self {
        Self {
            project_root,
            graph: DependencyGraph::new(),
        }
    }

    /// Update the internal dependency graph
    pub async fn refresh_graph(&mut self, dirs: Vec<String>) -> Result<(), UnevenError> {
        let scanner = ProjectScanner::new(&self.project_root);
        let files = scanner.scan(dirs)
            .map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        for file_path in files {
            let full_path = Path::new(&self.project_root).join(&file_path);
            if let Ok(content) = std::fs::read_to_string(full_path) {
                self.graph.analyze_file(Path::new(&file_path), &content);
            }
        }

        self.graph.resolve_dependencies();
        Ok(())
    }

    /// Route the query to specific files
    pub fn route(&self, query: &str, limit: usize) -> Vec<String> {
        let mut scores: HashMap<String, f32> = HashMap::new();
        let query_terms: Vec<String> = query.to_lowercase()
            .split_whitespace()
            .map(|s| s.replace(|c: char| !c.is_alphanumeric(), ""))
            .filter(|s| s.len() > 2)
            .collect();

        // 1. Initial scoring based on file path keywords
        for path in self.graph.nodes.keys() {
            let path_lower = path.to_lowercase();
            let mut score = 0.0;
            
            for term in &query_terms {
                if path_lower.contains(term) {
                    score += 1.0;
                }
            }
            
            if score > 0.0 {
                scores.insert(path.clone(), score);
            }
        }

        // 2. Expand score using Dependency Graph (Neighbors get points)
        let mut expansion = HashMap::new();
        for (path, score) in &scores {
            let neighbors = self.graph.get_related_files(path);
            for neighbor in neighbors {
                let entry = expansion.entry(neighbor).or_insert(0.0);
                *entry += score * 0.5; // Neighbors get half the score
            }
        }

        for (path, score) in expansion {
            let entry = scores.entry(path).or_insert(0.0);
            *entry += score;
        }

        // 3. Boost recently accessed files from cache
        for path in self.graph.nodes.keys() {
            if SESSION_CACHE.get_file(path).is_some() {
                let entry = scores.entry(path.clone()).or_insert(0.0);
                *entry += 0.5;
            }
        }

        // Sort and limit
        let mut results: Vec<(String, f32)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        results.into_iter()
            .take(limit)
            .map(|(path, _)| path)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_routing_logic() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap().to_string();
        
        // Setup mock project
        fs::write(tmp.path().join("auth.ts"), "import { db } from './db';").unwrap();
        fs::write(tmp.path().join("db.ts"), "export const db = {}").unwrap();
        fs::write(tmp.path().join("ui.ts"), "export const ui = {}").unwrap();

        let mut router = ContextRouter::new(root);
        router.refresh_graph(vec![".".into()]).await.unwrap();

        // Query about "auth" should find auth.ts and its neighbor db.ts
        let results = router.route("how does auth work?", 5);
        assert!(results.contains(&"auth.ts".to_string()));
        assert!(results.contains(&"db.ts".to_string()));
    }
}
