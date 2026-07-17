//! Text chunking and splitting module
//!
//! Implements high-performance recursive splitting based on token counts.


use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chunk {
    pub id: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub metadata: HashMap<String, String>,
}

pub struct Chunker {
    #[cfg(feature = "llm")]
    tokenizer: Option<tokenizers::Tokenizer>,
}

impl Chunker {
    pub fn new(tokenizer_path: Option<&str>) -> Self {
        #[cfg(feature = "llm")]
        {
            let tokenizer = tokenizer_path.and_then(|p| {
                tokenizers::Tokenizer::from_file(p).ok()
            });
            Self { tokenizer }
        }
        #[cfg(not(feature = "llm"))]
        {
            Self {}
        }
    }

    /// Estimate token count (fallback or when no tokenizer provided)
    fn count_tokens(&self, text: &str) -> usize {
        #[cfg(feature = "llm")]
        if let Some(ref t) = self.tokenizer {
            return t.encode(text, false).map(|e| e.get_ids().len()).unwrap_or(text.len() / 4);
        }
        
        text.len() / 4
    }

    pub fn chunk_document(
        &self,
        content: &str,
        doc_id: &str,
        max_tokens: usize,
        overlap_tokens: usize,
        file_type: &str,
        metadata: &HashMap<String, String>,
    ) -> Vec<Chunk> {
        if content.is_empty() {
            return vec![];
        }

        // Strategy selection
        match file_type {
            "md" | "markdown" => self.split_markdown(content, doc_id, max_tokens, metadata),
            "ts" | "tsx" | "js" | "jsx" | "py" | "rs" | "go" | "java" | "cpp" | "c" => {
                self.split_recursive(content, doc_id, max_tokens, overlap_tokens, metadata, &["\n\n", "\n", " ", ""])
            }
            _ => self.split_recursive(content, doc_id, max_tokens, overlap_tokens, metadata, &["\n\n", "\n", " ", ""]),
        }
    }

    fn split_recursive(
        &self,
        content: &str,
        doc_id: &str,
        max_tokens: usize,
        _overlap_tokens: usize,
        metadata: &HashMap<String, String>,
        _separators: &[&str],
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut start_line = 1;
        let mut current_line_count = 0;
        let mut chunk_idx = 0;

        let lines: Vec<&str> = content.lines().collect();
        
        for (i, line) in lines.iter().enumerate() {
            let test_chunk = if current_chunk.is_empty() {
                line.to_string()
            } else {
                format!("{}\n{}", current_chunk, line)
            };

            if self.count_tokens(&test_chunk) > max_tokens && !current_chunk.is_empty() {
                chunks.push(Chunk {
                    id: format!("{}-chunk-{}", doc_id, chunk_idx),
                    content: current_chunk.trim().to_string(),
                    start_line,
                    end_line: start_line + current_line_count - 1,
                    metadata: self.build_metadata(metadata, chunk_idx, start_line, start_line + current_line_count - 1),
                });

                chunk_idx += 1;
                // Simple overlap: last 2 lines if possible
                let overlap_start = if i >= 2 { i - 2 } else { i };
                current_chunk = lines[overlap_start..=i].join("\n");
                start_line = overlap_start + 1;
                current_line_count = i - overlap_start + 1;
            } else {
                if current_chunk.is_empty() {
                    start_line = i + 1;
                }
                current_chunk = test_chunk;
                current_line_count += 1;
            }
        }

        if !current_chunk.trim().is_empty() {
            chunks.push(Chunk {
                id: format!("{}-chunk-{}", doc_id, chunk_idx),
                content: current_chunk.trim().to_string(),
                start_line,
                end_line: lines.len(),
                metadata: self.build_metadata(metadata, chunk_idx, start_line, lines.len()),
            });
        }

        chunks
    }

    fn split_markdown(
        &self,
        content: &str,
        doc_id: &str,
        max_tokens: usize,
        metadata: &HashMap<String, String>,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_section = String::new();
        let mut start_line = 1;
        let mut section_idx = 0;

        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let is_heading = line.starts_with('#');

            if is_heading && !current_section.trim().is_empty() {
                if self.count_tokens(&current_section) > max_tokens {
                    // If section is too big, sub-split it recursively
                    let sub_chunks = self.split_recursive(&current_section, &format!("{}-s{}", doc_id, section_idx), max_tokens, 0, metadata, &["\n", " ", ""]);
                    chunks.extend(sub_chunks);
                } else {
                    chunks.push(Chunk {
                        id: format!("{}-section-{}", doc_id, section_idx),
                        content: current_section.trim().to_string(),
                        start_line,
                        end_line: i,
                        metadata: self.build_metadata(metadata, section_idx, start_line, i),
                    });
                }
                
                section_idx += 1;
                current_section = String::new();
                start_line = i + 1;
            }

            current_section.push_str(line);
            current_section.push('\n');
        }

        if !current_section.trim().is_empty() {
            chunks.push(Chunk {
                id: format!("{}-section-{}", doc_id, section_idx),
                content: current_section.trim().to_string(),
                start_line,
                end_line: lines.len(),
                metadata: self.build_metadata(metadata, section_idx, start_line, lines.len()),
            });
        }

        chunks
    }

    fn build_metadata(&self, base: &HashMap<String, String>, idx: usize, start: usize, end: usize) -> HashMap<String, String> {
        let mut meta = base.clone();
        meta.insert("chunk_index".into(), idx.to_string());
        meta.insert("lines".into(), format!("{}-{}", start, end));
        meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_chunking() {
        let chunker = Chunker::new(None);
        let content = "line 1\nline 2\nline 3\nline 4\nline 5";
        let meta = HashMap::new();
        // Force split at ~2 lines (8 chars / 4 = 2 tokens approx, actually more like 3-4 chars per line)
        let chunks = chunker.chunk_document(content, "test", 2, 0, "txt", &meta);
        
        assert!(chunks.len() > 1);
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn test_markdown_splitting() {
        let chunker = Chunker::new(None);
        let content = "# Title\nBody\n## Subtitle\nMore body";
        let meta = HashMap::new();
        let chunks = chunker.chunk_document(content, "test", 100, 0, "md", &meta);
        
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].content.contains("# Title"));
        assert!(chunks[1].content.contains("## Subtitle"));
    }
}
