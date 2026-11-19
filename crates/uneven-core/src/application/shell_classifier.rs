use serde::{Deserialize, Serialize};
use crate::domain::actions::{extract_action_response, ShellActionResponse};
use crate::domain::config::PentesterMode;
use crate::infrastructure::llm::history_manager::{prepare_structured_prompt, BrainTurn};
use crate::infrastructure::llm::{infer_with_params, InferParams};
use crate::infrastructure::security::laws;
use crate::UnevenConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellAction {
    Index,
    Scan,
    Pentest,
    Zeroday,
    Analyze,
    Watch,
    Ask,
    Think,
    Askf,
    Explain,
    Docs,
    Log,
    Undo,
    Help,
    Exit,
    Stats,
    None,
    Unknown,
    RunCmd,
    MultiAgents,
    CreatePod,
    DispatchPod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedIntent {
    pub action: ShellAction,
    pub file: Option<String>,
    pub task: Option<String>,
    pub question: Option<String>,
    pub commit: Option<String>,
    pub from: Option<String>,
    pub staged: bool,
    pub confidence: String,
    pub base: Option<String>,
    pub paths: Vec<String>,
    pub report: bool,
    pub tasks: Option<Vec<String>>,
}

impl ClassifiedIntent {
    /// Returns true if the intent specifies deep analysis mode (e.g., `--deep` or `deep`).
    pub fn is_deep(&self) -> bool {
        self.task
            .as_deref()
            .map(|t| t.split_whitespace().any(|w| w == "deep" || w == "--deep"))
            .unwrap_or(false)
    }

    /// Returns true if the intent specifies active dynamic pentesting mode.
    pub fn is_active_pentest(&self) -> bool {
        self.task
            .as_deref()
            .map(|t| t.split_whitespace().any(|w| w == "active" || w == "--active"))
            .unwrap_or(false)
    }

    /// Resolves the PentesterMode according to active/static intent flags.
    pub fn pentest_mode(&self) -> PentesterMode {
        if self.is_active_pentest() {
            PentesterMode::Active
        } else {
            PentesterMode::Static
        }
    }

    /// Returns the target directory or path stripped of flags like `--deep` or `--active`.
    pub fn clean_target_dir(&self) -> Option<String> {
        let raw = self.task.as_deref().or(self.file.as_deref())?;
        let tokens: Vec<&str> = raw
            .split_whitespace()
            .filter(|&w| w != "--deep" && w != "deep" && w != "--active" && w != "active")
            .collect();
        let cleaned = tokens.join(" ").trim().to_string();
        if cleaned.is_empty() || cleaned.starts_with("--") {
            None
        } else {
            Some(cleaned)
        }
    }

    /// Extracts the target pod and mission specification for pod dispatch.
    /// Format: `[pod_name] | [mission_statement]` or defaulting to "database" and whole input.
    pub fn pod_dispatch_spec(&self, fallback: &str) -> (String, String) {
        let raw = self.task.as_deref().unwrap_or(fallback).trim();
        if let Some((target, mission)) = raw.split_once('|') {
            let t = target.trim();
            let m = mission.trim();
            (
                if t.is_empty() { "database".to_string() } else { t.to_string() },
                if m.is_empty() { raw.to_string() } else { m.to_string() },
            )
        } else {
            ("database".to_string(), raw.to_string())
        }
    }
}

pub fn tokenize_shell_input(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_double_quotes = false;
    let mut in_single_quotes = false;
    let mut escaped = false;

    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' && !in_single_quotes {
            in_double_quotes = !in_double_quotes;
        } else if c == '\'' && !in_double_quotes {
            in_single_quotes = !in_single_quotes;
        } else if c.is_whitespace() && !in_double_quotes && !in_single_quotes {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
        i += 1;
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

pub fn extract_file_path(input: &str) -> Option<String> {
    let re = regex::Regex::new(r"^(\.{1,2}/)?([\w.-]+/)*[\w.-]+\.[\w]{1,6}$").ok()?;

    for word in input.split_whitespace() {
        let clean_word =
            word.trim_matches(|c| c == '?' || c == '!' || c == ',' || c == '.' || c == ';');
        if re.is_match(clean_word) {
            return Some(clean_word.to_string());
        }
    }
    None
}

/// Returns true if the user's input expresses intent to confirm / approve / apply pending patches.
pub fn is_approval_intent(input: &str) -> bool {
    let raw = input.trim().to_lowercase();
    let cleaned: String = raw.chars().filter(|c| !c.is_ascii_punctuation()).collect();
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    if words.is_empty() {
        return false;
    }

    let first = words[0];
    if (words.len() == 1 && first == "askf")
        || matches!(
            first,
            "apply"
                | "aplica"
                | "aplicar"
                | "aplique"
                | "aprova"
                | "aprovar"
                | "aprove"
                | "aprovei"
                | "aprovado"
                | "confirma"
                | "confirmar"
                | "confirme"
                | "confirmo"
                | "confirmado"
                | "sim"
                | "yes"
                | "ok"
                | "okay"
        )
    {
        return true;
    }

    let joined = words.join(" ");
    if matches!(
        joined.as_str(),
        "pode aplicar"
            | "pode fazer"
            | "pode rodar"
            | "pode salvar"
            | "manda bala"
            | "vai em frente"
            | "prossiga"
            | "prosseguir"
            | "executar"
            | "execute"
            | "executa"
            | "tudo certo"
            | "tudo ok"
            | "de acordo"
            | "concordo"
    ) || joined.starts_with("pode aplicar")
        || joined.starts_with("aplica o patch")
        || joined.starts_with("aplica as alteracoes")
        || joined.starts_with("aplicar alteracoes")
        || joined.starts_with("salvar alteracoes")
        || joined.starts_with("aprovei")
    {
        return true;
    }

    false
}

pub fn classify_intent(raw: &str) -> ClassifiedIntent {
    let input = raw.trim();
    if input.is_empty() {
        return ClassifiedIntent {
            action: ShellAction::Unknown,
            file: None,
            task: None,
            question: None,
            commit: None,
            from: None,
            staged: false,
            confidence: "low".to_string(),
            base: None,
            paths: Vec::new(),
            report: false,
            tasks: None,
        };
    }

    let tokens = tokenize_shell_input(input);
    let first = tokens.first().map(|s| s.to_lowercase()).unwrap_or_default();

    let mut intent = ClassifiedIntent {
        action: ShellAction::Unknown,
        file: None,
        task: None,
        question: None,
        commit: None,
        from: None,
        staged: false,
        confidence: "low".to_string(),
        base: None,
        paths: Vec::new(),
        report: false,
        tasks: None,
    };

    if first == "exit" || first == "quit" || first == "bye" {
        intent.action = ShellAction::Exit;
        intent.confidence = "high".to_string();
        return intent;
    }
    if first == "help" {
        intent.action = ShellAction::Help;
        intent.confidence = "high".to_string();
        return intent;
    }
    if first == "log" {
        intent.action = ShellAction::Log;
        intent.confidence = "high".to_string();
        return intent;
    }
    if first == "undo" {
        intent.action = ShellAction::Undo;
        intent.confidence = "high".to_string();
        return intent;
    }
    if first == "index" {
        intent.action = ShellAction::Index;
        intent.confidence = "high".to_string();
        return intent;
    }
    if first == "scan" || first == "/scan" {
        intent.action = ShellAction::Scan;
        intent.confidence = "high".to_string();
        let payload = tokens[1..].join(" ");
        if !payload.is_empty() {
            intent.file = Some(payload);
        }
        return intent;
    }
    if first == "pentest" || first == "/pentest" {
        intent.action = ShellAction::Pentest;
        intent.confidence = "high".to_string();
        let payload = tokens[1..].join(" ");
        if !payload.is_empty() {
            intent.task = Some(payload);
        }
        return intent;
    }
    if first == "zeroday" || first == "zero-day" || first == "/zeroday" {
        intent.action = ShellAction::Zeroday;
        intent.confidence = "high".to_string();
        let payload = tokens[1..].join(" ");
        if !payload.is_empty() {
            intent.task = Some(payload);
        }
        return intent;
    }
    if first == "analyze" || first == "/analyze" {
        intent.action = ShellAction::Analyze;
        intent.confidence = "high".to_string();
        let payload = tokens[1..].join(" ");
        if !payload.is_empty() {
            intent.task = Some(payload);
        }
        return intent;
    }
    if first == "watch" {
        intent.action = ShellAction::Watch;
        intent.confidence = "high".to_string();
        return intent;
    }
    if first == "stats" || first == "cost" {
        intent.action = ShellAction::Stats;
        intent.confidence = "high".to_string();
        return intent;
    }
    if first == "create_pod" || first == "pod" || (first == "new" && tokens.get(1).map(|s| s.as_str()) == Some("pod")) {
        intent.action = ShellAction::CreatePod;
        intent.confidence = "high".to_string();
        let payload = if first == "new" {
            tokens[2..].join(" ")
        } else {
            tokens[1..].join(" ")
        };
        intent.task = Some(payload);
        return intent;
    }

    if first == "review" {
        intent.action = ShellAction::Think;
        intent.confidence = "high".to_string();
        let payload = tokens[1..].join(" ");
        intent.task = if payload.is_empty() {
            Some("review changes".to_string())
        } else {
            Some(payload)
        };
        return intent;
    }

    if first == "docs" {
        intent.action = ShellAction::Docs;
        intent.confidence = "high".to_string();
        intent.file = extract_file_path(input);
        return intent;
    }

    if first == "explain" {
        intent.action = ShellAction::Explain;
        intent.confidence = "high".to_string();
        intent.file = extract_file_path(input);
        return intent;
    }

    if is_approval_intent(input) {
        intent.action = ShellAction::Askf;
        intent.confidence = "high".to_string();
        intent.task = Some(input.to_string());
        return intent;
    }

    if first == "askf" {
        intent.action = ShellAction::Askf;
        intent.confidence = "high".to_string();
        let task_content = input.strip_prefix(&tokens[0]).unwrap_or(input).trim().to_string();
        intent.task = Some(task_content);
        return intent;
    }

    if first == "think" || first == "plan" || first == "ask" {
        intent.action = ShellAction::Think;
        intent.confidence = "high".to_string();
        let task_content = input.strip_prefix(&tokens[0]).unwrap_or(input).trim().to_string();
        intent.task = Some(task_content);
        return intent;
    }

    let file = extract_file_path(input);
    if let Some(ref f) = file
        && input == f
    {
        intent.action = ShellAction::Explain;
        intent.confidence = "high".to_string();
        intent.file = Some(f.clone());
        return intent;
    }

    intent.action = ShellAction::Think;
    intent.confidence = "high".to_string();
    intent.task = Some(input.to_string());

    intent
}

pub fn json_to_intent(parsed: &ShellActionResponse) -> ClassifiedIntent {
    let command = &parsed.command;
    let args = &parsed.args;

    let mut intent = ClassifiedIntent {
        action: ShellAction::Unknown,
        file: None,
        task: None,
        question: None,
        commit: None,
        from: None,
        staged: false,
        confidence: "high".to_string(),
        base: None,
        paths: parsed.paths.clone(),
        report: false,
        tasks: None,
    };

    match command.as_str() {
        "index" => intent.action = ShellAction::Index,
        "scan" => intent.action = ShellAction::Scan,
        "pentest" => {
            intent.action = ShellAction::Pentest;
            intent.task = args.first().cloned();
        }
        "zeroday" | "zero-day" | "zero_day" => {
            intent.action = ShellAction::Zeroday;
            intent.task = args.first().cloned();
        }
        "analyze" | "bi" | "query" => {
            intent.action = ShellAction::Analyze;
            intent.task = args.first().cloned();
        }
        "watch" => intent.action = ShellAction::Watch,
        "log" => intent.action = ShellAction::Log,
        "undo" => intent.action = ShellAction::Undo,
        "help" => intent.action = ShellAction::Help,
        "exit" => intent.action = ShellAction::Exit,
        "stats" | "cost" => intent.action = ShellAction::Stats,
        "none" => intent.action = ShellAction::None,
        "explain" => {
            intent.action = ShellAction::Explain;
            intent.file = args.first().cloned();
        }
        "docs" => {
            intent.action = ShellAction::Docs;
            intent.file = args.first().cloned();
        }
        "dde" => {
            intent.action = ShellAction::Askf;
            intent.task = args.first().cloned();
        }
        "create_pod" | "new_pod" => {
            intent.action = ShellAction::CreatePod;
            intent.task = args.first().cloned();
        }
        "dispatch_pod" | "run_pod" | "order_pod" => {
            intent.action = ShellAction::DispatchPod;
            intent.task = args.first().cloned();
        }
        "think" | "plan" | "ask" | "review" => {
            intent.action = ShellAction::Think;
            intent.task = args.first().cloned();
        }
        "multi_agents" | "multi-agents" | "multi_agent" => {
            intent.action = ShellAction::MultiAgents;
            intent.tasks = Some(args.clone());
        }
        "run_cmd" | "run" | "exec" => {
            intent.action = ShellAction::RunCmd;
            intent.question = args.first().cloned();
        }
        _ => {
            intent.action = ShellAction::Unknown;
            intent.confidence = "low".to_string();
        }
    }
    intent
}

pub async fn classify_with_brain(
    input: &str,
    history: &[BrainTurn],
    files_map: &str,
    config: &UnevenConfig,
    external_files: Option<&[String]>,
) -> Option<(ClassifiedIntent, String, usize)> {
    let mut system_instructions = laws::get_context("shell");
    if config.multi_agents.unwrap_or(false) {
        system_instructions = format!(
            "{}\n\n## Multi-Agent Mode is ACTIVE\n\
            - You MUST delegate complex tasks involving multiple logical steps or multiple files to the `multi_agents` command.\n\
            - Output format: {{\"command\": \"multi_agents\", \"args\": [\"Detailed subtask 1 description in English\", \"Detailed subtask 2 description in English\", ...], \"paths\": [], \"message\": \"\"}}\n\
            - Break the user request down into smaller, atomic, sequential or independent subtasks. Each subtask must be specific, focused, and fully describable in English.\n\
            - IMPORTANT: Subagents are executed SEQUENTIALLY, and their file changes are written to disk immediately. Each subsequent agent will scan the updated codebase and see the files created or modified by preceding agents. Therefore, you must order subtasks logically by dependency (e.g., create base interfaces, models, or services BEFORE creating controllers or routes that depend on them).\n\
            - Do NOT suggest `apply` directly for complex multi-file or multi-step requests when Multi-Agent Mode is active. Use `multi_agents` instead.",
            system_instructions
        );
    }
    if let Some(ref ext_project_path) = config.knowledge.as_ref().and_then(|k| k.external_project.as_ref()) {
        let ext_folder_name = std::path::Path::new(ext_project_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(ext_project_path);
        system_instructions = format!(
            "{}\n\nAttached Repositories: [ {} ]",
            system_instructions, ext_folder_name
        );
    }

    let mut prompt_lines = Vec::new();

    // Inject external files map under [EXTERNAL_FILES_MAP] tag only when the query references the external project
    if let Some(ref ext_project_path) = config.knowledge.as_ref().and_then(|k| k.external_project.as_ref())
        && let Some(ext_folder_name) = std::path::Path::new(ext_project_path).file_name().and_then(|n| n.to_str())
    {
        let input_lower = input.to_lowercase();
        let folder_lower = ext_folder_name.to_lowercase();
        let references_external = input_lower.contains(&folder_lower)
            || input.contains("../")
            || input.contains(r"..\");

        if references_external
            && let Some(files) = external_files
            && !files.is_empty()
        {
            prompt_lines.push("[EXTERNAL_FILES_MAP]".to_string());
            prompt_lines.push(files.join("\n"));
            prompt_lines.push("[/EXTERNAL_FILES_MAP]".to_string());
            prompt_lines.push("".to_string());
        }
    }

    // Add current input to prompt
    prompt_lines.push(input.to_string());
    let raw_prompt = prompt_lines.join("\n");

    let map_opt = if files_map.is_empty() {
        None
    } else {
        Some(files_map)
    };
    let prepared = prepare_structured_prompt(history, &raw_prompt, &system_instructions, map_opt);

    let threads = config.brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    let max_context = if config.brain.provider == "local" {
        8000
    } else {
        1000000
    };
    let params = InferParams {
        model: config.brain.model.clone(),
        threads,
        max_tokens: config.brain.max_tokens,
        temperature: 0.1,
        repeat_penalty: 1.1,
        max_context,
        grammar: None,
        provider: Some(config.brain.provider.clone()),
        system: prepared.system.or(Some(system_instructions)),
        history: if prepared.history.is_empty() {
            None
        } else {
            Some(prepared.history)
        },
    };

    // Call LLM API
    let res = infer_with_params(&prepared.prompt, &params).await.ok()?;
    let parsed = extract_action_response(&res.content)?;
    let intent = json_to_intent(&parsed);
    Some((intent, parsed.message, res.tokens_used))
}

pub async fn classify_shell_intent(
    input: &str,
    history: &[BrainTurn],
    files_map: &str,
    config: &UnevenConfig,
    external_files: Option<&[String]>,
) -> (ClassifiedIntent, String, usize) {
    let quick = classify_intent(input);
    let is_utility = matches!(
        quick.action,
        ShellAction::Exit
            | ShellAction::Help
            | ShellAction::Log
            | ShellAction::Undo
            | ShellAction::Index
            | ShellAction::Scan
            | ShellAction::Pentest
            | ShellAction::Zeroday
            | ShellAction::Analyze
            | ShellAction::Watch
            | ShellAction::Askf
    );

    if quick.confidence == "high" && is_utility {
        return (quick, String::new(), 0);
    }

    if config.brain.provider != "local"
        && !config.brain.provider.is_empty()
        && let Some((mut intent, message, tokens)) =
            classify_with_brain(input, history, files_map, config, external_files).await
    {
        if intent.action == ShellAction::Unknown || intent.action == ShellAction::None || intent.action == ShellAction::Ask {
            intent.action = ShellAction::Think;
            intent.task = Some(input.to_string());
        }
        return (intent, message, tokens);
    }

    let final_intent = if quick.action == ShellAction::Unknown || quick.action == ShellAction::Ask {
        ClassifiedIntent {
            action: ShellAction::Think,
            task: Some(input.to_string()),
            confidence: "high".to_string(),
            file: None,
            question: None,
            commit: None,
            from: None,
            staged: false,
            base: None,
            paths: Vec::new(),
            report: false,
            tasks: None,
        }
    } else {
        quick
    };

    (final_intent, String::new(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_intent_pentest() {
        let intent = classify_intent("pentest");
        assert_eq!(intent.action, ShellAction::Pentest);

        let intent_active = classify_intent("pentest active");
        assert_eq!(intent_active.action, ShellAction::Pentest);
        assert_eq!(intent_active.task.as_deref(), Some("active"));
    }

    #[test]
    fn test_classify_intent_zeroday() {
        let intent = classify_intent("zeroday");
        assert_eq!(intent.action, ShellAction::Zeroday);

        let intent_slash = classify_intent("/zeroday --report html");
        assert_eq!(intent_slash.action, ShellAction::Zeroday);
        assert_eq!(intent_slash.task.as_deref(), Some("--report html"));
    }

    #[test]
    fn test_is_approval_intent() {
        assert!(is_approval_intent("aprovei"));
        assert!(is_approval_intent("aprovei pra ver oque acontecia"));
        assert!(is_approval_intent("sim"));
        assert!(is_approval_intent("pode aplicar"));
        assert!(is_approval_intent("aplica"));
        assert!(is_approval_intent("aplica as alteracoes"));
        assert!(is_approval_intent("ok"));
        assert!(is_approval_intent("confirmar"));
        assert!(is_approval_intent("askf"));
        assert!(is_approval_intent("yes"));

        assert!(!is_approval_intent("qual a diferenca entre rust e go?"));
        assert!(!is_approval_intent("como funciona o banco?"));
    }

    #[test]
    fn test_classify_intent_approval() {
        let intent = classify_intent("aprovei");
        assert_eq!(intent.action, ShellAction::Askf);
        assert_eq!(intent.confidence, "high");

        let intent2 = classify_intent("pode aplicar");
        assert_eq!(intent2.action, ShellAction::Askf);
    }

    #[test]
    fn test_classified_intent_helpers() {
        let intent = classify_intent("pentest active");
        assert!(intent.is_active_pentest());
        assert_eq!(intent.pentest_mode(), PentesterMode::Active);
        assert_eq!(intent.clean_target_dir(), None);

        let intent_pentest_static = classify_intent("pentest");
        assert!(!intent_pentest_static.is_active_pentest());
        assert_eq!(intent_pentest_static.pentest_mode(), PentesterMode::Static);

        let intent_zd_deep = classify_intent("zeroday --deep src/");
        assert!(intent_zd_deep.is_deep());
        assert_eq!(intent_zd_deep.clean_target_dir(), Some("src/".to_string()));

        let intent_zd_plain = classify_intent("zeroday crates/uneven-core");
        assert!(!intent_zd_plain.is_deep());
        assert_eq!(intent_zd_plain.clean_target_dir(), Some("crates/uneven-core".to_string()));

        let intent_pod = ClassifiedIntent {
            action: ShellAction::DispatchPod,
            file: None,
            task: Some("backend | optimize sql queries".to_string()),
            question: None,
            commit: None,
            from: None,
            staged: false,
            confidence: "high".to_string(),
            base: None,
            paths: Vec::new(),
            report: false,
            tasks: None,
        };
        let (target, mission) = intent_pod.pod_dispatch_spec("fallback");
        assert_eq!(target, "backend");
        assert_eq!(mission, "optimize sql queries");

        let intent_pod_no_pipe = ClassifiedIntent {
            action: ShellAction::DispatchPod,
            file: None,
            task: Some("optimize sql queries".to_string()),
            question: None,
            commit: None,
            from: None,
            staged: false,
            confidence: "high".to_string(),
            base: None,
            paths: Vec::new(),
            report: false,
            tasks: None,
        };
        let (target, mission) = intent_pod_no_pipe.pod_dispatch_spec("fallback");
        assert_eq!(target, "database");
        assert_eq!(mission, "optimize sql queries");
    }
}
