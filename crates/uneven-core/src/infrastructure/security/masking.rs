//! Data masking and redaction module
//!
//! Provides utilities to detect and redact sensitive information from
//! structured data (CSVs, database results) before it reaches the AI.
//! Powered by `shieldcraft`.

pub use shieldcraft::{
    is_blocked_column, is_safe_query, is_sensitive_value, BLOCKED_COLUMNS,
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SecurityContext {
    pub blocked_columns: Vec<String>,
    pub blocked_tables: Vec<String>,
    pub masked_columns: Vec<String>,
}

impl SecurityContext {
    pub fn new(
        blocked_columns: Vec<String>,
        blocked_tables: Vec<String>,
        masked_columns: Vec<String>,
    ) -> Self {
        Self {
            blocked_columns,
            blocked_tables,
            masked_columns,
        }
    }

    fn to_shieldcraft(&self) -> shieldcraft::SecurityContext {
        shieldcraft::SecurityContext::new(
            self.blocked_columns.clone(),
            self.blocked_tables.clone(),
            self.masked_columns.clone(),
        )
    }

    pub fn is_blocked_column(&self, name: &str) -> bool {
        self.to_shieldcraft().is_blocked_column(name)
    }

    pub fn is_masked_column(&self, name: &str) -> bool {
        self.to_shieldcraft().is_masked_column(name)
    }

    pub fn is_blocked_table(&self, name: &str) -> bool {
        self.to_shieldcraft().is_blocked_table(name)
    }

    pub fn is_safe_query(&self, sql: &str) -> bool {
        shieldcraft::is_safe_query(sql)
    }

    pub fn audit_sql(&self, sql: &str) -> Result<(), String> {
        self.to_shieldcraft().audit_sql(sql).map_err(|e| e.to_string())
    }

    pub fn mask_value(&self, col_name: &str, value: &str) -> String {
        if self.is_blocked_column(col_name)
            || self.is_masked_column(col_name)
            || is_sensitive_value(value)
        {
            "***REDACTED***".to_string()
        } else {
            value.to_string()
        }
    }
}

// Global fallback for quick masking when context is not available
pub fn mask_value(col_name: &str, value: &str) -> String {
    if is_blocked_column(col_name) || is_sensitive_value(value) {
        "***REDACTED***".to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_masking() {
        assert!(is_blocked_column("password"));
        assert!(is_blocked_column("user_password"));
        assert!(is_blocked_column("API_KEY"));
        assert!(!is_blocked_column("username"));
    }

    #[test]
    fn test_value_masking() {
        assert!(is_sensitive_value(
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        ));
        assert!(is_sensitive_value("123.456.789-09"));
        assert!(!is_sensitive_value("111.111.111-11"));
        assert!(!is_sensitive_value("normal text"));
    }

    #[test]
    fn test_mask_value_logic() {
        assert_eq!(mask_value("email", "test@test.com"), "test@test.com");
        assert_eq!(mask_value("password", "123456"), "***REDACTED***");
        assert_eq!(mask_value("data", "123.456.789-09"), "***REDACTED***");
    }
}
