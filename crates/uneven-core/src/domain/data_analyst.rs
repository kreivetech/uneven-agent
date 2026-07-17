use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Postgresql,
    Mysql,
    Sqlite,
    Mongodb,
}

use crate::domain::database::TableSchema;

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueryResultMetadata {
    pub sql: String,
    pub explanation: String,
    pub executive_report: Option<String>,
    pub visual_suggestions: Option<VisualSuggestions>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<HashMap<String, Value>>,
    pub row_count: i64,
    pub duration_ms: u64,
    pub metadata: Option<QueryResultMetadata>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VisualSuggestions {
    pub title: String,
    pub explanation: Option<String>,
    pub layout: Option<Vec<Value>>,
    pub colors: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedQuery {
    pub sql: String,
    pub explanation: String,
    pub executive_report: Option<String>,
    pub cannot_generate: bool,
    pub visual_suggestions: Option<VisualSuggestions>,
}

pub trait DbAdapter: Send + Sync {
    fn db_type(&self) -> DbType;
    fn connect(&mut self, url: &str) -> futures::future::BoxFuture<'_, Result<(), crate::domain::error::UnevenError>>;
    fn disconnect(&mut self) -> futures::future::BoxFuture<'_, Result<(), crate::domain::error::UnevenError>>;
    fn introspect(&self, tables: Option<Vec<String>>) -> futures::future::BoxFuture<'_, Result<Vec<TableSchema>, crate::domain::error::UnevenError>>;
    fn discover_tables(&self) -> futures::future::BoxFuture<'_, Result<Vec<String>, crate::domain::error::UnevenError>>;
    fn execute(&self, sql: &str) -> futures::future::BoxFuture<'_, Result<Vec<HashMap<String, Value>>, crate::domain::error::UnevenError>>;
}
