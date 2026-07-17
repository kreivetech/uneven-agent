use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AnalystTaskStatus {
    #[default]
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnalystTask {
    pub id: String,
    pub table_name: String,
    pub status: AnalystTaskStatus,
    pub result_path: Option<String>,
    pub error: Option<String>,
    pub attempts: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnalystJob {
    pub id: String,
    pub start_time: u64,
    pub goal: String,
    pub db_url: String,
    pub tasks: Vec<AnalystTask>,
}
