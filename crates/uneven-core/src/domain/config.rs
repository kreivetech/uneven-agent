use std::sync::OnceLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
pub struct RemoteShellHttpsConfig {
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteShellRateLimit {
    #[serde(alias = "requests_per_minute")]
    pub requests_per_minute: Option<u32>,
    pub burst: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteShellCors {
    #[serde(alias = "allowed_origins")]
    pub allowed_origins: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteShellConfig {
    pub port: u16,
    pub secret: Option<String>,
    pub https: Option<RemoteShellHttpsConfig>,
    #[serde(alias = "rate_limit")]
    pub rate_limit: Option<RemoteShellRateLimit>,
    pub cors: Option<RemoteShellCors>,
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
    #[serde(alias = "remote_shell")]
    pub remote_shell: Option<RemoteShellConfig>,
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
        
        brain
    }
}
