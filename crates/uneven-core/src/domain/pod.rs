use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PodCategory {
    Database,
    Backend,
    Frontend,
    Auth,
    DevOps,
    Custom(String),
}

impl PodCategory {
    pub fn icon(&self) -> &'static str {
        match self {
            PodCategory::Database => "🗄️",
            PodCategory::Backend => "⚙️",
            PodCategory::Frontend => "🎨",
            PodCategory::Auth => "🔒",
            PodCategory::DevOps => "🚀",
            PodCategory::Custom(_) => "📦",
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            PodCategory::Database => "Database",
            PodCategory::Backend => "Backend",
            PodCategory::Frontend => "Frontend",
            PodCategory::Auth => "Auth & Security",
            PodCategory::DevOps => "DevOps & Deploy",
            PodCategory::Custom(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PodStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl PodStatus {
    pub fn label(&self) -> &'static str {
        match self {
            PodStatus::Pending => "Pending",
            PodStatus::Running => "Running",
            PodStatus::Completed => "Completed",
            PodStatus::Failed => "Failed",
        }
    }

    pub fn badge_class(&self) -> &'static str {
        match self {
            PodStatus::Pending => "bg-neutral-800 text-neutral-400 border-neutral-700",
            PodStatus::Running => "bg-cyan-950/80 text-cyan-300 border-cyan-800 animate-pulse",
            PodStatus::Completed => "bg-emerald-950/80 text-emerald-300 border-emerald-800",
            PodStatus::Failed => "bg-rose-950/80 text-rose-300 border-rose-800",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PodHandoffReport {
    pub pod_id: String,
    pub pod_title: String,
    pub prime_name: String,
    pub mission: String,
    pub status: PodStatus,
    #[serde(default)]
    pub created_or_modified_files: Vec<String>,
    #[serde(default)]
    pub exported_contracts: Vec<String>,
    #[serde(default)]
    pub advisory_notes: Vec<String>,
    pub technical_summary: String,
    #[serde(default = "default_timestamp")]
    pub completed_at: String,
}

fn default_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

impl PodHandoffReport {
    /// Formats the handoff report into a clean executive summary for the Master Agent.
    pub fn to_executive_summary(&self) -> String {
        let mut out = format!(
            "### ⚡ Pod Handoff: {} [Prime: {}]\n**Mission**: {}\n**Status**: `{:?}`\n\n",
            self.pod_title, self.prime_name, self.mission, self.status
        );

        if !self.created_or_modified_files.is_empty() {
            out.push_str("**📁 Files Affected:**\n");
            for f in &self.created_or_modified_files {
                out.push_str(&format!("- `{}`\n", f));
            }
            out.push('\n');
        }

        if !self.exported_contracts.is_empty() {
            out.push_str("**🔗 Exported Contracts & Public Interfaces:**\n");
            for c in &self.exported_contracts {
                out.push_str(&format!("- {}\n", c));
            }
            out.push('\n');
        }

        if !self.advisory_notes.is_empty() {
            out.push_str("**💡 Advisory Notes for Sibling Pods:**\n");
            for note in &self.advisory_notes {
                out.push_str(&format!("- {}\n", note));
            }
            out.push('\n');
        }

        out.push_str("**Technical Synthesis:**\n");
        out.push_str(&self.technical_summary);
        out
    }

    /// Extracts structured handoff data from a pod's inference response.
    pub fn from_response(
        pod: &AgentPod,
        mission: &str,
        response_text: &str,
        file_obj: Option<&crate::infrastructure::patch::FileObject>,
    ) -> Self {
        let mut files = Vec::new();
        if let Some(fo) = file_obj {
            for p in &fo.patch {
                if !files.contains(&p.path) {
                    files.push(p.path.clone());
                }
            }
        }

        let mut exported_contracts = Vec::new();
        let mut advisory_notes = Vec::new();
        let mut in_contracts = false;
        let mut in_advisory = false;

        for line in response_text.lines() {
            let trimmed = line.trim();
            let lower = trimmed.to_lowercase();

            if lower.contains("contract") || lower.contains("interface") || lower.contains("exported") {
                in_contracts = true;
                in_advisory = false;
                continue;
            } else if lower.contains("advisory") || lower.contains("sibling") || lower.contains("requirements for") || lower.contains("camada") {
                in_advisory = true;
                in_contracts = false;
                continue;
            } else if trimmed.starts_with('#') {
                in_contracts = false;
                in_advisory = false;
            }

            if in_contracts && (trimmed.starts_with('-') || trimmed.starts_with('*')) {
                let item = trimmed.trim_start_matches(['-', '*', ' ']).trim();
                if !item.is_empty() && item.len() < 250 {
                    exported_contracts.push(item.to_string());
                }
            } else if in_advisory && (trimmed.starts_with('-') || trimmed.starts_with('*')) {
                let item = trimmed.trim_start_matches(['-', '*', ' ']).trim();
                if !item.is_empty() && item.len() < 300 {
                    advisory_notes.push(item.to_string());
                }
            }
        }

        if files.is_empty() {
            for line in response_text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("### File:") {
                    let p = trimmed.trim_start_matches("### File:").trim().trim_matches('`').trim();
                    if !p.is_empty() && !files.contains(&p.to_string()) {
                        files.push(p.to_string());
                    }
                }
            }
        }

        Self {
            pod_id: pod.id.clone(),
            pod_title: pod.title.clone(),
            prime_name: pod.prime_name.clone(),
            mission: mission.to_string(),
            status: PodStatus::Completed,
            created_or_modified_files: files,
            exported_contracts,
            advisory_notes,
            technical_summary: response_text.to_string(),
            completed_at: default_timestamp(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPod {
    pub id: String,
    pub title: String,
    pub category: PodCategory,
    pub status: PodStatus,
    pub prime_name: String,
    pub prime_role: String,
    pub instructions: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub target_files: Vec<String>,
    #[serde(default)]
    pub multi_agent_enabled: bool,
    #[serde(default)]
    pub completed_tasks: usize,
    #[serde(default)]
    pub total_tasks: usize,
    #[serde(default)]
    pub last_handoff: Option<PodHandoffReport>,
}

impl AgentPod {
    /// Generates canonical default architecture pods for a project.
    pub fn default_project_pods() -> Vec<Self> {
        vec![
            AgentPod {
                id: "pod-database".to_string(),
                title: "Database & Data Model".to_string(),
                category: PodCategory::Database,
                status: PodStatus::Completed,
                prime_name: "Data Architect Prime".to_string(),
                prime_role: "Manages migrations, relational schemas, ORM models and queries.".to_string(),
                instructions: "Define database schemas, migration files, and table relationships with zero data-loss risk.".to_string(),
                dependencies: Vec::new(),
                target_files: vec!["migrations/".to_string(), "schema.sql".to_string()],
                multi_agent_enabled: true,
                completed_tasks: 2,
                total_tasks: 2,
                last_handoff: None,
            },
            AgentPod {
                id: "pod-backend".to_string(),
                title: "Backend API & Logic".to_string(),
                category: PodCategory::Backend,
                status: PodStatus::Running,
                prime_name: "Backend Lead Prime".to_string(),
                prime_role: "Orchestrates API endpoints, business logic services, and middleware.".to_string(),
                instructions: "Implement robust endpoints, domain services, validation logic, and consume the database schema.".to_string(),
                dependencies: vec!["pod-database".to_string()],
                target_files: vec!["src/api/".to_string(), "src/services/".to_string()],
                multi_agent_enabled: true,
                completed_tasks: 1,
                total_tasks: 3,
                last_handoff: None,
            },
            AgentPod {
                id: "pod-frontend".to_string(),
                title: "Frontend UI & Client".to_string(),
                category: PodCategory::Frontend,
                status: PodStatus::Pending,
                prime_name: "Frontend Specialist Prime".to_string(),
                prime_role: "Constructs reactive UI views, state management, and API integration.".to_string(),
                instructions: "Create intuitive, responsive user interfaces and connect them cleanly with the backend endpoints.".to_string(),
                dependencies: vec!["pod-backend".to_string()],
                target_files: vec!["src/ui/".to_string(), "src/components/".to_string()],
                multi_agent_enabled: false,
                completed_tasks: 0,
                total_tasks: 2,
                last_handoff: None,
            },
        ]
    }

    /// Creates a new custom AgentPod with inferred category and unique ID.
    pub fn create_new(
        title: &str,
        role: Option<&str>,
        target_files: Vec<String>,
        instructions: Option<&str>,
    ) -> Self {
        let id_suffix: String = uuid::Uuid::new_v4().to_string().chars().take(8).collect();
        let id = format!("pod-{}", id_suffix);
        let lower = title.to_lowercase();
        let category = if lower.contains("db") || lower.contains("database") || lower.contains("sql") || lower.contains("data") {
            PodCategory::Database
        } else if lower.contains("back") || lower.contains("api") || lower.contains("server") {
            PodCategory::Backend
        } else if lower.contains("front") || lower.contains("ui") || lower.contains("client") || lower.contains("web") {
            PodCategory::Frontend
        } else if lower.contains("auth") || lower.contains("sec") || lower.contains("login") || lower.contains("jwt") {
            PodCategory::Auth
        } else if lower.contains("devops") || lower.contains("deploy") || lower.contains("ci") || lower.contains("docker") {
            PodCategory::DevOps
        } else {
            PodCategory::Custom("Custom".to_string())
        };

        AgentPod {
            id,
            category,
            title: title.trim().to_string(),
            instructions: instructions
                .unwrap_or("Coordinate domain-specific architecture tasks.")
                .trim()
                .to_string(),
            prime_name: format!("{} Prime", title.trim()),
            prime_role: role.unwrap_or("Domain Technical Specialist").trim().to_string(),
            dependencies: Vec::new(),
            target_files,
            status: PodStatus::Pending,
            completed_tasks: 0,
            total_tasks: 3,
            multi_agent_enabled: false,
            last_handoff: None,
        }
    }

    /// Loads pods from `.uneven/pods.json` if available, or initializes default project pods.
    pub fn load_project_pods(root: &std::path::Path) -> Vec<Self> {
        let pods_file = root.join(".uneven").join("pods.json");
        if pods_file.exists()
            && let Ok(content) = std::fs::read_to_string(&pods_file)
            && let Ok(pods) = serde_json::from_str::<Vec<Self>>(&content)
            && !pods.is_empty()
        {
            return pods;
        }
        let defaults = Self::default_project_pods();
        let _ = Self::save_project_pods(root, &defaults);
        defaults
    }

    /// Persists project pods to `.uneven/pods.json`.
    pub fn save_project_pods(root: &std::path::Path, pods: &[Self]) -> std::io::Result<()> {
        let dir = root.join(".uneven");
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
        let pods_file = dir.join("pods.json");
        let json = serde_json::to_string_pretty(pods)
            .map_err(std::io::Error::other)?;
        std::fs::write(pods_file, json)
    }

    /// Extracts autonomous `[CREATE_POD: ...]` tags emitted by the AI.
    /// Format: `[CREATE_POD: Title | Role | Instructions | File1, File2]`
    pub fn parse_create_pod_tags(text: &str) -> Vec<Self> {
        let mut pods = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("[CREATE_POD:") {
                let inner = rest.trim_end_matches(']').trim();
                let parts: Vec<&str> = inner.split('|').map(|s| s.trim()).collect();
                if let Some(title) = parts.first() {
                    if title.is_empty() {
                        continue;
                    }
                    let role = parts.get(1).copied();
                    let instructions = parts.get(2).copied();
                    let target_files = parts
                        .get(3)
                        .map(|files_str| {
                            files_str
                                .split(',')
                                .map(|f| f.trim().to_string())
                                .filter(|f| !f.is_empty())
                                .collect()
                        })
                        .unwrap_or_default();

                    pods.push(Self::create_new(title, role, target_files, instructions));
                }
            }
        }
        pods
    }

    /// Autonomously processes any `[CREATE_POD: ...]` tags emitted by the AI in its response,
    /// persisting newly instantiated pods to `.uneven/pods.json`.
    pub fn process_autonomous_pod_creations(root: &std::path::Path, text: &str) -> Vec<Self> {
        let new_pods = Self::parse_create_pod_tags(text);
        if new_pods.is_empty() {
            return Vec::new();
        }

        let mut current_pods = Self::load_project_pods(root);
        let mut added = Vec::new();

        for new_p in new_pods {
            // Avoid creating duplicate pods with identical titles
            if !current_pods.iter().any(|p| p.title.eq_ignore_ascii_case(&new_p.title)) {
                current_pods.push(new_p.clone());
                added.push(new_p);
            }
        }

        if !added.is_empty() {
            let _ = Self::save_project_pods(root, &current_pods);
        }

        added
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_create_pod_tags() {
        let text = r#"
Architecture identified:
[CREATE_POD: Auth & Security | Lead Security Specialist | Manage JWT and authentication | src/auth.rs, src/jwt.rs]
[CREATE_POD: Payments | Payment Architect | Process stripe and webhooks | src/pay.rs]
Final plan ready.
"#;
        let pods = AgentPod::parse_create_pod_tags(text);
        assert_eq!(pods.len(), 2);
        assert_eq!(pods[0].title, "Auth & Security");
        assert_eq!(pods[0].prime_role, "Lead Security Specialist");
        assert_eq!(pods[0].target_files, vec!["src/auth.rs", "src/jwt.rs"]);
        assert_eq!(pods[0].category, PodCategory::Auth);

        assert_eq!(pods[1].title, "Payments");
        assert_eq!(pods[1].target_files, vec!["src/pay.rs"]);
    }

    #[test]
    fn test_process_autonomous_pod_creations() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        let text = "[CREATE_POD: Analytics | Data Analyst | Track metrics | src/metrics.rs]";
        let created = AgentPod::process_autonomous_pod_creations(root, text);
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].title, "Analytics");

        let loaded = AgentPod::load_project_pods(root);
        assert!(loaded.iter().any(|p| p.title == "Analytics"));

        // Running again should not create duplicate
        let created_again = AgentPod::process_autonomous_pod_creations(root, text);
        assert!(created_again.is_empty());
    }
}
