//! Text chunking and splitting module
//!
//! Powered by `ragcraft`.

pub use ragcraft::{Chunk, Chunker};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_chunking() {
        let chunker = Chunker::new(None);
        let content = "line 1\nline 2\nline 3\nline 4\nline 5";
        let meta = std::collections::HashMap::new();
        let chunks = chunker.chunk_document(content, "test", 2, 0, "txt", &meta);

        assert!(chunks.len() > 1);
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn test_markdown_splitting() {
        let chunker = Chunker::new(None);
        let content = "# Title\nBody\n## Subtitle\nMore body";
        let meta = std::collections::HashMap::new();
        let chunks = chunker.chunk_document(content, "test", 100, 0, "md", &meta);

        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].content.contains("# Title"));
        assert!(chunks[1].content.contains("## Subtitle"));
    }
}
