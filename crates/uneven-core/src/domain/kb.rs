use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDocument {
    pub id: String,
    pub path: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WebDocument {
    pub id: String,
    pub url: String,
    pub content: String,
    pub title: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalResult {
    pub documents: Vec<String>,
    pub scores: Vec<f32>,
}
