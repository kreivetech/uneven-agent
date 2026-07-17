use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub output_dir: String,
    pub file_name: Option<String>,
    pub sheet_name: Option<String>,
    pub freeze_header: Option<bool>,
    pub auto_fit: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub file_path: String,
    pub sheets: u32,
    pub rows_written: u32,
}
