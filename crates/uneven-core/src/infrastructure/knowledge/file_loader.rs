//! File loader module
//!
//! Loads and parses various file types:
//! - Source code: Rust, TypeScript, JavaScript, Python, etc.
//! - Documentation: Markdown, plain text
//! - Structured: YAML, JSON, TOML

use crate::UnevenError;
use std::collections::HashSet;
use std::path::Path;

/// Extensions supported by the file loader.
///
/// Stored as a static HashSet so the set is built once and extension
/// lookups are O(1) instead of O(n) Vec::contains on every call.
static SUPPORTED_EXTENSIONS: std::sync::OnceLock<HashSet<&'static str>> =
    std::sync::OnceLock::new();

fn supported_extensions() -> &'static HashSet<&'static str> {
    SUPPORTED_EXTENSIONS.get_or_init(|| {
        [
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "java",
            "cpp", "c", "h", "hpp", "cc", "cxx", "md", "txt", "json", "yaml", "yml", "toml",
            "rb", "php", "cs", "dart", "swift", "kt", "kts", "scala", "sh", "bash", "zsh",
            "vue", "svelte", "astro", "css", "scss", "less", "html", "htm", "sql", "graphql", "gql",
            "sol", "zig", "lua", "rb", "pl", "pm", "ex", "exs", "erl", "hs", "jl", "r", "f90", "f95",
            "dockerfile", "tf", "tfvars", "hcl", "proto", "thrift", "xml", "csv", "log",
        ]
        .into_iter()
        .collect()
    })
}

pub struct FileLoader;

impl FileLoader {
    pub fn new() -> Self {
        FileLoader
    }

    pub async fn load_file(&self, path: &Path) -> Result<String, UnevenError> {
        tokio::fs::read_to_string(path)
            .await
            .map_err(|e| UnevenError::KnowledgeError(format!("Failed to read file: {}", e)))
    }

    pub fn is_supported(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| supported_extensions().contains(ext))
            .unwrap_or(false)
    }
}

impl Default for FileLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Load file content by path (public API for napi).
///
/// Reads file from disk and returns the entire content as a UTF-8 string.
pub async fn load_file_content(path: &str) -> Result<String, UnevenError> {
    tracing::debug!("Loading file content from: {}", path);

    if Path::new(path)
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(UnevenError::KnowledgeError(format!(
            "Refusing to load path with traversal component: {}",
            path
        )));
    }

    let file_path = Path::new(path);

    if !FileLoader.is_supported(file_path) {
        return Err(UnevenError::KnowledgeError(format!(
            "File type not supported: {}",
            path
        )));
    }

    FileLoader.load_file(file_path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_is_supported() {
        let loader = FileLoader::new();
        assert!(loader.is_supported(Path::new("main.rs")));
        assert!(loader.is_supported(Path::new("app.ts")));
        assert!(loader.is_supported(Path::new("config.toml")));
        assert!(!loader.is_supported(Path::new("image.png")));
        assert!(!loader.is_supported(Path::new("no_ext")));
    }

    #[tokio::test]
    async fn test_load_file_content_ok() {
        let path = "test_load_content.txt";
        fs::write(path, "hello world").unwrap();
        let content = load_file_content(path).await.unwrap();
        assert_eq!(content, "hello world");
        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn test_load_file_content_unsupported() {
        let path = "test_load_content.bin";
        fs::write(path, vec![0, 1, 2]).unwrap();
        let result = load_file_content(path).await;
        assert!(result.is_err());
        if let Err(UnevenError::KnowledgeError(msg)) = result {
            assert!(msg.contains("supported"));
        } else {
            panic!("Expected KnowledgeError::FileNotSupported");
        }
        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn test_load_file_content_traversal_denied() {
        let result = load_file_content("../outside.txt").await;
        assert!(result.is_err());
        if let Err(UnevenError::KnowledgeError(msg)) = result {
            assert!(msg.contains("traversal"));
        } else {
            panic!("Expected KnowledgeError with traversal message");
        }
    }
}
