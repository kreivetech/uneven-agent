//! Indexer orchestrator
//!
//! Coordinates the indexing of all knowledge sources,
//! generates embeddings, and stores in vector store.

use std::collections::VecDeque;
use std::path::{Component, PathBuf};
use crate::UnevenError;
use super::{IndexStats, IndexedDocument};
use super::file_loader::FileLoader;

pub struct KnowledgeIndexer;

impl KnowledgeIndexer {
    pub fn new() -> Self {
        KnowledgeIndexer
    }

    /// Indexes a directory and calls `on_doc` for each file found.
    /// This prevents buffering all documents in RAM, making it suitable for large projects.
    pub async fn index_directory<F>(
        &self, 
        dir_path: &str, 
        mut on_doc: F
    ) -> Result<IndexStats, UnevenError> 
    where
        F: FnMut(IndexedDocument) -> Result<(), UnevenError>
    {
        tracing::info!("Indexing directory: {}", dir_path);

        if PathBuf::from(dir_path)
            .components()
            .any(|c| c == Component::ParentDir)
        {
            return Err(UnevenError::KnowledgeError(format!(
                "Refusing to index path with traversal component: {}",
                dir_path
            )));
        }

        let root = PathBuf::from(dir_path);
        if !root.is_dir() {
            return Err(UnevenError::KnowledgeError(format!(
                "Path is not a directory: {}",
                dir_path
            )));
        }

        let root_path = crate::infrastructure::config::loader::find_project_root(None);
        let low_cpu_mode = crate::infrastructure::config::loader::load_config(&root_path)
            .ok()
            .map(|config| {
                config.watch.as_ref().and_then(|w| w.low_cpu_mode).unwrap_or(false)
                    || config.proactive.as_ref().and_then(|p| p.low_cpu_mode).unwrap_or(false)
            })
            .unwrap_or(false);

        let loader = FileLoader::new();
        let mut queue: VecDeque<PathBuf> = VecDeque::new();
        queue.push_back(root);

        let mut stats = IndexStats {
            total_documents: 0,
            disk_size: 0,
        };

        while let Some(dir) = queue.pop_front() {
            let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| {
                UnevenError::KnowledgeError(format!("Failed to read dir {:?}: {}", dir, e))
            })?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| UnevenError::KnowledgeError(format!("Failed to read entry: {}", e)))?
            {
                let file_type = entry.file_type().await.map_err(|e| {
                    UnevenError::KnowledgeError(format!("Failed to get file type: {}", e))
                })?;
                
                let path = entry.path();
                // Use symlink_metadata so symlinks to directories are NOT followed,
                // preventing infinite loops on circular symlink structures.
                let is_real_dir = !file_type.is_symlink() && file_type.is_dir();
                if is_real_dir {
                    queue.push_back(path);
                } else if file_type.is_file() && loader.is_supported(&path) {
                    if let Some(path_str) = path.to_str() {
                        if low_cpu_mode {
                            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                        }
                        let doc = self.load_single_doc(path_str).await?;
                        stats.total_documents += 1;
                        stats.disk_size += doc.content.len() as u64;
                        on_doc(doc)?;
                    }
                }
            }
        }

        Ok(stats)
    }

    async fn load_single_doc(&self, file_path: &str) -> Result<IndexedDocument, UnevenError> {
        tracing::debug!("Loading file for index: {}", file_path);
        let content = FileLoader::new()
            .load_file(std::path::Path::new(file_path))
            .await?;
            
        Ok(IndexedDocument {
            id: file_path.to_string(),
            path: file_path.to_string(),
            content,
            doc_type: "file".to_string(),
            metadata: std::collections::HashMap::new(),
        })
    }
}

impl Default for KnowledgeIndexer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[tokio::test]
    async fn test_index_directory_streaming() -> Result<(), UnevenError> {
        let test_dir = "test_knowledge_streaming";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        // Create 3 test files
        for i in 1..=3 {
            let path = format!("{}/file{}.ts", test_dir, i);
            let mut file = fs::File::create(path).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;
            writeln!(file, "console.log('file {}')", i).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;
        }

        let indexer = KnowledgeIndexer::new();
        let mut visited_files = 0;
        
        let stats = indexer.index_directory(test_dir, |_doc| {
            visited_files += 1;
            Ok(())
        }).await?;

        assert_eq!(visited_files, 3);
        assert_eq!(stats.total_documents, 3);
        assert!(stats.disk_size > 0);

        fs::remove_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;
        Ok(())
    }

    #[tokio::test]
    async fn test_refuse_traversal_component() {
        let indexer = KnowledgeIndexer::new();
        let result = indexer.index_directory("../outside", |_| Ok(())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_empty_directory_returns_zero_stats() -> Result<(), UnevenError> {
        let test_dir = "test_knowledge_empty";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        let indexer = KnowledgeIndexer::new();
        let stats = indexer.index_directory(test_dir, |_| Ok(())).await?;

        assert_eq!(stats.total_documents, 0);
        assert_eq!(stats.disk_size, 0);

        fs::remove_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;
        Ok(())
    }

    #[tokio::test]
    async fn test_unsupported_extension_not_indexed() -> Result<(), UnevenError> {
        let test_dir = "test_knowledge_unsupported";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        // .xyz is not in the supported extensions list
        fs::write(format!("{test_dir}/data.xyz"), "should be ignored")
            .map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;
        // .ts is supported
        fs::write(format!("{test_dir}/code.ts"), "const x = 1;")
            .map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        let indexer = KnowledgeIndexer::new();
        let mut count = 0;
        let stats = indexer.index_directory(test_dir, |_| { count += 1; Ok(()) }).await?;

        assert_eq!(count, 1, "only the .ts file should be indexed");
        assert_eq!(stats.total_documents, 1);

        fs::remove_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;
        Ok(())
    }

    #[tokio::test]
    async fn test_disk_size_matches_content_length() -> Result<(), UnevenError> {
        let test_dir = "test_knowledge_disksize";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        let content = "export const answer = 42;";
        fs::write(format!("{test_dir}/answer.ts"), content)
            .map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        let indexer = KnowledgeIndexer::new();
        let stats = indexer.index_directory(test_dir, |_| Ok(())).await?;

        assert_eq!(stats.disk_size, content.len() as u64);

        fs::remove_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;
        Ok(())
    }

    #[tokio::test]
    async fn test_on_doc_error_propagates() -> Result<(), UnevenError> {
        let test_dir = "test_knowledge_onerror";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        fs::write(format!("{test_dir}/file.ts"), "const x = 1;")
            .map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        let indexer = KnowledgeIndexer::new();
        let result = indexer.index_directory(test_dir, |_| {
            Err(UnevenError::KnowledgeError("injected error".to_string()))
        }).await;

        assert!(result.is_err(), "on_doc error should propagate");

        fs::remove_dir_all(test_dir).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;
        Ok(())
    }

    #[tokio::test]
    async fn test_non_existent_directory_returns_error() {
        let indexer = KnowledgeIndexer::new();
        let result = indexer.index_directory("/tmp/does_not_exist_uneven_test_xyz", |_| Ok(())).await;
        assert!(result.is_err());
    }
}
