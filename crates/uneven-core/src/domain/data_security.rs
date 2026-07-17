use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPolicy {
    pub max_rows_per_analysis: Option<usize>,
    pub max_analysis_duration_ms: Option<u64>,
    pub blocked_columns: Option<Vec<String>>,
    pub blocked_tables: Option<Vec<String>>,
    pub masked_columns: Option<Vec<String>>,
    pub sensitive_value_patterns: Option<Vec<String>>, // Stores regex patterns as raw strings
    pub report_redactions: Option<bool>,
}

impl SecurityPolicy {
    pub fn default_policy() -> Self {
        SecurityPolicy {
            max_rows_per_analysis: Some(1),
            max_analysis_duration_ms: Some(30000),
            blocked_columns: Some(
                vec![
                    "password", "passwd", "secret", "token", "key", "hash", "salt",
                    "credit_card", "cc_number", "cvv", "ssn", "social_security",
                    "access_token", "refresh_token", "api_key", "private_key",
                    "cpf", "pin", "ssn", "cvv",
                    "*_password", "*_key", "*_secret", "*_token", "*_hash",
                    "*_cpf", "*_ssn", "*_pin", "*_cvv",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
            ),
            blocked_tables: Some(
                vec![
                    "sessions", "user_sessions", "oauth_tokens", "refresh_tokens",
                    "password_reset_tokens", "api_keys", "audit_logs", "security_events",
                    "mfa_codes", "backup_codes", "encryption_keys",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
            ),
            sensitive_value_patterns: Some(
                vec![
                    r#"^\$2[aby]\$\d{2}\$.{20,}$"#,                  // BCrypt
                    r#"^\$argon2"#,                                  // Argon2
                    r#"^\$s0\$"#,                                    // SCrypt
                    r#"^eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$"#, // JWT
                    r#"^AKIA[0-9A-Z]{16}$"#,                          // AWS Access Key
                    r#"^\d{3}\.?\d{3}\.?\d{3}-?\d{2}$"#,             // Brazilian CPF
                    r#"^(?:\d[ -]?){13,19}$"#,                       // Credit Card
                    r#"^-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"#, // PEM
                    r#"^[0-9a-f]{32,}$"#,                            // Generic Hex
                ]
                .into_iter()
                .map(String::from)
                .collect(),
            ),
            report_redactions: Some(true),
            ..Default::default()
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditResult {
    pub was_blocked: bool,
    pub reasons: Vec<String>,
}

pub static SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", "dist", "build", "target", ".uneven",
    "coverage", ".nyc_output", "__pycache__", ".pytest_cache",
    "vendor", ".cargo",
];

pub static TEXT_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs",
    "py", "rb", "php", "go", "java", "rs",
    "sh", "bash", "zsh", "ps1",
    "json", "yaml", "yml", "toml",
];
