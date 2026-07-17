use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RetrievalResult {
    pub documents: Vec<String>,
    pub scores: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredDocument {
    pub id: String,
    pub path: String,
    pub content: String,
    pub doc_type: String,
}
