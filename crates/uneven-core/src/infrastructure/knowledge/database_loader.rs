use crate::UnevenError;
use sqlx::{sqlite::SqlitePool, postgres::PgPool, mysql::MySqlPool};
use redis::AsyncCommands;

use crate::infrastructure::security::SecurityContext;
use std::sync::Arc;

pub enum DbConnection {
    Sqlite(SqlitePool),
    Postgres(PgPool),
    Mysql(MySqlPool),
    Mongo(mongodb::Client),
    Redis(redis::aio::MultiplexedConnection),
}

pub struct DatabaseAnalyst {
    pub connection: Option<DbConnection>,
    security: Arc<SecurityContext>,
    pub allowed_tables: Vec<String>,
}

impl DatabaseAnalyst {
    pub fn new(security: Arc<SecurityContext>, allowed_tables: Vec<String>) -> Self {
        Self { 
            connection: None,
            security,
            allowed_tables,
        }
    }

    pub async fn connect(&mut self, url: &str) -> Result<(), UnevenError> {
        if url.starts_with("sqlite") {
            let pool = SqlitePool::connect(url).await
                .map_err(|e| UnevenError::KnowledgeError(format!("SQLite connect failed: {}", e)))?;
            self.connection = Some(DbConnection::Sqlite(pool));
        } else if url.starts_with("postgres") || url.starts_with("pg") {
            let pool = PgPool::connect(url).await
                .map_err(|e| UnevenError::KnowledgeError(format!("Postgres connect failed: {}", e)))?;
            self.connection = Some(DbConnection::Postgres(pool));
        } else if url.starts_with("mysql") {
            let pool = MySqlPool::connect(url).await
                .map_err(|e| UnevenError::KnowledgeError(format!("MySQL connect failed: {}", e)))?;
            self.connection = Some(DbConnection::Mysql(pool));
        } else if url.starts_with("mongodb") {
            let client = mongodb::Client::with_uri_str(url).await
                .map_err(|e| UnevenError::KnowledgeError(format!("MongoDB connect failed: {}", e)))?;
            self.connection = Some(DbConnection::Mongo(client));
        } else if url.starts_with("redis") {
            let client = redis::Client::open(url)
                .map_err(|e| UnevenError::KnowledgeError(format!("Redis connect failed: {}", e)))?;
            let conn = client.get_multiplexed_tokio_connection().await
                .map_err(|e| UnevenError::KnowledgeError(format!("Redis connection failed: {}", e)))?;
            self.connection = Some(DbConnection::Redis(conn));
        } else {
            return Err(UnevenError::KnowledgeError("Unsupported database scheme".to_string()));
        }
        Ok(())
    }

    pub async fn list_tables(&self) -> Result<Vec<String>, UnevenError> {
        let conn = self.connection.as_ref()
            .ok_or_else(|| UnevenError::KnowledgeError("Not connected to database".to_string()))?;

        let raw_tables: Vec<String> = match conn {
            DbConnection::Sqlite(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table'")
                    .fetch_all(pool).await
                    .map_err(|e| UnevenError::KnowledgeError(format!("SQLite list tables failed: {}", e)))?;
                rows.into_iter().map(|(n,)| n).collect()
            }
            DbConnection::Postgres(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as("SELECT table_name FROM information_schema.tables WHERE table_schema='public'")
                    .fetch_all(pool).await
                    .map_err(|e| UnevenError::KnowledgeError(format!("Postgres list tables failed: {}", e)))?;
                rows.into_iter().map(|(n,)| n).collect()
            }
            DbConnection::Mysql(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as("SHOW TABLES")
                    .fetch_all(pool).await
                    .map_err(|e| UnevenError::KnowledgeError(format!("MySQL list tables failed: {}", e)))?;
                rows.into_iter().map(|(n,)| n).collect()
            }
            DbConnection::Mongo(client) => {
                let db = client.default_database()
                    .ok_or_else(|| UnevenError::KnowledgeError("No default database in MongoDB URL".to_string()))?;
                let collections = db.list_collection_names().await
                    .map_err(|e| UnevenError::KnowledgeError(format!("MongoDB list collections failed: {}", e)))?;
                collections
            }
            DbConnection::Redis(conn) => {
                let mut conn = conn.clone();
                let keys: Vec<String> = conn.keys("*").await
                    .map_err(|e| UnevenError::KnowledgeError(format!("Redis list keys failed: {}", e)))?;
                keys
            }
        };

        let filtered: Vec<String> = raw_tables
            .into_iter()
            .filter(|t| self.allowed_tables.contains(t))
            .collect();
        Ok(filtered)
    }

    pub async fn introspect(&self, only_tables: Option<Vec<String>>) -> Result<Vec<crate::domain::database::TableSchema>, UnevenError> {
        let tables = if let Some(t) = only_tables {
            t.into_iter()
                .filter(|x| self.allowed_tables.contains(x))
                .collect()
        } else {
            self.list_tables().await?
        };
        
        let mut result = Vec::new();

        for table in tables {
            if table == "sqlite_sequence" { continue; }
            
            let columns = self.get_columns(&table).await?;
            let row_count = self.get_row_count(&table).await?;
            let foreign_keys = self.get_foreign_keys(&table).await?;
            
            let samples = if row_count > 0 && !columns.is_empty() {
                // Securely fetch samples by explicitly naming columns instead of using SELECT *
                let col_names: Vec<String> = columns.iter()
                    .map(|c| format!("\"{}\"", c.name.replace("\"", "\"\"")))
                    .collect();
                
                let sql = format!("SELECT {} FROM \"{}\" LIMIT 1", col_names.join(", "), table.replace("\"", "\"\""));
                let raw_samples = self.execute_query(&sql).await?;
                Some(raw_samples)
            } else {
                None
            };

            result.push(crate::domain::database::TableSchema {
                name: table,
                columns,
                row_count: Some(row_count),
                samples,
                foreign_keys: Some(foreign_keys),
            });
        }

        Ok(result)
    }

    async fn get_columns(&self, table: &str) -> Result<Vec<crate::domain::database::ColumnInfo>, UnevenError> {
        let conn = self.connection.as_ref()
            .ok_or_else(|| UnevenError::KnowledgeError("Not connected".to_string()))?;

        match conn {
            DbConnection::Sqlite(pool) => {
                let sql = format!("PRAGMA table_info(\"{}\")", table.replace("\"", "\"\""));
                let rows = sqlx::query(&sql).fetch_all(pool).await
                    .map_err(|e| UnevenError::KnowledgeError(format!("SQLite columns failed: {}", e)))?;
                
                use sqlx::{Row, Column};
                Ok(rows.into_iter().map(|row| {
                    /* OLD LOGIC (Keep for reference):
                    crate::domain::database::ColumnInfo {
                        name: row.get(1),
                        r#type: row.get(2),
                        nullable: row.try_get::<i32, _>(3).unwrap_or(0) == 0,
                    }
                    */
                    let get_val = |target: &str| -> String {
                        row.columns().iter()
                            .find(|c| c.name().to_lowercase() == target.to_lowercase())
                            .and_then(|c| row.try_get::<String, _>(c.ordinal()).ok())
                            .unwrap_or_default()
                    };
                    
                    crate::domain::database::ColumnInfo {
                        name: get_val("name"),
                        r#type: get_val("type"),
                        nullable: row.try_get::<i32, _>(
                            row.columns().iter()
                                .find(|c| c.name().to_lowercase() == "notnull")
                                .map(|c| c.ordinal())
                                .unwrap_or(0)
                        ).unwrap_or(0) == 0,
                    }
                }).collect())
            }
            _ => Ok(vec![])
        }
    }

    async fn get_foreign_keys(&self, table: &str) -> Result<Vec<crate::domain::database::ForeignKey>, UnevenError> {
        let conn = self.connection.as_ref()
            .ok_or_else(|| UnevenError::KnowledgeError("Not connected".to_string()))?;

        match conn {
            DbConnection::Sqlite(pool) => {
                let sql = format!("PRAGMA foreign_key_list(\"{}\")", table.replace("\"", "\"\""));
                let rows = sqlx::query(&sql).fetch_all(pool).await
                    .map_err(|e| UnevenError::KnowledgeError(format!("SQLite foreign keys failed: {}", e)))?;
                
                use sqlx::{Row, Column};
                let mut fks = Vec::new();
                for row in rows {
                    /* OLD LOGIC (Keep for reference):
                    fks.push(crate::domain::database::ForeignKey {
                        column: row.get(3),
                        referenced_table: row.get(2),
                        referenced_column: row.get(4),
                    });
                    */
                    let get_val = |target: &str| -> String {
                        row.columns().iter()
                            .find(|c| c.name().to_lowercase() == target.to_lowercase())
                            .and_then(|c| row.try_get::<String, _>(c.ordinal()).ok())
                            .unwrap_or_default()
                    };

                    let from = get_val("from");
                    let table_name = get_val("table");
                    let to = get_val("to");
                    
                    if !from.is_empty() && !table_name.is_empty() {
                         fks.push(crate::domain::database::ForeignKey {
                            column: from,
                            referenced_table: table_name,
                            referenced_column: to,
                        });
                    }
                }
                Ok(fks)
            }
            _ => Ok(vec![])
        }
    }

    async fn get_row_count(&self, table: &str) -> Result<i64, UnevenError> {
        let conn = self.connection.as_ref()
            .ok_or_else(|| UnevenError::KnowledgeError("Not connected to database".to_string()))?;

        match conn {
            DbConnection::Sqlite(pool) => {
                let sql = format!("SELECT COUNT(*) FROM \"{}\"", table.replace("\"", "\"\""));
                let row: (i64,) = sqlx::query_as(&sql).fetch_one(pool).await
                    .map_err(|e| UnevenError::KnowledgeError(format!("SQLite count failed: {}", e)))?;
                Ok(row.0)
            }
            _ => Ok(0),
        }
    }

    pub async fn execute_query(&self, sql: &str) -> Result<Vec<serde_json::Value>, UnevenError> {
        let conn = self.connection.as_ref()
            .ok_or_else(|| UnevenError::KnowledgeError("Not connected to database".to_string()))?;

        // 1. Audit SQL (Layer 2)
        self.security.audit_sql(sql)
            .map_err(UnevenError::SecurityError)?;

        // 2. Enforce Row Limit (Global security rule)
        let sql_trimmed = sql.trim().trim_end_matches(';');
        let sql_upper = sql_trimmed.to_uppercase();
        let final_sql = if !sql_upper.contains("LIMIT") {
            format!("{} LIMIT 1", sql_trimmed)
        } else {
            sql_trimmed.to_string()
        };

        // 3. Execute
        let rows: Vec<serde_json::Value> = match conn {
            DbConnection::Sqlite(pool) => {
                let sqlx_rows = sqlx::query(&final_sql).fetch_all(pool).await
                    .map_err(|e| UnevenError::KnowledgeError(format!("SQLite query failed: {}", e)))?;
                sqlx_rows.into_iter().map(|row| {
                    let mut map = serde_json::Map::new();
                    use sqlx::{Column, Row};
                    for col in row.columns() {
                        let name = col.name();
                        let val: serde_json::Value = if let Ok(s) = row.try_get::<String, _>(name) {
                            serde_json::Value::String(s)
                        } else if let Ok(n) = row.try_get::<i64, _>(name) {
                            serde_json::Value::Number(serde_json::Number::from(n))
                        } else if let Ok(f) = row.try_get::<f64, _>(name) {
                            serde_json::Value::Number(serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)))
                        } else if let Ok(b) = row.try_get::<bool, _>(name) {
                            serde_json::Value::Bool(b)
                        } else {
                            serde_json::Value::Null
                        };
                        map.insert(name.to_string(), val);
                    }
                    serde_json::Value::Object(map)
                }).collect()
            }
            DbConnection::Postgres(pool) => {
                let sqlx_rows = sqlx::query(&final_sql).fetch_all(pool).await
                    .map_err(|e| UnevenError::KnowledgeError(format!("Postgres query failed: {}", e)))?;
                sqlx_rows.into_iter().map(|row| {
                    let mut map = serde_json::Map::new();
                    use sqlx::{Column, Row};
                    for col in row.columns() {
                        let name = col.name();
                        let val: serde_json::Value = if let Ok(s) = row.try_get::<String, _>(name) {
                            serde_json::Value::String(s)
                        } else if let Ok(n) = row.try_get::<i64, _>(name) {
                            serde_json::Value::Number(serde_json::Number::from(n))
                        } else if let Ok(f) = row.try_get::<f64, _>(name) {
                            serde_json::Value::Number(serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)))
                        } else if let Ok(b) = row.try_get::<bool, _>(name) {
                            serde_json::Value::Bool(b)
                        } else {
                            serde_json::Value::Null
                        };
                        map.insert(name.to_string(), val);
                    }
                    serde_json::Value::Object(map)
                }).collect()
            }
            DbConnection::Mysql(pool) => {
                let sqlx_rows = sqlx::query(&final_sql).fetch_all(pool).await
                    .map_err(|e| UnevenError::KnowledgeError(format!("MySQL query failed: {}", e)))?;
                sqlx_rows.into_iter().map(|row| {
                    let mut map = serde_json::Map::new();
                    use sqlx::{Column, Row};
                    for col in row.columns() {
                        let name = col.name();
                        let val: serde_json::Value = if let Ok(s) = row.try_get::<String, _>(name) {
                            serde_json::Value::String(s)
                        } else if let Ok(n) = row.try_get::<i64, _>(name) {
                            serde_json::Value::Number(serde_json::Number::from(n))
                        } else if let Ok(f) = row.try_get::<f64, _>(name) {
                            serde_json::Value::Number(serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)))
                        } else if let Ok(b) = row.try_get::<bool, _>(name) {
                            serde_json::Value::Bool(b)
                        } else {
                            serde_json::Value::Null
                        };
                        map.insert(name.to_string(), val);
                    }
                    serde_json::Value::Object(map)
                }).collect()
            }
            DbConnection::Mongo(_) => {
                return Err(UnevenError::KnowledgeError("Mongo execution not fully implemented in Rust yet".to_string()));
            }
            DbConnection::Redis(_) => {
                return Err(UnevenError::KnowledgeError("Redis execution not fully implemented in Rust yet".to_string()));
            }
        };

        // 4. Mask Results (Layer 3)
        let masked_rows = rows.into_iter().map(|row| {
            if let serde_json::Value::Object(map) = row {
                let mut masked_map = serde_json::Map::new();
                for (col, val) in map {
                    let val_str = match &val {
                        serde_json::Value::String(s) => s.clone(),
                        _ => val.to_string(),
                    };
                    let masked_val = self.security.mask_value(&col, &val_str);
                    if masked_val == "***REDACTED***" {
                        masked_map.insert(col, serde_json::Value::String(masked_val));
                    } else {
                        masked_map.insert(col, val);
                    }
                }
                serde_json::Value::Object(masked_map)
            } else {
                row
            }
        }).collect();

        Ok(masked_rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_connection_and_list() {
        let security = Arc::new(SecurityContext::new(vec![], vec![], vec![]));

        let mut analyst = DatabaseAnalyst::new(security, vec!["test_table".to_string()]);
        // Use an in-memory SQLite database for testing
        analyst.connect("sqlite::memory:").await.unwrap();
        
        // Create a dummy table
        if let Some(DbConnection::Sqlite(pool)) = &analyst.connection {
            sqlx::query("CREATE TABLE test_table (id INTEGER PRIMARY KEY)").execute(pool).await.unwrap();
        }

        let tables = analyst.list_tables().await.unwrap();
        assert!(tables.contains(&"test_table".to_string()));
    }
}
