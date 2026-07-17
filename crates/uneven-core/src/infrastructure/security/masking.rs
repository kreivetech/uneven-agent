//! Data masking and redaction module
//!
//! Provides utilities to detect and redact sensitive information from
//! structured data (CSVs, database results) before it reaches the AI.

use regex::Regex;
use once_cell::sync::Lazy;

/// Default list of column names that should always be redacted.
pub const BLOCKED_COLUMNS: &[&str] = &[
    "password", "passwd", "secret", "token", "key", "hash", "salt",
    "credit_card", "cc_number", "cvv", "ssn", "social_security",
    "access_token", "refresh_token", "api_key", "private_key",
    "cpf", "pin",
    // Wildcards (handled in logic)
    "*_password", "*_key", "*_secret", "*_token", "*_hash",
    "*_cpf", "*_ssn", "*_pin", "*_cvv",
];

/// Patterns to detect sensitive values in raw strings.
static RE_SENSITIVE_VALUES: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // BCrypt / Argon2 / SCrypt hashes
        Regex::new(r"^\$2[aby]\$\d{2}\$.{20,}$").unwrap(),
        Regex::new(r"^\$argon2").unwrap(),
        
        // JSON Web Tokens (JWT)
        Regex::new(r"^eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$").unwrap(),
        
        // AWS Access Keys
        Regex::new(r"^AKIA[0-9A-Z]{16}$").unwrap(),
        
        // Brazilian CPF
        Regex::new(r"^\d{3}\.?\d{3}\.?\d{3}-?\d{2}$").unwrap(),
        
        // Credit card numbers (basic 13-19 digits)
        Regex::new(r"^(?:\d[ -]?){13,19}$").unwrap(),
    ]
});

/// Returns true if the column name matches a blocked pattern.
pub fn is_blocked_column(name: &str) -> bool {
    let lower = name.to_lowercase();
    BLOCKED_COLUMNS.iter().any(|&pattern| {
        if let Some(stripped) = pattern.strip_prefix('*') {
            let suffix = stripped.to_lowercase();
            lower.ends_with(&suffix)
        } else {
            lower == pattern.to_lowercase()
        }
    })
}

/// Returns true if the value matches a sensitive pattern.
pub fn is_sensitive_value(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    RE_SENSITIVE_VALUES.iter().any(|re| re.is_match(trimmed))
}

pub struct SecurityContext {
    pub blocked_columns: Vec<String>,
    pub blocked_tables: Vec<String>,
    pub masked_columns: Vec<String>,
}

impl SecurityContext {
    pub fn new(blocked_columns: Vec<String>, blocked_tables: Vec<String>, masked_columns: Vec<String>) -> Self {
        Self { blocked_columns, blocked_tables, masked_columns }
    }

    pub fn is_blocked_column(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        
        // Check user config first
        if self.blocked_columns.iter().any(|p| {
            if let Some(stripped) = p.strip_prefix('*') {
                lower.ends_with(&stripped.to_lowercase())
            } else {
                lower == p.to_lowercase()
            }
        }) {
            return true;
        }

        // Fallback to defaults
        BLOCKED_COLUMNS.iter().any(|&pattern| {
            if let Some(stripped) = pattern.strip_prefix('*') {
                let suffix = stripped.to_lowercase();
                lower.ends_with(&suffix)
            } else {
                lower == pattern.to_lowercase()
            }
        })
    }

    pub fn is_masked_column(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.masked_columns.iter().any(|p| {
            if let Some(stripped) = p.strip_prefix('*') {
                lower.ends_with(&stripped.to_lowercase())
            } else {
                lower == p.to_lowercase()
            }
        })
    }

    pub fn is_blocked_table(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.blocked_tables.iter().any(|t| lower == t.to_lowercase())
    }

    pub fn is_safe_query(&self, sql: &str) -> bool {
        let normalized = sql.trim().to_uppercase();
        
        // Allow SELECT, WITH, and safe PRAGMA
        if !normalized.starts_with("SELECT") && !normalized.starts_with("WITH ") && !normalized.starts_with("PRAGMA ") {
            return false;
        }

        // Block dangerous keywords
        let dangerous = [
            "INSERT", "UPDATE", "DELETE", "DROP", "TRUNCATE", "ALTER", "CREATE", 
            "GRANT", "REVOKE", "EXEC", "EXECUTE"
        ];
        
        for kw in dangerous {
            let re = Regex::new(&format!(r"\b{}\b", kw)).unwrap();
            if re.is_match(&normalized) {
                return false;
            }
        }

        // Restrict PRAGMA to safe introspection
        if normalized.starts_with("PRAGMA ") {
            let safe_pragmas = ["TABLE_INFO", "INDEX_LIST", "FOREIGN_KEY_LIST"];
            if !safe_pragmas.iter().any(|p| normalized.contains(p)) {
                return false;
            }
        }

        true
    }

    pub fn audit_sql(&self, sql: &str) -> Result<(), String> {
        if !self.is_safe_query(sql) {
            return Err("SQL is not a safe SELECT statement or contains blocked keywords".to_string());
        }

        let lower_sql = sql.to_lowercase();

        // Blocked Table Check
        for table in &self.blocked_tables {
            let re = Regex::new(&format!(r"\b{}\b", table.to_lowercase())).unwrap();
            if re.is_match(&lower_sql) {
                return Err(format!("SQL references blocked table '{}'", table));
            }
        }

        // Blocked Column Check
        for col in &self.blocked_columns {
            let pattern = if let Some(stripped) = col.strip_prefix('*') {
                format!(r"\w*{}\b", regex::escape(&stripped.to_lowercase()))
            } else {
                format!(r"\b{}\b", regex::escape(&col.to_lowercase()))
            };
            let re = Regex::new(&pattern).unwrap();
            if re.is_match(&lower_sql) {
                return Err(format!("SQL references blocked column pattern '{}'", col));
            }
        }

        Ok(())
    }

    pub fn mask_value(&self, col_name: &str, value: &str) -> String {
        if self.is_blocked_column(col_name) || self.is_masked_column(col_name) || is_sensitive_value(value) {
            "***REDACTED***".to_string()
        } else {
            value.to_string()
        }
    }
}

// Global fallback for quick masking when context is not available
pub fn mask_value(col_name: &str, value: &str) -> String {
    let ctx = SecurityContext::new(vec![], vec![], vec![]);
    ctx.mask_value(col_name, value)
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
        // JWT example
        assert!(is_sensitive_value("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"));
        // CPF example
        assert!(is_sensitive_value("123.456.789-01"));
        assert!(!is_sensitive_value("normal text"));
    }

    #[test]
    fn test_mask_value_logic() {
        assert_eq!(mask_value("email", "test@test.com"), "test@test.com");
        assert_eq!(mask_value("password", "123456"), "***REDACTED***");
        assert_eq!(mask_value("data", "123.456.789-01"), "***REDACTED***");
    }
}
