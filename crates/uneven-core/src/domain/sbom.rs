use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    #[serde(rename = "SHA-1")]
    Sha1,
    #[serde(rename = "SHA-256")]
    Sha256,
    #[serde(rename = "SHA-512")]
    Sha512,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXHash {
    pub alg: Option<HashAlgorithm>,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXLicenseDetails {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXLicense {
    pub license: CycloneDXLicenseDetails,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CycloneDXComponentType {
    Library,
    Application,
    Framework,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CycloneDXComponentScope {
    Required,
    Optional,
    Excluded,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CycloneDXRefType {
    Vcs,
    IssueTracker,
    Website,
    Advisories,
    Bom,
    MailingList,
    Social,
    Chat,
    Documentation,
    Support,
    Distribution,
    License,
    BuildMeta,
    BuildSystem,
    ReleaseNotes,
    Other,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXExternalReference {
    pub r#type: CycloneDXRefType,
    pub url: String,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXComponent {
    pub r#type: CycloneDXComponentType,
    pub name: String,
    pub version: String,
    pub purl: String,
    pub description: Option<String>,
    pub licenses: Option<Vec<CycloneDXLicense>>,
    pub hashes: Option<Vec<CycloneDXHash>>,
    pub supplier: Option<String>,
    pub scope: Option<CycloneDXComponentScope>,
    pub modified: Option<bool>,
    #[serde(rename = "bom-ref")]
    pub bom_ref: Option<String>,
    pub external_references: Option<Vec<CycloneDXExternalReference>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXVulnSource {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXVulnRating {
    pub score: f32,
    pub severity: String,
    pub method: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXVulnAffects {
    pub r#ref: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CycloneDXVulnState {
    Resolved,
    ResolvedWithPedigree,
    NotAffected,
    InStock,
    WaitingForFix,
    UnderInvestigation,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CycloneDXVulnJustification {
    CodeNotPresent,
    CodeNotReachable,
    RequiresConfiguration,
    RequiresSpecificEnvironment,
    RequiresNoPublicIp,
    ProtectedByCompiler,
    ProtectedAtRuntime,
    ProtectedAtPerimeter,
    ProtectedByMitigatingControl,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CycloneDXVulnResponse {
    CanNotFix,
    WillNotFix,
    Update,
    Rollback,
    WorkaroundAvailable,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXVulnAnalysis {
    pub state: Option<CycloneDXVulnState>,
    pub justification: Option<CycloneDXVulnJustification>,
    pub responses: Option<Vec<CycloneDXVulnResponse>>,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXVulnerability {
    pub id: String,
    pub source: Option<CycloneDXVulnSource>,
    pub ratings: Option<Vec<CycloneDXVulnRating>>,
    pub description: String,
    pub recommendation: String,
    pub affects: Vec<CycloneDXVulnAffects>,
    pub analysis: Option<CycloneDXVulnAnalysis>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXTool {
    pub vendor: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXMetadataComponent {
    pub r#type: String, // usually "application"
    pub name: String,
    pub version: Option<String>,
    #[serde(rename = "bom-ref")]
    pub bom_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXMetadata {
    pub timestamp: String,
    pub tools: Vec<CycloneDXTool>,
    pub component: CycloneDXMetadataComponent,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXDependency {
    pub r#ref: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDXBOM {
    pub bom_format: String, // usually "CycloneDX"
    pub spec_version: String, // usually "1.4"
    pub serial_number: String,
    pub version: u32,
    pub metadata: CycloneDXMetadata,
    pub components: Vec<CycloneDXComponent>,
    pub vulnerabilities: Vec<CycloneDXVulnerability>,
    pub dependencies: Option<Vec<CycloneDXDependency>>,
}
