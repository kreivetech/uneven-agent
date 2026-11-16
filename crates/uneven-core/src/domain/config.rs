use std::sync::OnceLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use catfive::{schema, string, number, Validator};
use crate::domain::UnevenError;

pub static PROJECT_ROOT: OnceLock<String> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalBrainConfig {
    pub threads: Option<u32>,
    #[serde(alias = "gpu_layers")]
    pub gpu_layers: Option<u32>,
    #[serde(alias = "unload_timeout")]
    pub unload_timeout: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrainConfig {
    pub id: Option<String>,
    pub provider: String,
    pub model: String,
    #[serde(alias = "api_key")]
    pub api_key: Option<String>,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default, alias = "max_tokens")]
    pub max_tokens: usize,
    pub local: Option<LocalBrainConfig>,
    #[serde(alias = "reasoning_effort")]
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DbConfig {
    pub url: Option<String>,
    #[serde(alias = "allowed_tables")]
    pub allowed_tables: Option<Vec<String>>,
    #[deprecated(note = "use allowed_tables instead")]
    pub tables: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeConfig {
    pub dirs: Option<Vec<String>>,
    pub files: Option<Vec<String>>,
    pub db: Option<DbConfig>,
    pub urls: Option<Vec<String>>,
    #[serde(alias = "external_project")]
    pub external_project: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BruteforceConfig {
    pub enabled: Option<bool>,
    pub endpoints: Option<Vec<String>>,
    pub wordlist: Option<String>,
    #[serde(alias = "detect_rate_limit")]
    pub detect_rate_limit: Option<bool>,
    #[serde(alias = "jwt_weak")]
    pub jwt_weak: Option<bool>,
    #[serde(alias = "session_expiry")]
    pub session_expiry: Option<bool>,
    #[serde(alias = "rate_limit")]
    pub rate_limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FirewallConfig {
    pub enabled: Option<bool>,
    #[serde(alias = "port_scan")]
    pub port_scan: Option<bool>,
    pub cors: Option<bool>,
    pub csp: Option<bool>,
    pub ssl: Option<bool>,
    pub hsts: Option<bool>,
    #[serde(alias = "x_frame_options")]
    pub x_frame_options: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveConfig {
    #[serde(alias = "sql_injection")]
    pub sql_injection: Option<bool>,
    #[serde(alias = "xss_reflected")]
    pub xss_reflected: Option<bool>,
    pub idor: Option<bool>,
    #[serde(alias = "directory_traversal")]
    pub directory_traversal: Option<bool>,
    #[serde(alias = "hidden_routes")]
    pub hidden_routes: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    pub timeout: Option<u32>,
    #[serde(alias = "follow_redirects")]
    pub follow_redirects: Option<bool>,
    #[serde(alias = "rate_limit")]
    pub rate_limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PentesterMode {
    Static,
    Active,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PentesterStaticConfig {
    pub owasp: Option<bool>,
    pub secrets: Option<bool>,
    pub dependencies: Option<bool>,
    pub injections: Option<bool>,
    pub headers: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PentesterConfig {
    pub enabled: Option<bool>,
    pub mode: Option<PentesterMode>,
    pub target: Option<String>,
    pub realtime: Option<bool>,
    pub bruteforce: Option<BruteforceConfig>,
    pub firewall: Option<FirewallConfig>,
    #[serde(rename = "static")]
    pub static_config: Option<PentesterStaticConfig>,
    pub active: Option<ActiveConfig>,
    pub network: Option<NetworkConfig>,
    pub severity: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WatchConfig {
    pub terminal: Option<String>,
    pub terminals: Option<Vec<String>>,
    #[serde(alias = "ignore_patterns")]
    pub ignore_patterns: Option<Vec<String>>,
    #[serde(alias = "auto_fix")]
    pub auto_fix: Option<bool>,
    #[serde(alias = "confirm_before_fix")]
    pub confirm_before_fix: Option<bool>,
    #[serde(alias = "alert_only")]
    pub alert_only: Option<bool>,
    pub dirs: Option<Vec<String>>,
    #[serde(alias = "only_use_strategies")]
    pub only_use_strategies: Option<bool>,
    #[serde(alias = "low_cpu_mode")]
    pub low_cpu_mode: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProactiveConfig {
    pub enabled: Option<bool>,
    #[serde(alias = "auto_fix")]
    pub auto_fix: Option<bool>,
    #[serde(alias = "confirm_before_fix")]
    pub confirm_before_fix: Option<bool>,
    pub dirs: Option<Vec<String>>,
    pub ignore: Option<Vec<String>>,
    pub vendor: Option<Vec<String>>,
    #[serde(alias = "only_use_strategies")]
    pub only_use_strategies: Option<bool>,
    #[serde(alias = "low_cpu_mode")]
    pub low_cpu_mode: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogConfig {
    pub path: Option<String>,
    pub append: Option<bool>,
    #[serde(alias = "include_timestamp")]
    pub include_timestamp: Option<bool>,
    #[serde(alias = "include_diff")]
    pub include_diff: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MinSeverity {
    Error,
    Warning,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsConfig {
    #[serde(alias = "webhook_url")]
    pub webhook_url: Option<String>,
    #[serde(alias = "min_severity")]
    pub min_severity: Option<MinSeverity>,
    #[serde(alias = "rate_limit")]
    pub rate_limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DataSecurityConfig {
    pub mask: Option<Vec<String>>,
    pub block: Option<Vec<String>>,
    #[serde(default, alias = "blocked_columns")]
    pub blocked_columns: Vec<String>,
    #[serde(default, alias = "blocked_tables")]
    pub blocked_tables: Vec<String>,
    #[serde(default, alias = "masked_columns")]
    pub masked_columns: Vec<String>,
    #[serde(alias = "report_redactions")]
    pub report_redactions: Option<bool>,
}


#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZeroDayConfig {
    pub enabled: Option<bool>,
    pub dirs: Option<Vec<String>>,
    #[serde(alias = "low_cpu_mode")]
    pub low_cpu_mode: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnevenConfig {
    #[serde(default, alias = "project_root")]
    pub project_root: String,
    pub brain: BrainConfig,
    pub brains: Option<Vec<BrainConfig>>,
    pub routing: Option<HashMap<String, String>>,
    pub knowledge: Option<KnowledgeConfig>,
    pub watch: Option<WatchConfig>,
    pub pentester: Option<PentesterConfig>,
    pub log: Option<LogConfig>,
    pub notifications: Option<NotificationsConfig>,
    #[serde(default)]
    pub security: DataSecurityConfig,
    pub proactive: Option<ProactiveConfig>,
    pub zeroday: Option<ZeroDayConfig>,
    pub skills: Option<String>,
    pub persona: Option<String>,
    #[serde(alias = "multi_agents")]
    pub multi_agents: Option<bool>,
}

impl UnevenConfig {
    pub fn get_brain_for_command(&self, command_name: &str) -> BrainConfig {
        let mut brain = self.routing.as_ref()
            .and_then(|r| r.get(command_name))
            .and_then(|bid| {
                self.brains.as_ref()
                    .and_then(|brains| {
                        brains.iter()
                            .find(|b| b.id.as_ref() == Some(bid))
                            .cloned()
                    })
            })
            .unwrap_or_else(|| self.brain.clone());

        // Optimize temperature per command for maximum precision and natural interaction
        if command_name == "chat" {
            brain.temperature = 0.7;
        } else {
            brain.temperature = 0.2;
        }

        let real_cap = crate::infrastructure::llm::get_default_max_output_tokens(&brain.provider, &brain.model);
        if brain.max_tokens <= 2048 {
            brain.max_tokens = real_cap;
        } else {
            brain.max_tokens = brain.max_tokens.max(real_cap);
        }
        
        brain
    }

    pub fn validate_schema(&self) -> Result<(), UnevenError> {
        let brain_schema = schema::<BrainConfig>()
            .field("provider", |b| &b.provider, string().min(1))
            .field("model", |b| &b.model, string().min(1))
            .field("temperature", |b| &b.temperature, number().min(0.0).max(2.0));

        brain_schema.validate(&self.brain)
            .map_err(|e| UnevenError::ConfigError(format!("Invalid brain config: {:?}", e)))?;

        if let Some(ref brains) = self.brains {
            for b in brains {
                brain_schema.validate(b)
                    .map_err(|e| UnevenError::ConfigError(format!("Invalid secondary brain config: {:?}", e)))?;
            }
        }

        if let Some(url) = self.notifications.as_ref().and_then(|n| n.webhook_url.as_ref()) {
            string().url().validate(url)
                .map_err(|e| UnevenError::ConfigError(format!("Invalid notifications webhook_url: {:?}", e)))?;
        }

        if let Some(urls) = self.knowledge.as_ref().and_then(|k| k.urls.as_ref()) {
            let url_val = string().url();
            for u in urls {
                url_val.validate(u)
                    .map_err(|e| UnevenError::ConfigError(format!("Invalid knowledge url '{}': {:?}", u, e)))?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uneven_config_schema_validation() {
        let mut config = UnevenConfig::default();
        config.brain.provider = "gemini".to_string();
        config.brain.model = "gemini-2.5-pro".to_string();
        config.brain.temperature = 0.5;

        assert!(config.validate_schema().is_ok());

        // Invalid temperature out of bounds
        config.brain.temperature = 5.0;
        assert!(config.validate_schema().is_err());

        // Invalid empty provider
        config.brain.temperature = 0.5;
        config.brain.provider = "".to_string();
        assert!(config.validate_schema().is_err());

        // Invalid webhook url
        config.brain.provider = "gemini".to_string();
        config.notifications = Some(NotificationsConfig {
            webhook_url: Some("not-a-valid-url".to_string()),
            ..Default::default()
        });
        assert!(config.validate_schema().is_err());

        // Valid webhook url
        config.notifications = Some(NotificationsConfig {
            webhook_url: Some("https://hooks.slack.com/services/test".to_string()),
            ..Default::default()
        });
        assert!(config.validate_schema().is_ok());
    }
}
