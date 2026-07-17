use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum VulnSeverity {
    #[default]
    Info,
    Low,
    Moderate,
    High,
    Critical,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FixDetails {
    pub name: String,
    pub version: String,
    pub is_sem_ver_major: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum FixAvailable {
    Boolean(bool),
    Details(FixDetails),
}

impl Default for FixAvailable {
    fn default() -> Self {
        FixAvailable::Boolean(false)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DepVulnerability {
    pub name: String,
    pub severity: VulnSeverity,
    pub is_direct: bool,
    pub range: String,
    pub fix_available: FixAvailable,
    pub via: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DepAuditResult {
    pub vulnerabilities: Vec<DepVulnerability>,
    pub skipped: bool,
    pub error: Option<String>,
}
