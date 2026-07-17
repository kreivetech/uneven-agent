use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PackageTarget {
    WinX64,
    WinArm64,
    MacosX64,
    MacosArm64,
    LinuxX64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackageOptions {
    pub output_dir: String,
    pub file_name: Option<String>,
    pub target: Option<PackageTarget>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackageResult {
    pub exe_path: String,
    pub html_path: String,
    pub size_mb: f32,
    pub offline: bool,
}
