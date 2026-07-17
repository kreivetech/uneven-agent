use crate::UnevenError;
use std::path::Path;
use calamine::{Reader, Xlsx};
use pdf_extract::extract_text;

use crate::infrastructure::security::SecurityContext;
use std::sync::Arc;

pub struct DocumentLoader {
    security: Arc<SecurityContext>,
}

impl DocumentLoader {
    pub fn new(security: Arc<SecurityContext>) -> Self {
        DocumentLoader { security }
    }

    pub async fn load_csv(&self, path: &Path) -> Result<String, UnevenError> {
        let path = path.to_path_buf();
        let security = self.security.clone();
        tokio::task::spawn_blocking(move || {
            let mut reader = csv::Reader::from_path(&path)
                .map_err(|e| UnevenError::KnowledgeError(format!("CSV error: {}", e)))?;
            
            let mut markdown = String::new();
            let headers = reader.headers()
                .map_err(|e| UnevenError::KnowledgeError(format!("CSV headers error: {}", e)))?
                .clone();

            let header_list: Vec<String> = headers.iter().map(|s| s.to_string()).collect();

            markdown.push_str("| ");
            markdown.push_str(&header_list.join(" | "));
            markdown.push_str(" |\n");

            markdown.push_str("| ");
            markdown.push_str(&header_list.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
            markdown.push_str(" |\n");

            for result in reader.records() {
                let record = result.map_err(|e| UnevenError::KnowledgeError(format!("CSV record error: {}", e)))?;
                markdown.push_str("| ");
                let masked_row: Vec<String> = record.iter().enumerate().map(|(i, val)| {
                    let col_name = header_list.get(i).map(|s| s.as_str()).unwrap_or("");
                    security.mask_value(col_name, val)
                }).collect();
                markdown.push_str(&masked_row.join(" | "));
                markdown.push_str(" |\n");
            }

            Ok(markdown)
        }).await.map_err(|e| UnevenError::KnowledgeError(format!("Task failed: {e}")))?
    }

    pub async fn load_excel(&self, path: &Path) -> Result<String, UnevenError> {
        let path = path.to_path_buf();
        let security = self.security.clone();
        tokio::task::spawn_blocking(move || {
            let mut workbook: Xlsx<_> = calamine::open_workbook(&path)
                .map_err(|e| UnevenError::KnowledgeError(format!("Excel error: {}", e)))?;
            
            let mut markdown = String::new();
            let sheet_names = workbook.sheet_names().to_vec();

            for name in sheet_names {
                if security.is_blocked_table(&name) {
                    continue;
                }
                if let Ok(range) = workbook.worksheet_range(&name) {
                    markdown.push_str(&format!("## Sheet: {}\n\n", name));
                    
                    let mut rows = range.rows().take(800);
                    let mut headers = Vec::new();

                    if let Some(header_row) = rows.next() {
                        headers = header_row.iter().map(|c| c.to_string()).collect();
                        markdown.push_str("| ");
                        markdown.push_str(&headers.join(" | "));
                        markdown.push_str(" |\n");

                        markdown.push_str("| ");
                        markdown.push_str(&headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
                        markdown.push_str(" |\n");
                    }

                    for row in rows {
                        markdown.push_str("| ");
                        let masked_row: Vec<String> = row.iter().enumerate().map(|(i, val)| {
                            let val_str = val.to_string();
                            let col_name = headers.get(i).map(|s| s.as_str()).unwrap_or("");
                            security.mask_value(col_name, &val_str)
                        }).collect();
                        markdown.push_str(&masked_row.join(" | "));
                        markdown.push_str(" |\n");
                    }
                    markdown.push('\n');
                }
            }

            Ok(markdown)
        }).await.map_err(|e| UnevenError::KnowledgeError(format!("Task failed: {e}")))?
    }

    pub async fn load_pdf(&self, path: &Path) -> Result<String, UnevenError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let content = extract_text(&path)
                .map_err(|e| UnevenError::KnowledgeError(format!("PDF error: {}", e)))?;
            
            Ok(format!("## PDF Document: {:?}\n\n{}", path.file_name().unwrap_or_default(), content))
        }).await.map_err(|e| UnevenError::KnowledgeError(format!("Task failed: {e}")))?
    }

    pub async fn load_docx(&self, path: &Path) -> Result<String, UnevenError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let file = std::fs::File::open(&path)
                .map_err(|e| UnevenError::KnowledgeError(format!("Failed to open DOCX: {e}")))?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| UnevenError::KnowledgeError(format!("Failed to read ZIP: {e}")))?;
            let mut xml_file = archive.by_name("word/document.xml")
                .map_err(|e| UnevenError::KnowledgeError(format!("Failed to find word/document.xml: {e}")))?;
            let mut xml_content = String::new();
            xml_file.read_to_string(&mut xml_content)
                .map_err(|e| UnevenError::KnowledgeError(format!("Failed to read word/document.xml: {e}")))?;

            let re = regex::Regex::new(r"<w:t[^>]*>([^<]*)</w:t>").unwrap();
            let mut text = String::new();
            for cap in re.captures_iter(&xml_content) {
                text.push_str(&cap[1]);
            }
            
            Ok(format!("## Word Document: {}\n\n{}", path.file_name().unwrap_or_default().to_string_lossy(), text))
        }).await.map_err(|e| UnevenError::KnowledgeError(format!("Task failed: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_load_csv() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name,age\nAlice,30\nBob,25").unwrap();
        
        let security = Arc::new(SecurityContext::new(vec![], vec![], vec![]));
        let loader = DocumentLoader::new(security);
        let markdown = loader.load_csv(file.path()).await.unwrap();
        
        assert!(markdown.contains("| name | age |"));
        assert!(markdown.contains("| Alice | 30 |"));
        assert!(markdown.contains("| Bob | 25 |"));
    }

    #[tokio::test]
    async fn test_load_csv_masking() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name,password,token\nAlice,secret123,eyJhbGciOiJIUzI1NiJ9...").unwrap();
        
        let security = Arc::new(SecurityContext::new(vec![], vec![], vec![]));
        let loader = DocumentLoader::new(security);
        let markdown = loader.load_csv(file.path()).await.unwrap();
        
        assert!(markdown.contains("| name | password | token |"));
        assert!(markdown.contains("| Alice | ***REDACTED*** | ***REDACTED*** |"));
        assert!(!markdown.contains("secret123"));
    }

    #[tokio::test]
    async fn test_load_docx() {
        let mut temp_file = NamedTempFile::new().unwrap();
        
        {
            let mut zip = zip::ZipWriter::new(temp_file.as_file_mut());
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("word/document.xml", options).unwrap();
            zip.write_all(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello from docx!</w:t></w:r></w:p></w:body></w:document>"#.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let security = Arc::new(SecurityContext::new(vec![], vec![], vec![]));
        let loader = DocumentLoader::new(security);
        let content = loader.load_docx(temp_file.path()).await.unwrap();

        assert!(content.contains("Hello from docx!"));
    }
}

