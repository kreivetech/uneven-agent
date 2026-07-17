use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReportFormat {
    Md,
    Html,
    Both,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportOptions {
    pub format: Option<ReportFormat>,
    pub output_dir: Option<String>,
    pub project_name: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportResult {
    pub md_path: Option<String>,
    pub html_path: Option<String>,
}
