use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use uneven_core::application::think_service::{ThoughtPhase, ThoughtResult};
use uneven_core::domain::pod::AgentPod;
use uneven_core::infrastructure::patch::FileObject;

#[derive(Clone, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub id: String,
    pub role: MessageRole,
    pub text: String,
    pub files: Vec<String>,
    pub blocks: Vec<crate::components::markdown_view::MarkdownBlock>,
}

impl ChatMessage {
    pub fn new(role: MessageRole, text: impl Into<String>, files: Vec<String>) -> Self {
        let text_str = text.into();
        let blocks = crate::components::markdown_view::parse_markdown_blocks(&text_str);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role,
            text: text_str,
            files,
            blocks,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingSqlQuery {
    pub id: String,
    pub original_sql: String,
    pub current_sql: String,
    pub explanation: String,
    pub goal: String,
    pub visual_suggestions: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TelemetryData {
    pub tokens_used: usize,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub cost: f64,
    pub saved: f64,
    pub elapsed: Duration,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: PathBuf,
}

fn get_config_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("XDG_CONFIG_HOME")
        && !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    if let Ok(p) = std::env::var("HOME")
        && !p.is_empty() {
            return Some(PathBuf::from(p).join(".config"));
        }
    if let Ok(p) = std::env::var("APPDATA")
        && !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    None
}

pub fn load_recent_projects() -> Vec<ProjectInfo> {
    if let Some(cfg) = get_config_dir() {
        let file = cfg.join("uneven").join("recent_projects.json");
        if let Ok(data) = std::fs::read_to_string(&file)
            && let Ok(list) = serde_json::from_str::<Vec<ProjectInfo>>(&data) {
                return list.into_iter().filter(|p| p.path.exists()).collect();
            }
    }
    Vec::new()
}

pub fn save_recent_projects(projects: &[ProjectInfo]) {
    if let Some(cfg) = get_config_dir() {
        let dir = cfg.join("uneven");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("recent_projects.json");
        if let Ok(data) = serde_json::to_string_pretty(projects) {
            let _ = std::fs::write(file, data);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorTab {
    pub rel_path: String,
    pub content: String,
    pub is_modified: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TerminalLineKind {
    Command,
    Stdout,
    Stderr,
    System,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalLine {
    pub kind: TerminalLineKind,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalTab {
    pub id: String,
    pub name: String,
    pub current_dir: Option<PathBuf>,
    pub lines: Vec<TerminalLine>,
    pub command_history: Vec<String>,
    pub is_running: bool,
    pub current_command: Option<String>,
}

impl TerminalTab {
    pub fn new(id: impl Into<String>, name: impl Into<String>, dir: Option<PathBuf>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            current_dir: dir,
            lines: Vec::new(),
            command_history: Vec::new(),
            is_running: false,
            current_command: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalState {
    pub tabs: Vec<TerminalTab>,
    pub active_tab_index: usize,
    pub is_expanded: bool,

    // Legacy fields preserved for direct access and backward-compatible tests
    pub current_dir: Option<PathBuf>,
    pub lines: Vec<TerminalLine>,
    pub command_history: Vec<String>,
    pub is_running: bool,
    pub current_command: Option<String>,
}

impl Default for TerminalState {
    fn default() -> Self {
        let initial_tab = TerminalTab::new("term-1", "Terminal 1", None);
        Self {
            tabs: vec![initial_tab],
            active_tab_index: 0,
            is_expanded: false,
            current_dir: None,
            lines: Vec::new(),
            command_history: Vec::new(),
            is_running: false,
            current_command: None,
        }
    }
}

impl TerminalState {
    #[allow(dead_code)]
    pub fn active_tab(&self) -> Option<&TerminalTab> {
        self.tabs.get(self.active_tab_index).or_else(|| self.tabs.first())
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut TerminalTab> {
        let idx = self.active_tab_index;
        if idx < self.tabs.len() {
            self.tabs.get_mut(idx)
        } else {
            self.tabs.first_mut()
        }
    }

    pub fn add_tab(&mut self, dir: Option<PathBuf>) -> usize {
        let count = self.tabs.len() + 1;
        let id = format!("term-{}", uuid::Uuid::new_v4().to_string().chars().take(6).collect::<String>());
        let name = format!("Terminal {}", count);
        let tab = TerminalTab::new(id, name, dir.clone().or_else(|| self.current_dir.clone()));
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        self.sync_active_tab_to_legacy();
        self.active_tab_index
    }

    pub fn select_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab_index = index;
            self.sync_active_tab_to_legacy();
        }
    }

    pub fn close_tab(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.tabs.remove(index);
            if self.tabs.is_empty() {
                let tab = TerminalTab::new("term-1", "Terminal 1", self.current_dir.clone());
                self.tabs.push(tab);
                self.active_tab_index = 0;
                self.sync_active_tab_to_legacy();
                return false;
            }
            if self.active_tab_index >= self.tabs.len() {
                self.active_tab_index = self.tabs.len() - 1;
            }
            self.sync_active_tab_to_legacy();
        }
        true
    }

    pub fn toggle_expanded(&mut self) {
        self.is_expanded = !self.is_expanded;
    }

    pub fn sync_active_tab_to_legacy(&mut self) {
        if let Some(tab) = self.tabs.get(self.active_tab_index) {
            self.lines = tab.lines.clone();
            self.command_history = tab.command_history.clone();
            self.is_running = tab.is_running;
            self.current_command = tab.current_command.clone();
            if tab.current_dir.is_some() {
                self.current_dir = tab.current_dir.clone();
            }
        }
    }

    #[allow(dead_code)]
    pub fn sync_legacy_to_active_tab(&mut self) {
        let idx = self.active_tab_index;
        if let Some(tab) = self.tabs.get_mut(idx) {
            tab.lines = self.lines.clone();
            tab.command_history = self.command_history.clone();
            tab.is_running = self.is_running;
            tab.current_command = self.current_command.clone();
            if self.current_dir.is_some() {
                tab.current_dir = self.current_dir.clone();
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppState {
    pub project_root: Option<PathBuf>,
    pub active_project: Option<ProjectInfo>,
    pub recent_projects: Vec<ProjectInfo>,
    pub active_provider: String,
    pub active_model: String,
    pub thought_phase: Option<ThoughtPhase>,
    pub thought_result: Option<ThoughtResult>,
    pub pending_patch: Option<FileObject>,
    pub pending_sql: Option<PendingSqlQuery>,
    pub show_sql_inspector: bool,
    pub telemetry: TelemetryData,
    pub messages: Vec<ChatMessage>,
    pub is_reasoning: bool,
    pub multi_agent_mode: bool,
    pub active_subagent_info: Option<String>,
    pub pods: Vec<AgentPod>,
    pub active_pod_id: Option<String>,
    pub pod_histories: HashMap<String, Vec<ChatMessage>>,
    pub auto_translate_to_en: bool,
    pub show_pods: bool,
    pub sessions: Vec<String>,
    pub active_session: String,
    pub show_model_modal: bool,
    pub show_agent_dropdown: bool,
    pub show_patch_viewer: bool,
    pub attached_files: Vec<String>,
    pub open_tabs: Vec<EditorTab>,
    pub active_tab_index: usize,
    pub show_terminal: bool,
    pub terminal: TerminalState,
}

impl AppState {
    pub fn active_pod(&self) -> Option<&AgentPod> {
        let pod_id = self.active_pod_id.as_ref()?;
        self.pods.iter().find(|p| &p.id == pod_id)
    }

    pub fn active_session_key(&self) -> String {
        if let Some(ref pid) = self.active_pod_id {
            format!("{}_{}", self.active_session, pid)
        } else {
            self.active_session.clone()
        }
    }

    pub fn attach_file(&mut self, path: String) {
        if !self.attached_files.contains(&path) {
            self.attached_files.push(path);
        }
    }

    pub fn remove_attached_file(&mut self, index: usize) {
        if index < self.attached_files.len() {
            self.attached_files.remove(index);
        }
    }

    pub fn active_file_tab(&self) -> Option<&EditorTab> {
        self.open_tabs.get(self.active_tab_index)
    }

    pub fn open_file_tab(&mut self, rel_path: String) {
        if let Some(pos) = self.open_tabs.iter().position(|t| t.rel_path == rel_path) {
            self.active_tab_index = pos;
            return;
        }

        if let Some(ref root) = self.project_root {
            let full_path = root.join(&rel_path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                self.open_tabs.push(EditorTab {
                    rel_path,
                    content,
                    is_modified: false,
                });
                self.active_tab_index = self.open_tabs.len() - 1;
            }
        }
    }

    pub fn select_tab(&mut self, index: usize) {
        if index < self.open_tabs.len() {
            self.active_tab_index = index;
        }
    }

    pub fn close_tab(&mut self, index: usize) {
        if index < self.open_tabs.len() {
            self.open_tabs.remove(index);
            if self.active_tab_index >= self.open_tabs.len() && !self.open_tabs.is_empty() {
                self.active_tab_index = self.open_tabs.len() - 1;
            }
        }
    }

    pub fn close_all_tabs(&mut self) {
        self.open_tabs.clear();
        self.active_tab_index = 0;
    }

    pub fn update_active_file_content(&mut self, new_content: String) {
        if let Some(tab) = self.open_tabs.get_mut(self.active_tab_index) {
            tab.content = new_content;
            tab.is_modified = true;
        }
    }

    pub fn save_active_file(&mut self) -> Result<(), String> {
        if let Some(ref root) = self.project_root
            && let Some(tab) = self.open_tabs.get_mut(self.active_tab_index) {
                let full_path = root.join(&tab.rel_path);
                std::fs::write(&full_path, &tab.content)
                    .map_err(|e| format!("Failed to save file: {}", e))?;
                tab.is_modified = false;
                return Ok(());
            }
        Err("No active file or project root".to_string())
    }

    pub fn session_title(&self, session_id: &str) -> String {
        if let Some(ref root) = self.project_root {
            // If checking the active session with loaded in-memory messages
            if session_id == self.active_session && !self.messages.is_empty()
                && let Some(first_user) = self.messages.iter().find(|m| m.role == MessageRole::User) {
                    let first_line = first_user.text.lines().next().unwrap_or("").trim();
                    if !first_line.is_empty() {
                        let chars: String = first_line.chars().take(28).collect();
                        return if first_line.chars().count() > 28 {
                            format!("{}…", chars)
                        } else {
                            chars
                        };
                    }
                }

            // Otherwise check persisted history on disk
            if let Some(turns) =
                uneven_core::infrastructure::llm::history_manager::get_history_from_disk(session_id, root)
                && let Some(first_user) = turns.iter().find(|t| t.role == "user") {
                    let first_line = first_user.content.lines().next().unwrap_or("").trim();
                    if !first_line.is_empty() {
                        let chars: String = first_line.chars().take(28).collect();
                        return if first_line.chars().count() > 28 {
                            format!("{}…", chars)
                        } else {
                            chars
                        };
                    }
                }
        }

        if session_id == "default" {
            "Main Conversation".to_string()
        } else if let Some(stripped) = session_id.strip_prefix("session-") {
            format!("Chat #{}", stripped)
        } else {
            session_id.to_string()
        }
    }

    pub fn select_pod(&mut self, pod_id: Option<String>) {
        self.active_pod_id = pod_id;
        if let Some(ref pid) = self.active_pod_id
            && !self.pod_histories.contains_key(pid)
                && let Some(ref root) = self.project_root {
                    let key = self.active_session_key();
                    if let Some(turns) = uneven_core::infrastructure::llm::history_manager::get_history_from_disk(&key, root) {
                        let msgs = turns
                            .into_iter()
                            .map(|t| {
                                let role = if t.role == "user" { MessageRole::User } else { MessageRole::Assistant };
                                ChatMessage::new(role, t.content, Vec::new())
                            })
                            .collect();
                        self.pod_histories.insert(pid.clone(), msgs);
                    }
                }
    }

    pub fn active_messages(&self) -> Vec<ChatMessage> {
        if let Some(ref id) = self.active_pod_id {
            if let Some(msgs) = self.pod_histories.get(id) {
                return msgs.clone();
            }
            if let Some(ref root) = self.project_root {
                let key = self.active_session_key();
                if let Some(turns) = uneven_core::infrastructure::llm::history_manager::get_history_from_disk(&key, root) {
                    return turns
                        .into_iter()
                        .map(|t| {
                            let role = if t.role == "user" { MessageRole::User } else { MessageRole::Assistant };
                            ChatMessage::new(role, t.content, Vec::new())
                        })
                        .collect();
                }
            }
            Vec::new()
        } else {
            self.messages.clone()
        }
    }

    pub fn push_active_message(&mut self, mut msg: ChatMessage) {
        if msg.blocks.is_empty() && !msg.text.is_empty() {
            msg.blocks = crate::components::markdown_view::parse_markdown_blocks(&msg.text);
        }
        if let Some(ref id) = self.active_pod_id {
            self.pod_histories.entry(id.clone()).or_default().push(msg);
        } else {
            self.messages.push(msg);
        }
    }

    pub fn push_message(&mut self, role: MessageRole, text: impl Into<String>, files: Vec<String>) {
        let msg = ChatMessage::new(role, text, files);
        self.push_active_message(msg);
    }

    pub fn open_project(&mut self, path: PathBuf) {
        if !path.exists() {
            return;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_string();

        let proj = ProjectInfo {
            name: name.clone(),
            path: path.clone(),
        };

        self.project_root = Some(path.clone());
        self.active_project = Some(proj.clone());

        // Update recents list (move to top, deduplicate)
        self.recent_projects.retain(|p| p.path != path);
        self.recent_projects.insert(0, proj);
        save_recent_projects(&self.recent_projects);

        // Load project-specific configuration if present
        if let Ok(cfg) = uneven_core::infrastructure::config::loader::load_config(&path) {
            self.active_provider = cfg.brain.provider;
            self.active_model = cfg.brain.model;
        }

        // Load real sessions from project cache
        let mut sess_list = uneven_core::infrastructure::llm::history_manager::list_project_sessions(&path);
        if sess_list.is_empty() {
            let initial_id = "default".to_string();
            sess_list.push(initial_id.clone());
            let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&[], &initial_id, &path);
        }
        let current_sess = sess_list.first().cloned().unwrap_or_else(|| "default".to_string());
        self.sessions = sess_list;
        self.active_session = current_sess.clone();

        // Load messages from disk for the active session
        if let Some(turns) = uneven_core::infrastructure::llm::history_manager::get_history_from_disk(&current_sess, &path) {
            self.messages = turns
                .into_iter()
                .map(|t| {
                    let role = if t.role == "user" { MessageRole::User } else { MessageRole::Assistant };
                    ChatMessage::new(role, t.content, Vec::new())
                })
                .collect();
        } else {
            self.messages.clear();
        }

        // Reset transient pods and states
        self.pod_histories.clear();
        self.pending_patch = None;
        self.thought_phase = None;
        self.thought_result = None;
        self.active_pod_id = None;
        self.pods = AgentPod::load_project_pods(&path);

        // Point integrated terminal to open project directory
        self.terminal.current_dir = Some(path.clone());
        for tab in self.terminal.tabs.iter_mut() {
            if tab.current_dir.is_none() {
                tab.current_dir = Some(path.clone());
            }
        }
    }

    pub fn switch_session(&mut self, session_id: String) {
        if let Some(ref root) = self.project_root {
            self.active_session = session_id.clone();
            self.active_pod_id = None;
            self.pod_histories.clear();
            if let Some(turns) = uneven_core::infrastructure::llm::history_manager::get_history_from_disk(&session_id, root) {
                self.messages = turns
                    .into_iter()
                    .map(|t| {
                        let role = if t.role == "user" { MessageRole::User } else { MessageRole::Assistant };
                        ChatMessage::new(role, t.content, Vec::new())
                    })
                    .collect();
            } else {
                self.messages.clear();
            }
        }
    }

    pub fn create_new_session(&mut self) {
        if let Some(ref root) = self.project_root {
            let new_id = format!("session-{}", uuid::Uuid::new_v4().to_string().chars().take(6).collect::<String>());
            let _ = uneven_core::infrastructure::llm::history_manager::save_history_to_disk(&[], &new_id, root);
            self.sessions.insert(0, new_id.clone());
            self.active_session = new_id;
            self.active_pod_id = None;
            self.pod_histories.clear();
            self.messages.clear();
        }
    }

    pub fn delete_session(&mut self, session_id: &str) {
        if let Some(ref root) = self.project_root {
            let _ = uneven_core::infrastructure::llm::history_manager::delete_history_from_disk(session_id, root);
            self.sessions.retain(|s| s != session_id);
            if self.active_session == session_id {
                if let Some(next_id) = self.sessions.first().cloned() {
                    self.switch_session(next_id);
                } else {
                    self.create_new_session();
                }
            }
        }
    }

    pub fn close_project(&mut self) {
        self.project_root = None;
        self.active_project = None;
        self.messages.clear();
        self.sessions.clear();
        self.active_session = String::new();
        self.pod_histories.clear();
        self.pending_patch = None;
        self.show_patch_viewer = false;
        self.thought_phase = None;
        self.thought_result = None;
        self.active_pod_id = None;
        self.show_terminal = false;
        self.terminal = TerminalState::default();
    }

    pub fn append_terminal_line(&mut self, kind: TerminalLineKind, text: impl Into<String>) {
        let text_str = text.into();
        let line = TerminalLine {
            kind,
            text: text_str,
        };
        if let Some(tab) = self.terminal.active_tab_mut() {
            tab.lines.push(line.clone());
            if tab.lines.len() > 1500 {
                tab.lines.drain(0..300);
            }
        }
        self.terminal.lines.push(line);
        if self.terminal.lines.len() > 1500 {
            self.terminal.lines.drain(0..300);
        }
    }

    pub fn clear_terminal(&mut self) {
        if let Some(tab) = self.terminal.active_tab_mut() {
            tab.lines.clear();
        }
        self.terminal.lines.clear();
    }

    pub fn remove_recent_project(&mut self, path: &std::path::Path) {
        self.recent_projects.retain(|p| p.path != path);
        save_recent_projects(&self.recent_projects);
        if let Some(ref cur) = self.active_project
            && cur.path == path {
                self.close_project();
            }
    }
}

impl Default for AppState {
    fn default() -> Self {
        let recents = load_recent_projects();

        Self {
            project_root: None,
            active_project: None,
            recent_projects: recents,
            active_provider: "gemini".to_string(),
            active_model: "gemini-2.5-flash".to_string(),
            thought_phase: None,
            thought_result: None,
            pending_patch: None,
            pending_sql: None,
            show_patch_viewer: false,
            show_sql_inspector: false,
            telemetry: TelemetryData::default(),
            messages: Vec::new(),
            is_reasoning: false,
            multi_agent_mode: false,
            active_subagent_info: None,
            pods: AgentPod::default_project_pods(),
            active_pod_id: None,
            pod_histories: HashMap::new(),
            auto_translate_to_en: true,
            show_pods: false,
            sessions: vec!["session 1".to_string()],
            active_session: "session 1".to_string(),
            show_model_modal: false,
            show_agent_dropdown: false,
            attached_files: Vec::new(),
            open_tabs: Vec::new(),
            active_tab_index: 0,
            show_terminal: false,
            terminal: TerminalState::default(),
        }
    }
}

