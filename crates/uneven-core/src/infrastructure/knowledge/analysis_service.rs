use crate::domain::{TableSchema, UnevenConfig};
use crate::infrastructure::knowledge::database_loader::DatabaseAnalyst;
use crate::infrastructure::knowledge::parser::{GeneratedQuery, ResponseParser};
use crate::infrastructure::knowledge::prompts::{
    build_analysis_prompt, build_sql_gen_prompt, build_style_extraction_prompt,
};
use crate::infrastructure::llm::InferParams;
use crate::infrastructure::security::masking::BLOCKED_COLUMNS;
use crate::infrastructure::security::SecurityContext;
use crate::UnevenError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub sql: String,
    pub explanation: String,
    pub executive_report: String,
    pub visual_suggestions: Option<Value>,
    pub cannot_generate: bool,
    pub rows: Vec<Value>,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalystTask {
    pub id: String,
    #[serde(rename = "tableName")]
    pub table_name: String,
    pub status: String, // "pending", "completed", "failed"
    pub attempts: u32,
    #[serde(rename = "resultPath")]
    pub result_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalystJob {
    pub id: String,
    #[serde(rename = "startTime")]
    pub start_time: i64,
    pub goal: String,
    #[serde(rename = "dbUrl")]
    pub db_url: String,
    pub tasks: Vec<AnalystTask>,
}

/// Service orchestrating blind data analysis, query generation, safety verification, and execution.
pub struct AnalysisService {
    pub analyst: DatabaseAnalyst,
    pub security: Arc<SecurityContext>,
    pub system_instructions: String,
}

impl AnalysisService {
    pub fn new(
        analyst: DatabaseAnalyst,
        security: Arc<SecurityContext>,
        system_instructions: String,
    ) -> Self {
        Self {
            analyst,
            security,
            system_instructions,
        }
    }

    /// Formats the database schema for the AI prompt, masking sample data and filtering sensitive columns.
    pub fn format_schema(&self, schema: &[TableSchema]) -> String {
        format_schema_for_ai(schema, &self.security)
    }

    /// Generates a SQL query and metadata from a natural language prompt without executing it.
    pub async fn generate_query(
        &self,
        db_type: Option<&str>,
        schema: &[TableSchema],
        user_prompt: &str,
        infer_params: &InferParams,
    ) -> Result<GeneratedQuery, UnevenError> {
        if !crate::application::LicenseService::check_pro() {
            crate::application::LicenseService::check_and_increment_usage("analyze")?;
        }
        let schema_text = self.format_schema(schema);
        let rules = build_prompt_rules(&self.security, &self.system_instructions);
        let sql_prompt = build_sql_gen_prompt(db_type, &schema_text, user_prompt, &rules);

        let response = crate::infrastructure::llm::infer_with_params(&sql_prompt, infer_params)
            .await
            .map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        let generated = ResponseParser::parse(&response.content);
        Ok(generated)
    }

    /// Validates safety and executes the query against the connected database.
    pub async fn execute_query(&self, sql: &str) -> Result<Vec<Value>, UnevenError> {
        if !is_safe_query(sql) {
            return Err(UnevenError::SecurityError(
                "Query blocked: only SELECT, WITH, and PRAGMA read queries are permitted.".to_string(),
            ));
        }
        self.analyst.execute_query(sql).await
    }

    /// Executes multiple named queries safely, returning a map of dataset_name -> rows
    pub async fn execute_multi_queries(
        &self,
        queries: &[crate::domain::dashboard_spec::NamedQuery],
    ) -> Result<std::collections::HashMap<String, Vec<Value>>, UnevenError> {
        let mut results = std::collections::HashMap::new();
        for q in queries {
            let rows = self.execute_query(&q.sql).await?;
            results.insert(q.name.clone(), rows);
        }
        Ok(results)
    }

    /// Extracts visual theme and dashboard styling options from a user prompt.
    pub async fn extract_style_and_theme(
        &self,
        user_prompt: &str,
        columns: &[String],
        infer_params: &InferParams,
    ) -> Option<Value> {
        let style_prompt = build_style_extraction_prompt(user_prompt, columns);
        if let Ok(res) = crate::infrastructure::llm::infer_with_params(&style_prompt, infer_params).await {
            ResponseParser::extract_theme(&res.content)
        } else {
            None
        }
    }

    /// Generates analytical suggestions based on table schemas using the configured LLM.
    pub async fn suggest(
        &self,
        schema: &[TableSchema],
        config: &UnevenConfig,
        threads: u32,
    ) -> Result<Vec<String>, UnevenError> {
        suggest_analyses(schema, config, &self.system_instructions, threads, &self.security).await
    }
}

/// Formats the schema for analysis suggestion prompts, filtering blocked columns.
pub fn format_schema_for_suggestion(schema: &[TableSchema], security: &SecurityContext) -> String {
    let limit = schema.len().min(20);
    schema[..limit]
        .iter()
        .map(|t| {
            let cols = t
                .columns
                .iter()
                .filter(|c| !security.is_blocked_column(&c.name))
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("  {}({})", t.name, cols)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats the database schema for the LLM, filtering blocked columns and masking samples.
pub fn format_schema_for_ai(schema: &[TableSchema], security: &SecurityContext) -> String {
    if schema.is_empty() {
        return "No tables available".to_string();
    }

    let limit = schema.len().min(20);
    let tables = &schema[..limit];
    let details: Vec<String> = tables
        .iter()
        .map(|t| {
            let cols = t
                .columns
                .iter()
                .filter(|c| !security.is_blocked_column(&c.name))
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(",");

            let fks_str = if let Some(ref fks) = t.foreign_keys {
                if !fks.is_empty() {
                    let fks_mapped = fks
                        .iter()
                        .filter(|fk| !security.is_blocked_column(&fk.column))
                        .map(|fk| {
                            format!(
                                "{}->{}.{}",
                                fk.column, fk.referenced_table, fk.referenced_column
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    if !fks_mapped.is_empty() {
                        format!(" | RELATIONSHIPS: {}", fks_mapped)
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            };

            let samples_str = if let Some(ref samples) = t.samples {
                if !samples.is_empty() {
                    let formatted_samples = format_samples_for_ai(samples, security);
                    if !formatted_samples.is_empty() {
                        format!(" | SAMPLE: {}", formatted_samples)
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            };
            format!("{}{{{}}}{}{}", t.name, cols, fks_str, samples_str)
        })
        .collect();

    format!(
        "CONTEXT (Tables & Relationships & Samples):\n{}",
        details.join("\n")
    )
}

/// Formats sample rows by masking values through the security context and removing blocked columns.
pub fn format_samples_for_ai(samples: &[Value], security: &SecurityContext) -> String {
    if samples.is_empty() {
        return "".to_string();
    }
    let first = &samples[0];
    if let Value::Object(map) = first {
        map.iter()
            .filter(|(k, _)| !security.is_blocked_column(k))
            .map(|(k, v)| {
                let val_str = match v {
                    Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                let masked = security.mask_value(k, &val_str);
                let display_val = if masked == "***REDACTED***" {
                    "***REDACTED***".to_string()
                } else if masked.len() > 30 {
                    format!("{}...", &masked[..30])
                } else {
                    masked
                };
                format!("{}={}", k, display_val)
            })
            .collect::<Vec<_>>()
            .join("|")
    } else {
        "".to_string()
    }
}

/// Builds prompt security rules restricting blocked tables and columns.
pub fn build_prompt_rules(security: &SecurityContext, blindados_laws: &str) -> String {
    let mut exact_cols = security.blocked_columns.clone();
    for col in BLOCKED_COLUMNS {
        let col_str = col.to_string();
        if !exact_cols.contains(&col_str) {
            exact_cols.push(col_str);
        }
    }

    let mut lines = vec![
        "SECURITY RULES — you MUST follow these unconditionally:".to_string(),
        format!(
            "- NEVER SELECT these BLOCKED columns (they are forbidden): {}",
            exact_cols.join(", ")
        ),
        format!(
            "- NEVER query these BLOCKED tables: {}",
            security.blocked_tables.join(", ")
        ),
        "- If the user request requires any of the BLOCKED items, respond with: SECURITY_BLOCKED"
            .to_string(),
    ];

    lines.retain(|l| l.len() > 40 || l.starts_with("SECURITY"));

    let mut final_prompt = lines.join("\n");
    if !blindados_laws.is_empty() {
        final_prompt.push_str(&format!(
            "\n\n### GLOBAL SYSTEM PROTOCOLS (BLINDADOS)\n{}",
            blindados_laws
        ));
    }

    final_prompt
}

/// Checks if a query is strictly read-only (`SELECT`, `WITH`, `PRAGMA`).
pub fn is_safe_query(sql: &str) -> bool {
    shieldcraft::is_safe_query(sql)
}

/// Generates fallback analytical suggestions synchronously from table schemas.
pub fn suggest_analyses_sync(schema: &[TableSchema]) -> Vec<String> {
    if schema.is_empty() {
        return vec![];
    }
    let mut sug = vec![];
    for t in schema.iter().take(5) {
        sug.push(format!("Show a summary of all records in table {}", t.name));
    }
    sug
}

/// Generates contextual analysis suggestions from database schemas using LLM inference with fallback.
pub async fn suggest_analyses(
    schema: &[TableSchema],
    config: &UnevenConfig,
    system_instructions: &str,
    threads: u32,
    security: &SecurityContext,
) -> Result<Vec<String>, UnevenError> {
    if schema.is_empty() {
        return Ok(vec![]);
    }
    let schema_text = format_schema_for_suggestion(schema, security);
    let prompt = build_analysis_prompt(&schema_text);

    let brain = config.get_brain_for_command("analyze");
    let params = InferParams {
        model: brain.model.clone(),
        threads,
        max_tokens: 200,
        temperature: brain.temperature as f64,
        repeat_penalty: 1.1,
        max_context: 2048,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: Some(system_instructions.to_string()),
        history: None,
    };

    let response = match crate::infrastructure::llm::infer_with_params(&prompt, &params).await {
        Ok(res) => res,
        Err(_) => return Ok(suggest_analyses_sync(schema)),
    };

    let content = &response.content;
    let mut target_content = content.as_str();

    if let Some(start_idx) = content.find("<suggestions>") {
        let after_start = &content[start_idx + "<suggestions>".len()..];
        if let Some(end_idx) = after_start.find("</suggestions>") {
            target_content = &after_start[..end_idx];
        } else {
            target_content = after_start;
        }
    } else if let Some(start_idx) = content.to_lowercase().find("[suggestions]") {
        let after_start = &content[start_idx + "[suggestions]".len()..];
        if let Some(end_idx) = after_start.to_lowercase().find("[/suggestions]") {
            target_content = &after_start[..end_idx];
        } else {
            target_content = after_start;
        }
    }

    let suggestions: Vec<String> = target_content
        .split('\n')
        .map(|l| {
            let re = regex::Regex::new(r"^[•\-\d.]+\s*").unwrap();
            re.replace(l, "").trim().to_string()
        })
        .filter(|l| {
            let len = l.len();
            len > 10
                && !l.starts_with('<')
                && !l.starts_with('[')
                && !l.to_lowercase().contains("suggestion")
        })
        .take(5)
        .collect();

    if suggestions.is_empty() {
        Ok(suggest_analyses_sync(schema))
    } else {
        Ok(suggestions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::database::ColumnInfo;

    #[test]
    fn test_is_safe_query() {
        assert!(is_safe_query("SELECT * FROM users"));
        assert!(is_safe_query("WITH cte AS (SELECT 1) SELECT * FROM cte"));
        assert!(is_safe_query("PRAGMA table_info(users)"));
        assert!(!is_safe_query("INSERT INTO users VALUES (1)"));
        assert!(!is_safe_query("DROP TABLE users"));
        assert!(!is_safe_query("UPDATE users SET name='x'"));
        assert!(!is_safe_query("DELETE FROM users"));
    }

    #[test]
    fn test_format_schema_and_samples_filters_sensitive() {
        let security = SecurityContext::new(vec![], vec![], vec![]);
        let schema = vec![TableSchema {
            name: "users".to_string(),
            columns: vec![
                ColumnInfo {
                    name: "id".to_string(),
                    r#type: "INTEGER".to_string(),
                    nullable: false,
                },
                ColumnInfo {
                    name: "password_hash".to_string(),
                    r#type: "TEXT".to_string(),
                    nullable: false,
                },
                ColumnInfo {
                    name: "email".to_string(),
                    r#type: "TEXT".to_string(),
                    nullable: false,
                },
            ],
            row_count: Some(10),
            foreign_keys: None,
            samples: Some(vec![serde_json::json!({
                "id": 1,
                "password_hash": "secret123",
                "email": "user@example.com"
            })]),
        }];

        let formatted = format_schema_for_ai(&schema, &security);
        assert!(formatted.contains("id"));
        assert!(formatted.contains("email"));
        assert!(!formatted.contains("password_hash"));
    }
}
