use anyhow::Result;
use console::style;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use indicatif::{ProgressBar, ProgressStyle};
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::security::laws;
use uneven_core::infrastructure::security::SafetyGuard;
use uneven_core::infrastructure::llm::inference::chat_template::{
    detect_template, apply_template, ChatTurn,
};
use uneven_core::infrastructure::llm::{
    InferParams, init as init_llm, infer_with_params_streaming,
};
use uneven_core::infrastructure::knowledge::scanner::ProjectScanner;
use uneven_core::UnevenConfig;
use crate::ui::{self, truecolor, truecolor_bold};
use super::explain;
use super::docs;
use super::askf;
use super::review;
use super::resource_guardian::ResourceGuardian;
use super::context_assembler::ContextAssembler;


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrainTurn {
    pub role: String,
    pub content: String,
}

impl uneven_core::infrastructure::llm::history_manager::HistoryItem for BrainTurn {
    fn get_role(&self) -> &str { &self.role }
    fn get_content(&self) -> &str { &self.content }
    fn set_content(&mut self, content: String) { self.content = content; }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShellAction {
    Index,
    Scan,
    Watch,
    Ask,
    Askf,
    Explain,
    Review,
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
}

static SUBPROCESS_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone)]
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

#[derive(serde::Deserialize, Debug)]
struct BrainResponse {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    message: String,
    #[serde(default)]
    paths: Vec<String>,
}

fn tokenize_shell_input(input: &str) -> Vec<String> {
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

fn parse_analyze_input(input_after_analyze: &str) -> Vec<String> {
    let raw_tokens = tokenize_shell_input(input_after_analyze);
    let mut args = vec!["analyze".to_string()];
    let mut prompt_words = Vec::new();

    let mut i = 0;
    while i < raw_tokens.len() {
        let token = &raw_tokens[i];
        if token == "--db" || token == "--report" || token == "--theme" || token == "--provider" || token == "--model" || token == "--prompt" {
            // Flag with argument
            args.push(token.clone());
            if i + 1 < raw_tokens.len() {
                args.push(raw_tokens[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        } else if token == "--package-exe" || token == "--clear-cache" {
            // Flag without argument
            args.push(token.clone());
            i += 1;
        } else {
            // Word belonging to the prompt
            prompt_words.push(token.clone());
            i += 1;
        }
    }

    if !prompt_words.is_empty() {
        let prompt_str = prompt_words.join(" ");
        args.push("--prompt".to_string());
        args.push(prompt_str);
    }

    args
}

fn extract_file_path(input: &str) -> Option<String> {
    let re = regex::Regex::new(
        r"^(\.{1,2}/)?([\w.-]+/)*[\w.-]+\.[\w]{1,6}$"
    ).ok()?;

    for word in input.split_whitespace() {
        let clean_word = word.trim_matches(|c| c == '?' || c == '!' || c == ',' || c == '.' || c == ';');
        if re.is_match(clean_word) {
            return Some(clean_word.to_string());
        }
    }
    None
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
    if first == "scan" {
        intent.action = ShellAction::Scan;
        intent.confidence = "high".to_string();
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

    if first == "review" {
        intent.action = ShellAction::Review;
        intent.confidence = "high".to_string();
        let mut i = 1;
        while i < tokens.len() {
            let tok = &tokens[i];
            if tok == "--commit" && i + 1 < tokens.len() {
                intent.commit = Some(tokens[i + 1].clone());
                i += 2;
            } else if tok == "--from" && i + 1 < tokens.len() {
                intent.from = Some(tokens[i + 1].clone());
                i += 2;
            } else if tok == "--base" && i + 1 < tokens.len() {
                intent.base = Some(tokens[i + 1].clone());
                i += 2;
            } else if tok == "--staged" {
                intent.staged = true;
                i += 1;
            } else if tok == "-r" || tok == "--report" {
                intent.report = true;
                i += 1;
            } else if tok == "--" {
                i += 1;
                while i < tokens.len() {
                    intent.paths.push(tokens[i].clone());
                    i += 1;
                }
            } else {
                if !tok.starts_with('-') {
                    intent.paths.push(tok.clone());
                }
                i += 1;
            }
        }
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

    if first == "askf" {
        intent.action = ShellAction::Askf;
        intent.confidence = "high".to_string();
        let task_content = input.strip_prefix(&tokens[0]).unwrap_or(input).trim().to_string();
        intent.task = Some(task_content);
        return intent;
    }

    let file = extract_file_path(input);
    if let Some(ref f) = file {
        if input == f {
            intent.action = ShellAction::Explain;
            intent.confidence = "high".to_string();
            intent.file = Some(f.clone());
            return intent;
        }
    }

    intent.action = ShellAction::Ask;
    intent.confidence = "high".to_string();
    intent.question = Some(input.to_string());

    intent
}

fn extract_json(text: &str) -> Option<BrainResponse> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if start > end {
        return None;
    }
    let json_str = &text[start..=end];
    serde_json::from_str(json_str).ok()
}

fn json_to_intent(parsed: &BrainResponse) -> ClassifiedIntent {
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
        "watch" => intent.action = ShellAction::Watch,
        "log" => intent.action = ShellAction::Log,
        "undo" => intent.action = ShellAction::Undo,
        "help" => intent.action = ShellAction::Help,
        "exit" => intent.action = ShellAction::Exit,
        "stats" => intent.action = ShellAction::Stats,
        "cost" => intent.action = ShellAction::Stats,
        "none" => intent.action = ShellAction::None,
        "explain" => {
            intent.action = ShellAction::Explain;
            intent.file = args.first().cloned();
        }
        "docs" => {
            intent.action = ShellAction::Docs;
            intent.file = args.first().cloned();
        }
        "askf" => {
            intent.action = ShellAction::Askf;
            intent.task = args.first().cloned();
        }
        "multi_agents" | "multi-agents" | "multi_agent" => {
            intent.action = ShellAction::MultiAgents;
            intent.tasks = Some(args.clone());
        }
        "ask" => {
            intent.action = ShellAction::Ask;
            intent.question = args.first().cloned();
        }
        "review" => {
            intent.action = ShellAction::Review;
            let mut i = 0;
            while i < args.len() {
                if args[i] == "--commit" && i + 1 < args.len() {
                    intent.commit = Some(args[i + 1].clone());
                    i += 2;
                } else if args[i] == "--from" && i + 1 < args.len() {
                    intent.from = Some(args[i + 1].clone());
                    i += 2;
                } else if args[i] == "--base" && i + 1 < args.len() {
                    intent.base = Some(args[i + 1].clone());
                    i += 2;
                } else if args[i] == "--staged" {
                    intent.staged = true;
                    i += 1;
                } else if args[i] == "-r" || args[i] == "--report" {
                    intent.report = true;
                    i += 1;
                } else if args[i] == "--" {
                    i += 1;
                    while i < args.len() {
                        intent.paths.push(args[i].clone());
                        i += 1;
                    }
                } else {
                    if !args[i].starts_with('-') {
                        intent.paths.push(args[i].clone());
                    }
                    i += 1;
                }
            }
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

async fn classify_with_brain(
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
            - Do NOT suggest `askf` directly for complex multi-file or multi-step requests when Multi-Agent Mode is active. Use `multi_agents` instead.",
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
            system_instructions,
            ext_folder_name
        );
    }

    let mut prompt_lines = Vec::new();
    if !files_map.is_empty() {
        prompt_lines.push("PROJECT FILES MAP (Relative paths):".to_string());
        prompt_lines.push(files_map.to_string());
        prompt_lines.push("".to_string());
    }

    // Inject external files map under [EXTERNAL_FILES_MAP] tag only when the query references the external project
    if let Some(ref ext_project_path) = config.knowledge.as_ref().and_then(|k| k.external_project.as_ref()) {
        if let Some(ext_folder_name) = std::path::Path::new(ext_project_path).file_name().and_then(|n| n.to_str()) {
            let input_lower = input.to_lowercase();
            let folder_lower = ext_folder_name.to_lowercase();
            let references_external = input_lower.contains(&folder_lower) || input.contains("../") || input.contains(r"..\");

            if references_external {
                if let Some(files) = external_files {
                    if !files.is_empty() {
                        prompt_lines.push("[EXTERNAL_FILES_MAP]".to_string());
                        prompt_lines.push(files.join("\n"));
                        prompt_lines.push("[/EXTERNAL_FILES_MAP]".to_string());
                        prompt_lines.push("".to_string());
                    }
                }
            }
        }
    }

    for turn in history {
        if turn.role == "user" {
            prompt_lines.push(format!("User: {}", turn.content));
        } else {
            prompt_lines.push(format!("Assistant (JSON): {}", turn.content));
        }
    }
    prompt_lines.push(format!("User: {}", input));
    let prompt = prompt_lines.join("\n");

    let threads = config.brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    let params = InferParams {
        model: config.brain.model.clone(),
        threads,
        max_tokens: config.brain.max_tokens,
        temperature: 0.1,
        repeat_penalty: 1.1,
        max_context: 5000, // Classifier context limit increased to 5000 tokens
        grammar: None,
        provider: Some(config.brain.provider.clone()),
        system: Some(system_instructions),
        history: None,
    };

    let res = uneven_core::infrastructure::llm::infer_with_params(&prompt, &params).await.ok()?;
    let parsed = extract_json(&res.content)?;
    let intent = json_to_intent(&parsed);
    Some((intent, parsed.message, res.tokens_used))
}

pub async fn classify(
    input: &str,
    history: &[BrainTurn],
    files_map: &str,
    config: &UnevenConfig,
    external_files: Option<&[String]>,
) -> (ClassifiedIntent, String, usize) {
    let frames = [
        format!("{} {}", style("⠋").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠙").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠹").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠸").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠼").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠴").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠦").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠧").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠇").cyan(), style("Classifying intent…").dim()),
        format!("{} {}", style("⠏").cyan(), style("Classifying intent…").dim()),
    ];
    let frames_ref: Vec<&str> = frames.iter().map(|s| s.as_str()).collect();
    let classify_spinner = ProgressBar::new_spinner();
    classify_spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&frames_ref)
            .template("  ↳ {spinner}").unwrap(),
    );
    classify_spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let quick = classify_intent(input);
    let is_utility = matches!(
        quick.action,
        ShellAction::Exit
            | ShellAction::Help
            | ShellAction::Log
            | ShellAction::Undo
            | ShellAction::Index
            | ShellAction::Scan
            | ShellAction::Watch
    );

    if quick.confidence == "high" && is_utility {
        classify_spinner.finish_and_clear();
        return (quick, String::new(), 0);
    }

    if config.brain.provider != "local" && !config.brain.provider.is_empty() {
        if let Some((intent, message, tokens)) = classify_with_brain(input, history, files_map, config, external_files).await {
            classify_spinner.finish_and_clear();
            return (intent, message, tokens);
        }
    }

    classify_spinner.finish_and_clear();
    (quick, String::new(), 0)
}

pub(crate) fn get_project_files_map(config: &UnevenConfig, root: &std::path::Path) -> String {
    let dirs = config.knowledge.as_ref()
        .and_then(|k| k.dirs.clone())
        .unwrap_or_else(|| vec![".".to_string()]);
    let scanner = ProjectScanner::new(root);
    let mut files = scanner.scan(dirs.clone()).unwrap_or_default();

    if let Some(extra_files) = config.knowledge.as_ref().and_then(|k| k.files.clone()) {
        for f in extra_files {
            let normalized_f = f.replace('\\', "/");
            let abs_path = root.join(&normalized_f);
            if abs_path.exists() && abs_path.is_file() {
                files.push(normalized_f);
            }
        }
        files.sort();
        files.dedup();
    }
    files.join("\n")
}

fn print_formatted_content(text: &str, formatter: &mut crate::ui::TerminalMarkdownFormatter) -> std::io::Result<()> {
    let parts: Vec<&str> = text.split('\n').collect();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            print!("\n  \x1b[38;2;232;237;245m");
        }
        let formatted = formatter.process_token(part);
        print!("{}", formatted);
    }
    std::io::stdout().flush()
}

#[allow(clippy::too_many_arguments)]
async fn run_inference_streaming(
    prompt: &str,
    system_instructions: &str,
    _config: &UnevenConfig,
    brain: &uneven_core::domain::config::BrainConfig,
    classifier_tokens: usize,
    elapsed_classify: std::time::Duration,
    total_session_tokens: Arc<std::sync::atomic::AtomicUsize>,
    mut history: Option<&mut Vec<BrainTurn>>,
) -> Result<String> {
    let start_time = std::time::Instant::now();
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    init_llm(&brain.provider, &brain.model, threads).await?;

    let mut prompt_to_use = prompt.to_string();
    let mut system_instructions_to_use = system_instructions.to_string();

    if let Some(ref mut mut_history) = history {
        if brain.provider != "local" {
            let root = std::path::PathBuf::from(&_config.project_root);
            let files_map = get_project_files_map(_config, &root);

            prompt_to_use = uneven_core::infrastructure::llm::history_manager::optimize_context(
                mut_history,
                prompt,
                system_instructions,
                Some(&files_map),
            );

            // Update the last turn in history to the optimized prompt
            if let Some(last_turn) = mut_history.last_mut() {
                if last_turn.role == "user" {
                    last_turn.content = prompt_to_use.clone();
                }
            }

            system_instructions_to_use = String::new();
        }
    }

    // Clone the history excluding the last user turn (which is the current prompt)
    // to pass to LLM providers as conversational history.
    let past_history = history.as_ref().map(|hist| {
        let mut turns = (*hist).clone();
        if let Some(last) = turns.last() {
            if last.role == "user" {
                turns.pop();
            }
        }
        turns
    });

    let chat_template = detect_template(&brain.model);
    let prompt_to_use = if brain.provider == "local" {
        let chat_history = past_history.as_ref().map(|hist| {
            hist.iter().map(|turn| ChatTurn {
                role: turn.role.clone(),
                content: turn.content.clone(),
            }).collect::<Vec<_>>()
        }).unwrap_or_default();
        apply_template(&chat_template, &system_instructions_to_use, &chat_history, &prompt_to_use)
    } else {
        prompt_to_use
    };

    let guardian = ResourceGuardian::start(brain.provider.clone(), std::path::PathBuf::from(&_config.project_root));

    let api_history = past_history.as_ref().map(|hist| {
        hist.iter().map(|turn| {
            (turn.role.clone(), turn.content.clone())
        }).collect::<Vec<(String, String)>>()
    });

    let system_param = if system_instructions_to_use.is_empty() {
        None
    } else {
        Some(system_instructions_to_use)
    };

    let params = InferParams {
        model: brain.model.clone(),
        threads,
        max_tokens: brain.max_tokens,
        temperature: brain.temperature as f64,
        repeat_penalty: 1.1,
        max_context: 4096,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: system_param,
        history: api_history,
    };

    let frames = [
        format!("{} {}", style("⠋").cyan(), style("Thinking.  ").dim()),
        format!("{} {}", style("⠙").cyan(), style("Thinking.. ").dim()),
        format!("{} {}", style("⠹").cyan(), style("Thinking...").dim()),
        format!("{} {}", style("⠸").cyan(), style("Thinking.  ").dim()),
        format!("{} {}", style("⠼").cyan(), style("Thinking.. ").dim()),
        format!("{} {}", style("⠴").cyan(), style("Thinking...").dim()),
        format!("{} {}", style("⠦").cyan(), style("Thinking.  ").dim()),
        format!("{} {}", style("⠧").cyan(), style("Thinking.. ").dim()),
        format!("{} {}", style("⠇").cyan(), style("Thinking...").dim()),
        format!("{} {}", style("⠏").cyan(), style("Thinking.  ").dim()),
    ];
    let frames_ref: Vec<&str> = frames.iter().map(|s| s.as_str()).collect();

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&frames_ref)
            .template("  ↳ {spinner}").unwrap(),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
    let tx_clone = tx.clone();
    let prompt_to_use_clone = prompt_to_use.clone();

    let params_clone = InferParams {
        model: params.model.clone(),
        threads: params.threads,
        max_tokens: params.max_tokens,
        temperature: params.temperature,
        repeat_penalty: params.repeat_penalty,
        max_context: params.max_context,
        grammar: params.grammar.clone(),
        provider: params.provider.clone(),
        system: params.system.clone(),
        history: params.history.clone(),
    };

    let inference_handle = tokio::spawn(async move {
        infer_with_params_streaming(
            &prompt_to_use_clone,
            &params_clone,
            move |token| {
                let _ = tx_clone.try_send(token);
            }
        ).await
    });

    drop(tx);

    let mut full_response = String::new();
    let mut streaming_started = false;
    let mut formatter = crate::ui::TerminalMarkdownFormatter::new();

    let mut buffer = String::new();
    let mut buffering = false;

    while let Some(token) = rx.recv().await {
        if !streaming_started {
            spinner.finish_and_clear();
            println!("{}", truecolor("  ┌─ Response", 107, 122, 150));
            println!();
            print!("  \x1b[38;2;232;237;245m");
            let _ = std::io::stdout().flush();
            streaming_started = true;
        }

        full_response.push_str(&token);

        for c in token.chars() {
            if c == '[' {
                if buffering {
                    let _ = print_formatted_content(&buffer, &mut formatter);
                    buffer.clear();
                }
                buffering = true;
                buffer.push(c);
            } else if buffering {
                buffer.push(c);
                let is_prefix_of_any = {
                    let b_lower = buffer.to_lowercase();
                    "[need_file".starts_with(&b_lower) ||
                    "[web_search".starts_with(&b_lower) ||
                    "[web_url".starts_with(&b_lower) ||
                    b_lower.starts_with("[need_file") ||
                    b_lower.starts_with("[web_search") ||
                    b_lower.starts_with("[web_url")
                };

                if !is_prefix_of_any || buffer.len() > 200 {
                    let _ = print_formatted_content(&buffer, &mut formatter);
                    buffer.clear();
                    buffering = false;
                } else if c == ']' {
                    buffer.clear();
                    buffering = false;
                }
            } else {
                let single_char_str = c.to_string();
                let _ = print_formatted_content(&single_char_str, &mut formatter);
            }
        }
    }

    if buffering && !buffer.is_empty() {
        let _ = print_formatted_content(&buffer, &mut formatter);
    }

    if streaming_started {
        print!("\x1b[0m");
        println!();
    } else {
        spinner.finish_and_clear();
    }

    guardian.stop();

    let result = inference_handle.await??;
    total_session_tokens.fetch_add(result.tokens_used, std::sync::atomic::Ordering::Relaxed);
    let elapsed = start_time.elapsed() + elapsed_classify;
    let total_tokens = result.tokens_used + classifier_tokens;

    let (cost, saved) = uneven_core::infrastructure::llm::pricing::calculate_cost_and_saved(
        &result.model,
        result.prompt_tokens,
        result.completion_tokens,
    );

    println!("\n{}", truecolor("  └─ Done", 107, 122, 150));
    println!(
        "{}",
        truecolor(
            &format!(
                "     [Tokens used: {} | Time: {} | Cost: ${:.5} | Saved: ${:.5}]",
                total_tokens, format_duration(elapsed), cost, saved
            ),
            107, 122, 150
        )
    );

    Ok(full_response)
}

async fn spawn_subcommand_silent(args: &[&str]) -> Result<i32> {
    let current_exe = std::env::current_exe()?;
    let mut child = tokio::process::Command::new(current_exe)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let status = child.wait().await?;
    Ok(status.code().unwrap_or(0))
}

async fn spawn_subcommand_silent_in_dir(args: &[&str], dir: &std::path::Path) -> Result<i32> {
    let current_exe = std::env::current_exe()?;
    let mut child = tokio::process::Command::new(current_exe)
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let status = child.wait().await?;
    Ok(status.code().unwrap_or(0))
}

async fn spawn_subcommand(args: &[&str]) -> Result<i32> {
    SUBPROCESS_RUNNING.store(true, std::sync::atomic::Ordering::Relaxed);
    let current_exe = std::env::current_exe()?;
    let mut child = tokio::process::Command::new(current_exe)
        .args(args)
        .spawn()?;
    let status = child.wait().await?;
    SUBPROCESS_RUNNING.store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(status.code().unwrap_or(0))
}

fn format_duration(d: std::time::Duration) -> String {
    let secs_f = d.as_secs_f64();
    if secs_f < 1.0 {
        let ms = (secs_f * 1000.0).round() as u64;
        format!("{}ms", ms)
    } else if secs_f < 60.0 {
        let secs = secs_f.round() as u64;
        format!("{}s", secs)
    } else {
        let total_secs = secs_f.round() as u64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if secs == 0 {
            format!("{}m", mins)
        } else {
            format!("{}m {}s", mins, secs)
        }
    }
}

fn get_hint(action: &ShellAction) -> Option<&'static str> {
    match action {
        ShellAction::Index => Some("Project indexed. Try \"scan\" or ask anything about your code."),
        ShellAction::Scan => Some("Scan complete. Use \"watch\" to monitor errors in real time."),
        ShellAction::Watch => Some("Watcher stopped. Back to interactive mode."),
        ShellAction::Explain => Some("Want more detail? Ask a question or try \"docs <file>\"."),
        ShellAction::Review => Some("Review done. Use \"watch\" to catch live errors or \"undo\" to revert the last fix."),
        ShellAction::Docs => Some("Tip: use --out docs/<file>.md via CLI to save output to a file."),
        ShellAction::Undo => Some("Fix reverted. The code is back to its previous state."),
        _ => None,
    }
}

fn print_welcome(version: &str, brain: &str, is_first_run: bool) {
    let d = |s: &str| truecolor(s, 107, 122, 150);
    println!();
    ui::box_welcome(version, brain, is_first_run);
    println!();
    println!("{}", d("  How can I help you today?"));
    println!();
}

fn print_shell_help() {
    let a = |s: &str| truecolor(s, 168, 200, 255);
    let d = |s: &str| truecolor(s, 107, 122, 150);

    let w = |phrase: &str, desc: &str| {
        println!("  {}{}", a(&format!("{:width$}", phrase, width = 36)), d(desc));
    };

    println!();
    println!("{}", d("  What I can do for you:"));
    println!();
    w("\"index my project\"", "Learn your codebase and docs");
    w("\"scan\"", "Check for vulnerabilities and malware");
    w("\"watch\"", "Monitor terminal errors and fix them automatically");
    w("\"explain src/app.ts\"", "Explain a file in plain English");
    w("\"review my last commit\"", "AI code review of the latest commit");
    w("\"docs src/utils.ts\"", "Generate Markdown API documentation");
    w("\"create a login endpoint\"", "Create or edit files for a given task");
    w("\"<any question>\"", "Ask anything about the project");
    w("\"log\"", "Show Uneven activity history");
    w("\"undo\"", "Revert the last fix that was applied");
    w("\"exit\"", "Leave the interactive shell");
    println!();
    println!("{}", d("  Tip: all CLI subcommands still work directly (uneven watch, etc.)"));
    println!();
}

pub async fn run() -> Result<()> {
    struct TerminalGuard;
    impl Drop for TerminalGuard {
        fn drop(&mut self) {
            print!("\x1b[r");
            let _ = std::io::stdout().flush();
        }
    }
    let _guard = TerminalGuard;

    let root = loader::find_project_root(None);
    let is_first_run = std::env::var("UNEVEN_FIRST_RUN").is_ok();

    let mut config = loader::load_config(&root)?;

    tokio::spawn(async move {
        let _ = spawn_subcommand_silent(&["index"]).await;
    });

    let mut external_files_list: Option<Vec<String>> = None;
    if let Some(ref ext_project_path) = config.knowledge.as_ref().and_then(|k| k.external_project.as_ref()) {
        let ext_path = std::path::Path::new(ext_project_path);
        let ext_hash = uneven_core::infrastructure::utils::paths::project_hash(ext_path);
        let cache_path = uneven_core::infrastructure::utils::paths::global_bin_dir()
            .join("cache")
            .join(&ext_hash)
            .join("file-list-cache.json");
        if cache_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cache_path) {
                if let Ok(files) = serde_json::from_str::<Vec<String>>(&content) {
                    external_files_list = Some(files);
                }
            }
        }

        let ext_dir = std::path::PathBuf::from(ext_project_path);
        tokio::spawn(async move {
            let _ = spawn_subcommand_silent_in_dir(&["index"], &ext_dir).await;
        });
    }

    let history_limit = if config.brain.provider == "local" { 8 } else { 14 };
    let static_strip_re = regex::Regex::new(r"(?i)FILE:\s*\S+\n```[\s\S]*?```\s*END_FILE\n?").ok();
    let interactive_session_id = uuid::Uuid::new_v4().to_string()[..6].to_uppercase();

    let threads = config.brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    let project_name = root.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let version = env!("CARGO_PKG_VERSION");
    let active_brain = format!("{} / {}", config.brain.provider, config.brain.model);
    print_welcome(version, &active_brain, is_first_run);

    if is_first_run {
        let accent = |s: &str| style(s.to_string()).color256(111).bold();
        let dim = |s: &str| style(s.to_string()).dim();
        let bold = |s: &str| style(s.to_string()).bold();

        println!("  {} Welcome to Uneven Agent! Let's get you set up.", style("◈").color256(111).bold());
        println!("  {} To get started, type {} (to open the menu) and select {} to choose your model before continuing.",
            style("›").color256(111).bold(), accent("/"), accent("/models")
        );
        println!();
        println!("  {} Configuring Cloud API Keys (if using cloud brains):", style("ℹ").color256(111).bold());
        println!("  You can supply your API keys (e.g., GOOGLE_API_KEY or ANTHROPIC_API_KEY) in three ways:");
        println!("    {}  {}   Export them directly in your current terminal session (e.g., export GOOGLE_API_KEY=...).", dim("1."), bold("Session:"));
        println!("    {}  {}   Create a {} file in the root directory of your project.", dim("2."), bold("Project:"), accent(".env"));
        println!("    {}  {}   Use the {} command inside this shell. This is a global, secure,", dim("3."), bold("Global:"), accent("/export <KEY> [value]"));
        println!("                  and persistent setup stored at {} that works across all your projects.", dim("~/.uneven/bin/env"));
        println!();
    }

    let is_local = config.brain.provider == "local";
    let term = console::Term::stdout();
    let (rows, cols) = term.size();

    // Render initial status bar and prompt
    if rows > 5 {
        print!("\x1b[1;{}r\x1b[{};1H\x1b[2K", rows - 3, rows);
        let left_text = "  [ / ] Menu   [ help ] Help   [ exit ] Exit";
        let right_text = if is_local {
            format!("Brain: {}/{} (Loading...)  ", config.brain.provider, config.brain.model)
        } else {
            format!("Brain: {}/{}  ", config.brain.provider, config.brain.model)
        };
        let padding = (cols as usize).saturating_sub(left_text.len() + right_text.len());
        let bar = format!("{}{:<width$}{}", left_text, "", right_text, width = padding);
        let styled_bar = truecolor(&bar, 170, 180, 195);
        let bar_bg = format!("\x1b[48;2;30;35;45m{}\x1b[0m", styled_bar);
        print!("{}", bar_bg);

        print!("\x1b[{};1H\x1b[2K", rows - 1);
        print!("\x1b[{};1H\x1b[2K", rows - 2);

        // Draw initial prompt
        print!("\x1b[{};1H\x1b[2K", rows - 3);
        let prompt = format!(
            "  {} {} {} ",
            truecolor("uneven", 232, 237, 245),
            truecolor(&format!("({})", project_name), 107, 122, 150),
            truecolor("›", 168, 200, 255)
        );
        print!("{}", prompt);
        let _ = std::io::stdout().flush();
    }

    // Hide cursor if local
    if is_local {
        print!("\x1b[?25l");
        let _ = std::io::stdout().flush();
    }

    let _ = init_llm(&config.brain.provider, &config.brain.model, threads).await;

    // Restore cursor and status bar if local
    if is_local {
        print!("\x1b[?25h");
        if rows > 5 {
            print!("\x1b[{};1H\x1b[2K", rows);
            let left_text = "  [ / ] Menu   [ help ] Help   [ exit ] Exit";
            let right_text = format!("Brain: {}/{}  ", config.brain.provider, config.brain.model);
            let padding = (cols as usize).saturating_sub(left_text.len() + right_text.len());
            let bar = format!("{}{:<width$}{}", left_text, "", right_text, width = padding);
            let styled_bar = truecolor(&bar, 170, 180, 195);
            let bar_bg = format!("\x1b[48;2;30;35;45m{}\x1b[0m", styled_bar);
            print!("{}", bar_bg);
            print!("\x1b[{};1H", rows - 3);
            let _ = std::io::stdout().flush();
        }
    }

    let total_session_tokens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_session_tokens_ctrlc = total_session_tokens.clone();

    tokio::spawn(async move {
        loop {
            let _ = tokio::signal::ctrl_c().await;
            if !SUBPROCESS_RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                let total = total_session_tokens_ctrlc.load(std::sync::atomic::Ordering::Relaxed);
                restore_terminal_ui();
                if total > 0 {
                    println!("\n{}", truecolor(&format!("  [Total tokens used in this session: {}]", total), 107, 122, 150));
                }
                println!("{}", truecolor("\n  Goodbye!\n", 107, 122, 150));
                {
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }
                std::process::exit(0);
            }
        }
    });




    let pending_index = Arc::new(AtomicBool::new(false));
    let pending_index_clone = pending_index.clone();

    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
    let mut _watcher = None;
    let handle = tokio::runtime::Handle::current();
    if config_path.exists() {
        let handle_clone = handle.clone();
        if let Ok(mut fw) = uneven_core::infrastructure::watcher::file::FileWatcher::new(move |path| {
            let path_lower = path.to_lowercase();
            let path_buf = std::path::Path::new(&path_lower);
            let is_ignored = path_buf.components().any(|c| {
                if let Some(s) = c.as_os_str().to_str() {
                    s == ".git"
                        || s == ".uneven"
                        || s == "node_modules"
                        || s == "target"
                        || s == "dist"
                        || s == "build"
                        || s == "venv"
                        || s == ".venv"
                } else {
                    false
                }
            });

            if !is_ignored {
                pending_index_clone.store(true, Ordering::Relaxed);
                handle_clone.spawn(async move {
                    let _ = spawn_subcommand_silent(&["index"]).await;
                });
            }
        }) {
            let _ = fw.watch(&root.to_string_lossy());
            let _ = fw.watch(&config_path.to_string_lossy());
            _watcher = Some(fw);
        }
    }

    let mut history: Vec<BrainTurn> = Vec::new();
    let safety = SafetyGuard::new();
    let mut auto_prompt: Option<String> = None;
    let mut always_approve_commands = false;

    let rl = rustyline::DefaultEditor::new()?;
    let rl = Arc::new(std::sync::Mutex::new(rl));

    loop {
        if pending_index.swap(false, Ordering::Relaxed) {
            if let Ok(new_cfg) = loader::load_config(&root) {
                config = new_cfg;
            }
        }

        let input = if let Some(ap) = auto_prompt.take() {
            ap
        } else {
            // Draw the sticky bottom status bar
            let term = console::Term::stdout();
            let (rows, cols) = term.size();
            if rows > 5 {
                // Set scrolling region (1 to rows - 3) and move to bottom line (rows) to draw status bar
                print!("\x1b[1;{}r\x1b[{};1H\x1b[2K", rows - 3, rows);
                let left_text = "  [ / ] Menu   [ help ] Help   [ exit ] Exit";
                let right_text = format!("Brain: {}/{}  ", config.brain.provider, config.brain.model);
                let padding = (cols as usize).saturating_sub(left_text.len() + right_text.len());
                let bar = format!("{}{:<width$}{}", left_text, "", right_text, width = padding);

                // Dark grey background (RGB 30, 35, 45) and light grey text (RGB 170, 180, 195)
                let styled_bar = truecolor(&bar, 170, 180, 195);
                let bar_bg = format!("\x1b[48;2;30;35;45m{}\x1b[0m", styled_bar);
                print!("{}", bar_bg);

                // Clear the two lines of space (rows - 1 and rows - 2)
                print!("\x1b[{};1H\x1b[2K", rows - 1);
                print!("\x1b[{};1H\x1b[2K", rows - 2);

                // Move cursor to the prompt line (rows - 3) and clear it
                print!("\x1b[{};1H\x1b[2K", rows - 3);
                let _ = std::io::stdout().flush();
            } else if rows > 3 {
                // Fallback for smaller screens
                print!("\x1b[1;{}r\x1b[{};1H\x1b[2K", rows - 1, rows);
                let left_text = "  [ / ] Menu   [ help ] Help   [ exit ] Exit";
                let right_text = format!("Brain: {}/{}  ", config.brain.provider, config.brain.model);
                let padding = (cols as usize).saturating_sub(left_text.len() + right_text.len());
                let bar = format!("{}{:<width$}{}", left_text, "", right_text, width = padding);

                let styled_bar = truecolor(&bar, 170, 180, 195);
                let bar_bg = format!("\x1b[48;2;30;35;45m{}\x1b[0m", styled_bar);
                print!("{}", bar_bg);

                print!("\x1b[{};1H\x1b[2K", rows - 1);
                let _ = std::io::stdout().flush();
            }

            // Force cursor to be shown and blinking before reading input, and clear prompt line
            print!("\x1b[?25h\x1b[?12h\r\x1b[2K");
            let _ = std::io::stdout().flush();

            let prompt = format!(
                "  {} {} {} ",
                truecolor("uneven", 232, 237, 245),
                truecolor(&format!("({})", project_name), 107, 122, 150),
                truecolor("›", 168, 200, 255)
            );

            let rl_clone = rl.clone();
            let prompt_clone = prompt.clone();

            let line_res = match tokio::task::spawn_blocking(move || {
                let mut guard = rl_clone.lock().unwrap();
                guard.readline(&prompt_clone)
            }).await {
                Ok(Ok(line)) => Ok(line),
                Ok(Err(rustyline::error::ReadlineError::Interrupted)) => Err("interrupted"),
                Ok(Err(rustyline::error::ReadlineError::Eof)) => Err("eof"),
                _ => Err("error"),
            };

            match line_res {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let mut guard = rl.lock().unwrap();
                    let _ = guard.add_history_entry(trimmed);
                    line
                }
                Err("interrupted") => {
                    restore_terminal_ui();
                    let total = total_session_tokens.load(std::sync::atomic::Ordering::Relaxed);
                    if total > 0 {
                        println!("{}", truecolor(&format!("\n  [Total tokens used in this session: {}]", total), 107, 122, 150));
                    }
                    println!("{}", truecolor("\n  Goodbye!\n", 107, 122, 150));
                    std::process::exit(0);
                }
                _ => break,
            }
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('/') {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            let slash_cmd = parts[0].to_lowercase();

            if slash_cmd == "/export" {
                if parts.len() < 2 {
                    println!("\n  {} Usage: /export <VARIABLE_NAME> [value]\n", style("  ✗ ").red());
                    continue;
                }
                let payload = trimmed.strip_prefix("/export").unwrap().trim();
                let (key, val) = if let Some((k, v)) = payload.split_once('=') {
                    (k.trim(), v.trim())
                } else if let Some((k, v)) = payload.split_once(' ') {
                    (k.trim(), v.trim())
                } else {
                    (payload, "")
                };

                let mut val_clean = val.trim_matches(|c| c == '\'' || c == '"').to_string();
                if val_clean.is_empty() {
                    println!("  {}", ui::accent(&format!("Enter the value for {} (or press Enter to cancel):", key)));
                    print!("  › ");
                    let _ = std::io::stdout().flush();
                    let mut val_buf = String::new();
                    if std::io::stdin().read_line(&mut val_buf).is_ok() {
                        val_clean = val_buf.trim().trim_matches(|c| c == '\'' || c == '"').to_string();
                    }
                }

                if val_clean.is_empty() {
                    println!("\n  {} Operation cancelled.\n", style("  ✗ ").red());
                    continue;
                }

                let global_env_path = uneven_core::infrastructure::utils::paths::global_bin_dir().join("env");
                if let Some(parent) = global_env_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                let mut lines = Vec::new();
                if global_env_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&global_env_path) {
                        for line in content.lines() {
                            let l_trim = line.trim();
                            if !l_trim.starts_with(&format!("{}=", key)) && !l_trim.is_empty() {
                                lines.push(line.to_string());
                            }
                        }
                    }
                }
                lines.push(format!("{}={}", key, val_clean));
                if std::fs::write(&global_env_path, lines.join("\n") + "\n").is_ok() {
                    std::env::set_var(key, val_clean);
                    println!("\n  {} {} saved successfully to the global environment!\n", ui::green("✓"), key);
                } else {
                    println!("\n  {} Failed to save variable to the global environment\n", style("✗").red());
                }
            } else if slash_cmd == "/models" || slash_cmd == "/model" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                let mut selected_opt = None;

                if !payload.is_empty() {
                    match parse_multi_brains(payload) {
                        Err(e) => {
                            println!("\n  {} {}\n", style("  ✗ ").red(), e);
                            continue;
                        }
                        Ok(parsed_brains) => {
                            if !parsed_brains.is_empty() {
                                if slash_cmd == "/model" && parsed_brains.len() == 1 && parsed_brains[0].0.is_none() {
                                    let (_, m, p) = &parsed_brains[0];
                                    selected_opt = Some((m.clone(), p.clone()));
                                } else {
                                    allocate_brains(parsed_brains, &mut config);

                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }

                                    let threads = config.brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
                                    let _ = init_llm(&config.brain.provider, &config.brain.model, threads).await;

                                    println!("\n  {} Multi-Brains configuration successfully activated!", ui::green("✓"));
                                    if let Some(ref r) = config.routing {
                                        for (cmd, bid) in r {
                                            println!("     - {}: {}", ui::accent(cmd), ui::bright(bid));
                                        }
                                    }
                                    println!();
                                    continue;
                                }
                            }
                        }
                    }
                    let list_options = vec![
                        "GPT 5.5                          | Cloud model",
                        "GPT 4o                           | Cloud model",
                        "Gemini 3.5 Flash                 | Cloud model",
                        "Gemini 2.5 Flash                 | Cloud model",
                        "Claude 4.8 Opus                  | Cloud model",
                        "Claude 4.6 Sonnet                | Cloud model",
                        "DeepSeek V4 Flash                | Cloud model",
                        "DeepSeek V4 Pro                  | Cloud model",
                        "Llama 3.2 1B Q4                  | Local model",
                        "Qwen 2.5 1.5B Q8                 | Local model",
                        "DeepSeek R1 Distill 8B Q4        | Local model",
                        "Phi-4 14B Q4                     | Local model",
                        "Others (Type custom spec)        | Custom model",
                    ];

                    if let Some(idx) = ui::select_item("Choose AI Model:", &list_options) {
                        if idx == 12 {
                            println!("\n  {} Enter custom model spec (e.g. deepseek-r1-8b-q4 or openai/gpt-4o):", ui::accent("›"));
                            print!("  › ");
                            let _ = std::io::stdout().flush();
                            let mut spec_buf = String::new();
                            if std::io::stdin().read_line(&mut spec_buf).is_ok() {
                                let spec = spec_buf.trim().to_string();
                                if !spec.is_empty() {
                                    match parse_multi_brains(&spec) {
                                        Err(e) => {
                                            println!("\n  {} {}\n", style("  ✗ ").red(), e);
                                            continue;
                                        }
                                        Ok(parsed_brains) => {
                                            if !parsed_brains.is_empty() && parsed_brains[0].0.is_none() {
                                                let (_, m, p) = &parsed_brains[0];
                                                selected_opt = Some((m.clone(), p.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            let mapped = match idx {
                                0 => ("gpt-5.5", "openai"),
                                1 => ("gpt-4o", "openai"),
                                2 => ("gemini-3.5-flash", "gemini"),
                                3 => ("gemini-2.5-flash", "gemini"),
                                4 => ("claude-opus-4-8", "claude"),
                                5 => ("claude-sonnet-4-6", "claude"),
                                6 => ("deepseek-v4-flash", "deepseek"),
                                7 => ("deepseek-v4-pro", "deepseek"),
                                8 => ("llama-3.2-1b-q4", "local"),
                                9 => ("qwen-2.5-1.5b-q8", "local"),
                                10 => ("deepseek-r1-8b-q4", "local"),
                                _ => ("phi-4-14b-q4", "local"),
                            };
                            selected_opt = Some((mapped.0.to_string(), mapped.1.to_string()));
                        }
                    }
                }

                if let Some((m, p)) = selected_opt {
                    if p == "local" {
                        if let Err(e) = ensure_local_model_files(&m).await {
                            println!("\n  {} {}\n", style("✗").red(), e);
                            continue;
                        }
                    }
                    config.brain.provider = p.clone();
                    config.brain.model = m.clone();
                    config.brains = None;
                    config.routing = None;

                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }

                    let threads = config.brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
                    let _ = init_llm(&config.brain.provider, &config.brain.model, threads).await;

                    println!("\n  {} Active Brain updated to: {} / {}\n", ui::green("✓"), p, m);
                }
            } else if slash_cmd == "/analyze" {
                if parts.len() < 2 {
                    println!("\n  {} Usage:\n", style("  ✗ ").red());
                    println!("     - /analyze db <url>               : Configure database URL");
                    println!("     - /analyze tables <t1> <t2> ...   : Configure allowed tables for reading");
                    println!("     - /analyze mask <c1> <c2> ...     : Mask values of specified columns");
                    println!("     - /analyze mask tables <t1> ... columns <c1> ... : Mask tables and columns separately\n");
                    continue;
                }

                let sub_action = parts[1].to_lowercase();

                if sub_action == "db" {
                    if parts.len() < 3 {
                        println!("\n  {} Please provide the database URL. Ex: /analyze db postgres://localhost/db\n", style("  ✗ ").red());
                        continue;
                    }
                    let db_url = parts[2..].join(" ");

                    let mut db_cfg = config.knowledge.as_ref()
                        .and_then(|k| k.db.clone())
                        .unwrap_or_default();
                    db_cfg.url = Some(db_url.clone());

                    let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                    k_cfg.db = Some(db_cfg);
                    config.knowledge = Some(k_cfg);

                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }
                    println!("\n  {} Database configured to: {}\n", ui::green("✓"), db_url);
                } else if sub_action == "tables" {
                    if parts.len() < 3 {
                        println!("\n  {} Please provide the table names. Ex: /analyze tables products sales\n", style("  ✗ ").red());
                        continue;
                    }
                    let tables: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();

                    let mut db_cfg = config.knowledge.as_ref()
                        .and_then(|k| k.db.clone())
                        .unwrap_or_default();
                    db_cfg.allowed_tables = Some(tables.clone());

                    let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                    k_cfg.db = Some(db_cfg);
                    config.knowledge = Some(k_cfg);

                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }
                    println!("\n  {} Allowed tables updated: {}\n", ui::green("✓"), tables.join(", "));
                } else if sub_action == "mask" {
                    if parts.len() < 3 {
                        println!("\n  {} Please provide items to mask. Ex: /analyze mask cpf password\n", style("  ✗ ").red());
                        continue;
                    }

                    let payload = parts[2..].join(" ");
                    if payload.contains("tables") || payload.contains("columns") {
                        let mut current_mode = 0; // 0 = none, 1 = tables, 2 = columns
                        let mut m_tables = Vec::new();
                        let mut m_columns = Vec::new();

                        let tokens_to_parse = &parts[2..];
                        for token in tokens_to_parse {
                            let tok_lower = token.to_lowercase();
                            if tok_lower == "tables" || tok_lower == "table" {
                                current_mode = 1;
                            } else if tok_lower == "columns" || tok_lower == "column" {
                                current_mode = 2;
                            } else {
                                match current_mode {
                                    1 => m_tables.push(token.to_string()),
                                    2 => m_columns.push(token.to_string()),
                                    _ => {
                                        m_columns.push(token.to_string());
                                    }
                                }
                            }
                        }

                        if !m_tables.is_empty() {
                            config.security.blocked_tables = m_tables.clone();
                        }
                        if !m_columns.is_empty() {
                            config.security.masked_columns = m_columns.clone();
                        }

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }

                        println!("\n  {} Advanced masking configured:", ui::green("✓"));
                        if !m_tables.is_empty() {
                            println!("     - Blocked tables: {}", m_tables.join(", "));
                        }
                        if !m_columns.is_empty() {
                            println!("     - Masked columns: {}", m_columns.join(", "));
                        }
                        println!();
                    } else {
                        let columns: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();
                        config.security.masked_columns = columns.clone();

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }
                        println!("\n  {} Masked columns updated: {}\n", ui::green("✓"), columns.join(", "));
                    }
                } else {
                    println!("\n  {} Unknown subcommand. Options: db, file, tables, mask\n", style("  ✗ ").red());
                }
            } else if slash_cmd == "/pentest" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    let sub_parts: Vec<&str> = payload.split_whitespace().collect();
                    if sub_parts[0].to_lowercase() == "target" {
                        if sub_parts.len() < 2 {
                            println!("\n  {} Usage: /pentest target <target_url> [scope_file]\n", style("  ✗ ").red());
                            continue;
                        }
                        let target_url = sub_parts[1].to_string();

                        let mut p_cfg = config.pentester.clone().unwrap_or_default();
                        p_cfg.target = Some(target_url.clone());
                        config.pentester = Some(p_cfg);

                        if sub_parts.len() >= 3 {
                            let src_file = sub_parts[2];
                            let src_path = std::path::Path::new(src_file);
                            if src_path.exists() && src_path.is_file() {
                                let dest_dir = root.join(".uneven");
                                let _ = std::fs::create_dir_all(&dest_dir);
                                let dest_path = dest_dir.join("pentest-scope.json");
                                if let Err(e) = std::fs::copy(src_path, &dest_path) {
                                    println!("\n  {} Error copying scope file: {}\n", style("✗").red(), e);
                                } else {
                                    println!("\n  {} Scope file successfully copied to: {}\n", ui::green("✓"), dest_path.to_string_lossy());
                                }
                            } else {
                                println!("\n  {} Scope file '{}' not found.\n", style("  ✗ ").red(), src_file);
                            }
                        }

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }
                        println!("\n  {} Pentest target configured to: {}\n", ui::green("✓"), target_url);
                    } else {
                        println!("\n  {} Unknown subcommand. Usage: /pentest target <target_url> [scope_file]\n", style("  ✗ ").red());
                    }
                    continue;
                }

                let mut temp_pentester = config.pentester.clone().unwrap_or_default();
                loop {
                    let mode_str = match temp_pentester.mode {
                        Some(uneven_core::domain::PentesterMode::Active) => "Active",
                        _ => "Static",
                    };

                    let owasp_val = temp_pentester.static_config.as_ref()
                        .and_then(|s| s.owasp)
                        .unwrap_or(false);

                    let secrets_val = temp_pentester.static_config.as_ref()
                        .and_then(|s| s.secrets)
                        .unwrap_or(false);

                    let bruteforce_val = temp_pentester.bruteforce.as_ref()
                        .and_then(|b| b.enabled)
                        .unwrap_or(false);

                    let firewall_val = temp_pentester.firewall.as_ref()
                        .and_then(|f| f.enabled)
                        .unwrap_or(false);

                    let options = [
                        format!("Mode: {} (Toggle Static/Active)", mode_str),
                        format!("Static - OWASP: {} (Toggle)", owasp_val),
                        format!("Static - Secrets: {} (Toggle)", secrets_val),
                        format!("Bruteforce: {} (Toggle)", bruteforce_val),
                        format!("Firewall: {} (Toggle)", firewall_val),
                        "Save and Exit".to_string(),
                        "Cancel / Exit".to_string(),
                    ];

                    let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                    match ui::select_item("Pentester Settings:", &opt_refs) {
                        Some(0) => {
                            temp_pentester.mode = match temp_pentester.mode {
                                Some(uneven_core::domain::PentesterMode::Active) => Some(uneven_core::domain::PentesterMode::Static),
                                _ => Some(uneven_core::domain::PentesterMode::Active),
                            };
                        }
                        Some(1) => {
                            let mut s_cfg = temp_pentester.static_config.clone().unwrap_or_default();
                            s_cfg.owasp = Some(!owasp_val);
                            temp_pentester.static_config = Some(s_cfg);
                        }
                        Some(2) => {
                            let mut s_cfg = temp_pentester.static_config.clone().unwrap_or_default();
                            s_cfg.secrets = Some(!secrets_val);
                            temp_pentester.static_config = Some(s_cfg);
                        }
                        Some(3) => {
                            let mut b_cfg = temp_pentester.bruteforce.clone().unwrap_or_default();
                            b_cfg.enabled = Some(!bruteforce_val);
                            temp_pentester.bruteforce = Some(b_cfg);
                        }
                        Some(4) => {
                            let mut f_cfg = temp_pentester.firewall.clone().unwrap_or_default();
                            f_cfg.enabled = Some(!firewall_val);
                            temp_pentester.firewall = Some(f_cfg);
                        }
                        Some(5) => {
                            config.pentester = Some(temp_pentester);
                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            println!("\n  {} Pentester configuration saved successfully!\n", ui::green("✓"));
                            break;
                        }
                        Some(6) | None => {
                            println!("\n  Changes discarded.\n");
                            break;
                        }
                        _ => {}
                    }
                }
            } else if slash_cmd == "/scan" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                let subcommand_args = if !payload.is_empty() {
                    let mut args_vec = vec!["scan"];
                    args_vec.extend(payload.split_whitespace());
                    args_vec
                } else {
                    vec!["scan"]
                };
                println!("{}", truecolor("\n  ↳ Scanning for vulnerabilities and malicious patterns…\n", 107, 122, 150));
                let _ = spawn_subcommand(&subcommand_args).await;
                continue;
            } else if slash_cmd == "/zeroday" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                let subcommand_args = if !payload.is_empty() {
                    let mut args_vec = vec!["zeroday"];
                    args_vec.extend(payload.split_whitespace());
                    args_vec
                } else {
                    vec!["zeroday"]
                };
                println!("{}", truecolor("\n  ↳ Scanning project files for custom zero-day vulnerability flows…\n", 107, 122, 150));
                let _ = spawn_subcommand(&subcommand_args).await;
                continue;
            } else if slash_cmd == "/fix" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    let sub_parts: Vec<&str> = payload.split_whitespace().collect();
                    let mut action = "";
                    let mut val_idx = 1;

                    if sub_parts[0].to_lowercase() == "watch" || sub_parts[0].to_lowercase() == "proactive" {
                        if sub_parts.len() > 1 {
                            action = sub_parts[1];
                            val_idx = 2;
                        }
                    } else {
                        action = sub_parts[0];
                    }

                    let action_lower = action.to_lowercase();
                    if sub_parts.len() <= val_idx {
                        println!("\n  {} Usage: /fix terminal <cmd> | dirs <d1> <d2> ... | autofix <true/false> | confirm <true/false>\n", style("  ✗ ").red());
                        continue;
                    }

                    let mut w_cfg = config.watch.clone().unwrap_or_default();

                    if action_lower == "terminal" {
                        w_cfg.terminal = Some(sub_parts[val_idx..].join(" "));
                    } else if action_lower == "dirs" {
                        let dirs: Vec<String> = sub_parts[val_idx..].iter().map(|s| s.to_string()).collect();
                        w_cfg.dirs = Some(dirs);
                    } else if action_lower == "autofix" || action_lower == "enabled" {
                        if let Ok(val) = sub_parts[val_idx].parse::<bool>() {
                            w_cfg.auto_fix = Some(val);
                        } else {
                            println!("\n  {} Please provide true or false.\n", style("  ✗ ").red());
                            continue;
                        }
                    } else if action_lower == "confirm" {
                        if let Ok(val) = sub_parts[val_idx].parse::<bool>() {
                            w_cfg.confirm_before_fix = Some(val);
                        } else {
                            println!("\n  {} Please provide true or false.\n", style("  ✗ ").red());
                            continue;
                        }
                    } else {
                        println!("\n  {} Unknown action. Options: terminal, dirs, autofix, confirm\n", style("  ✗ ").red());
                        continue;
                    }

                    config.watch = Some(w_cfg.clone());
                    config.proactive = Some(uneven_core::domain::ProactiveConfig {
                        enabled: Some(w_cfg.auto_fix.unwrap_or(true)),
                        auto_fix: w_cfg.auto_fix,
                        confirm_before_fix: w_cfg.confirm_before_fix,
                        dirs: w_cfg.dirs.clone(),
                        ..Default::default()
                    });

                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }
                    println!("\n  {} Watch and Auto-Fix settings updated successfully!\n", ui::green("✓"));
                    continue;
                }

                let mut temp_watch = config.watch.clone().unwrap_or_default();
                loop {
                    let w_term = temp_watch.terminal.as_deref().unwrap_or("None");
                    let w_dirs = temp_watch.dirs.as_ref()
                        .map(|d| d.join(", "))
                        .unwrap_or_else(|| "None".to_string());
                    let w_autofix = temp_watch.auto_fix.unwrap_or(false);
                    let w_confirm = temp_watch.confirm_before_fix.unwrap_or(false);

                    let options = [
                        format!("Terminal Command: {} (Click to change)", w_term),
                        format!("Directories to Watch/Fix: {} (Click to change)", w_dirs),
                        format!("Auto Fix: {} (Toggle)", w_autofix),
                        format!("Confirm Before Fix: {} (Toggle)", w_confirm),
                        "Save and Exit".to_string(),
                        "Cancel / Exit".to_string(),
                    ];

                    let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                    match ui::select_item("Auto-Fix / Watch Settings:", &opt_refs) {
                        Some(0) => {
                            println!("\n  {} Enter Watch Terminal command (e.g. npm run dev):", ui::accent("›"));
                            print!("  › ");
                            let _ = std::io::stdout().flush();
                            let mut buf = String::new();
                            if std::io::stdin().read_line(&mut buf).is_ok() {
                                let val = buf.trim().to_string();
                                if !val.is_empty() {
                                    temp_watch.terminal = Some(val);
                                }
                            }
                        }
                        Some(1) => {
                            println!("\n  {} Enter Watch/Fix directories separated by space (e.g. ./src ./lib):", ui::accent("›"));
                            print!("  › ");
                            let _ = std::io::stdout().flush();
                            let mut buf = String::new();
                            if std::io::stdin().read_line(&mut buf).is_ok() {
                                let dirs: Vec<String> = buf.split_whitespace().map(|s| s.to_string()).collect();
                                if !dirs.is_empty() {
                                    temp_watch.dirs = Some(dirs);
                                }
                            }
                        }
                        Some(2) => {
                            temp_watch.auto_fix = Some(!w_autofix);
                        }
                        Some(3) => {
                            temp_watch.confirm_before_fix = Some(!w_confirm);
                        }
                        Some(4) => {
                            config.watch = Some(temp_watch.clone());
                            config.proactive = Some(uneven_core::domain::ProactiveConfig {
                                enabled: Some(temp_watch.auto_fix.unwrap_or(true)),
                                auto_fix: temp_watch.auto_fix,
                                confirm_before_fix: temp_watch.confirm_before_fix,
                                dirs: temp_watch.dirs.clone(),
                                ..Default::default()
                            });

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            println!("\n  {} Watch and Auto-Fix settings saved successfully!\n", ui::green("✓"));
                            break;
                        }
                        Some(5) | None => {
                            println!("\n  Changes discarded.\n");
                            break;
                        }
                        _ => {}
                    }
                }
                continue;
            } else if slash_cmd == "/urls" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    let urls: Vec<String> = payload.split_whitespace().map(|s| s.to_string()).collect();
                    let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                    k_cfg.urls = Some(urls.clone());
                    config.knowledge = Some(k_cfg);

                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }
                    println!("\n  {} Documentation URLs updated: {}\n", ui::green("✓"), urls.join(", "));
                    continue;
                }

                let mut temp_knowledge = config.knowledge.clone().unwrap_or_default();
                loop {
                    let k_urls = temp_knowledge.urls.as_ref()
                        .map(|u| u.join(", "))
                        .unwrap_or_else(|| "None".to_string());

                    let options = [
                        format!("Documentation URLs: {} (Click to change)", k_urls),
                        "Apply and return to chat".to_string(),
                        "Cancel and return to chat".to_string(),
                    ];

                    let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                    match ui::select_item("URL Settings:", &opt_refs) {
                        Some(0) => {
                            println!("\n  {} Enter documentation URLs separated by space (or press Enter to cancel):", ui::accent("›"));
                            print!("  › ");
                            let _ = std::io::stdout().flush();
                            let mut buf = String::new();
                            if std::io::stdin().read_line(&mut buf).is_ok() {
                                let urls: Vec<String> = buf.split_whitespace().map(|s| s.to_string()).collect();
                                if !urls.is_empty() {
                                    temp_knowledge.urls = Some(urls);
                                }
                            }
                        }
                        Some(1) => {
                            config.knowledge = Some(temp_knowledge.clone());

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            println!("\n  {} URL settings saved successfully!\n", ui::green("✓"));
                            break;
                        }
                        Some(2) | None => {
                            println!("\n  Changes discarded.\n");
                            break;
                        }
                        _ => {}
                    }
                }
                continue;
            } else if slash_cmd == "/dir" || slash_cmd == "/directory" || slash_cmd == "/folder" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    let path = std::path::Path::new(payload);
                    let abs_path = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        root.join(path)
                    };
                    let resolved_path = abs_path.canonicalize().unwrap_or(abs_path);
                    let resolved_str = resolved_path.to_string_lossy().to_string();

                    let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                    k_cfg.external_project = Some(resolved_str.clone());
                    config.knowledge = Some(k_cfg);

                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }

                    if let Some(files) = sync_external_project(&resolved_str) {
                        external_files_list = Some(files);
                        let ext_dir = resolved_path.clone();
                        tokio::spawn(async move {
                            let _ = spawn_subcommand_silent_in_dir(&["index"], &ext_dir).await;
                        });
                        println!("\n  {} External project path updated and synced: {}\n", ui::green("✓"), resolved_str);
                    } else {
                        println!("\n  {} Failed to sync external project: {}\n", style("✗").red(), resolved_str);
                    }
                    continue;
                }

                let mut temp_dir = config.knowledge.as_ref().and_then(|k| k.external_project.clone());
                loop {
                    let current_dir = temp_dir.as_deref().unwrap_or("None");

                    let options = [
                        format!("External Project Path: {} (Click to change)", current_dir),
                        "Apply and return to chat".to_string(),
                        "Cancel and return to chat".to_string(),
                    ];

                    let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                    match ui::select_item("External Project Settings:", &opt_refs) {
                        Some(0) => {
                            println!("\n  {} Enter the path to your external project directory (or press Enter to cancel):", ui::accent("›"));
                            print!("  › ");
                            let _ = std::io::stdout().flush();
                            let mut buf = String::new();
                            if std::io::stdin().read_line(&mut buf).is_ok() {
                                let new_path_str = buf.trim().to_string();
                                if !new_path_str.is_empty() {
                                    let path = std::path::Path::new(&new_path_str);
                                    let abs_path = if path.is_absolute() {
                                        path.to_path_buf()
                                    } else {
                                        root.join(path)
                                    };
                                    let resolved_path = abs_path.canonicalize().unwrap_or(abs_path);
                                    temp_dir = Some(resolved_path.to_string_lossy().to_string());
                                }
                            }
                        }
                        Some(1) => {
                            let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                            k_cfg.external_project = temp_dir.clone();
                            config.knowledge = Some(k_cfg);

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }

                            if let Some(ref resolved_str) = temp_dir {
                                if let Some(files) = sync_external_project(resolved_str) {
                                    external_files_list = Some(files);
                                    let ext_dir = std::path::PathBuf::from(resolved_str);
                                    tokio::spawn(async move {
                                        let _ = spawn_subcommand_silent_in_dir(&["index"], &ext_dir).await;
                                    });
                                }
                            } else {
                                external_files_list = None;
                            }

                            println!("\n  {} External project path saved and synced successfully!\n", ui::green("✓"));
                            break;
                        }
                        Some(2) | None => {
                            println!("\n  Changes discarded.\n");
                            break;
                        }
                        _ => {}
                    }
                }
                continue;
            } else if slash_cmd == "/skills" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    config.skills = Some(payload.to_string());

                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }
                    println!("\n  {} Custom skills path updated: {}\n", ui::green("✓"), payload);
                    continue;
                }

                let mut temp_skills = config.skills.clone();
                loop {
                    let current_skills = temp_skills.as_deref().unwrap_or("None");

                    let options = [
                        format!("Skills Path: {} (Click to change)", current_skills),
                        "Apply and return to chat".to_string(),
                        "Cancel and return to chat".to_string(),
                    ];

                    let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                    match ui::select_item("Custom Skills Settings:", &opt_refs) {
                        Some(0) => {
                            println!("\n  {} Enter the path to your custom skills directory (or press Enter to cancel):", ui::accent("›"));
                            print!("  › ");
                            let _ = std::io::stdout().flush();
                            let mut buf = String::new();
                            if std::io::stdin().read_line(&mut buf).is_ok() {
                                let new_path = buf.trim().to_string();
                                if !new_path.is_empty() {
                                    temp_skills = Some(new_path);
                                }
                            }
                        }
                        Some(1) => {
                            config.skills = temp_skills;

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            println!("\n  {} Custom skills path saved successfully!\n", ui::green("✓"));
                            break;
                        }
                        Some(2) | None => {
                            println!("\n  Changes discarded.\n");
                            break;
                        }
                        _ => {}
                    }
                }
                continue;
            } else if slash_cmd == "/person" || slash_cmd == "/persona" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    if payload.to_lowercase() == "default" || payload.to_lowercase() == "reset" {
                        config.persona = None;
                        println!("\n  {} Custom agent persona reset to default (Senior Engineer).\n", ui::green("✓"));
                    } else {
                        config.persona = Some(payload.to_string());
                        println!("\n  {} Custom agent persona updated: {}\n", ui::green("✓"), payload);
                    }

                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }
                    continue;
                }

                let mut temp_persona = config.persona.clone();
                loop {
                    let current_persona = temp_persona.as_deref().unwrap_or("Default (Senior Engineer)");

                    let options = [
                        format!("Agent Persona: {} (Click to change)", current_persona),
                        "Apply and return to chat".to_string(),
                        "Cancel and return to chat".to_string(),
                    ];

                    let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                    match ui::select_item("Custom Agent Persona Settings:", &opt_refs) {
                        Some(0) => {
                            println!("\n  {} Enter your custom agent persona description (or press Enter to reset to default):", ui::accent("›"));
                            print!("  › ");
                            let _ = std::io::stdout().flush();
                            let mut buf = String::new();
                            if std::io::stdin().read_line(&mut buf).is_ok() {
                                let new_persona = buf.trim().to_string();
                                if !new_persona.is_empty() {
                                    temp_persona = Some(new_persona);
                                } else {
                                    temp_persona = None;
                                }
                            }
                        }
                        Some(1) => {
                            config.persona = temp_persona;

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            println!("\n  {} Custom agent persona saved successfully!\n", ui::green("✓"));
                            break;
                        }
                        Some(2) | None => {
                            println!("\n  Changes discarded.\n");
                            break;
                        }
                        _ => {}
                    }
                }
                continue;
            } else if slash_cmd == "/max_tokens" || slash_cmd == "/max-tokens" || slash_cmd == "/maxtokens" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    if let Ok(tokens) = payload.parse::<usize>() {
                        config.brain.max_tokens = tokens;

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }
                        println!("\n  {} Max tokens updated: {}\n", ui::green("✓"), tokens);
                    } else {
                        println!("\n  {} Invalid token number: {}\n", style("✗").red(), payload);
                    }
                    continue;
                }

                let mut temp_tokens = config.brain.max_tokens;
                loop {
                    let options = [
                        format!("Max Tokens: {} (Click to change)", temp_tokens),
                        "Apply and return to chat".to_string(),
                        "Cancel and return to chat".to_string(),
                    ];

                    let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                    match ui::select_item("Max Tokens Settings:", &opt_refs) {
                        Some(0) => {
                            println!("\n  {} Enter maximum output tokens (default: 2048):", ui::accent("›"));
                            print!("  › ");
                            let _ = std::io::stdout().flush();
                            let mut buf = String::new();
                            if std::io::stdin().read_line(&mut buf).is_ok() {
                                let val = buf.trim();
                                if !val.is_empty() {
                                    if let Ok(tokens) = val.parse::<usize>() {
                                        temp_tokens = tokens;
                                    } else {
                                        println!("\n  {} Invalid number: {}\n", style("✗").red(), val);
                                    }
                                }
                            }
                        }
                        Some(1) => {
                            config.brain.max_tokens = temp_tokens;

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            println!("\n  {} Max tokens saved successfully!\n", ui::green("✓"));
                            break;
                        }
                        Some(2) | None => {
                            println!("\n  Changes discarded.\n");
                            break;
                        }
                        _ => {}
                    }
                }
                continue;
            } else if slash_cmd == "/effort" || slash_cmd == "/reasoning" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    if payload.to_lowercase() == "default" || payload.to_lowercase() == "reset" || payload.to_lowercase() == "none" {
                        config.brain.reasoning_effort = None;
                        println!("\n  {} Reasoning effort reset to default.\n", ui::green("✓"));
                    } else {
                        config.brain.reasoning_effort = Some(payload.to_string());
                        println!("\n  {} Reasoning effort updated: {}\n", ui::green("✓"), payload);
                    }

                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }
                    continue;
                }

                let mut temp_effort = config.brain.reasoning_effort.clone();
                loop {
                    let current_effort = temp_effort.as_deref().unwrap_or("Default (None / Provider Managed)");

                    let options = [
                        format!("Reasoning Effort: {} (Click to change)", current_effort),
                        "Apply and return to chat".to_string(),
                        "Cancel and return to chat".to_string(),
                    ];

                    let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                    match ui::select_item("Reasoning Effort Settings:", &opt_refs) {
                        Some(0) => {
                            println!("\n  {} Enter reasoning effort (low, medium, high, none, or custom tokens limit):", ui::accent("›"));
                            print!("  › ");
                            let _ = std::io::stdout().flush();
                            let mut buf = String::new();
                            if std::io::stdin().read_line(&mut buf).is_ok() {
                                let new_effort = buf.trim().to_string();
                                if !new_effort.is_empty() && new_effort.to_lowercase() != "none" {
                                    temp_effort = Some(new_effort);
                                } else {
                                    temp_effort = None;
                                }
                            }
                        }
                        Some(1) => {
                            config.brain.reasoning_effort = temp_effort;

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            println!("\n  {} Reasoning effort saved successfully!\n", ui::green("✓"));
                            break;
                        }
                        Some(2) | None => {
                            println!("\n  Changes discarded.\n");
                            break;
                        }
                        _ => {}
                    }
                }
                continue;
            } else if slash_cmd == "/agents" || slash_cmd == "/multi_agents" || slash_cmd == "/multi-agents" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    let lower = payload.to_lowercase();
                    let active = if lower == "on" || lower == "true" || lower == "enable" {
                        true
                    } else if lower == "off" || lower == "false" || lower == "disable" {
                        false
                    } else {
                        println!("\n  {} Invalid option for multi-agent mode: {}. Use 'on' or 'off'.\n", style("✗").red(), payload);
                        continue;
                    };

                    config.multi_agents = Some(active);
                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&config_path, config_json);
                    }
                    let msg = if active { "enabled" } else { "disabled" };
                    println!("\n  {} Multi-agent mode {} successfully!\n", ui::green("✓"), msg);
                    continue;
                }

                let mut temp_agents = config.multi_agents.unwrap_or(false);
                loop {
                    let current_status = if temp_agents { "Enabled" } else { "Disabled" };
                    let options = [
                        format!("Multi-Agent Mode: {} (Click to toggle)", current_status),
                        "Apply and return to chat".to_string(),
                        "Cancel and return to chat".to_string(),
                    ];
                    let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
                    match ui::select_item("Multi-Agent Mode Settings:", &opt_refs) {
                        Some(0) => {
                            temp_agents = !temp_agents;
                        }
                        Some(1) => {
                            config.multi_agents = Some(temp_agents);
                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            let msg = if temp_agents { "enabled" } else { "disabled" };
                            println!("\n  {} Multi-agent mode {} successfully!\n", ui::green("✓"), msg);
                            break;
                        }
                        Some(2) | None => {
                            println!("\n  Changes discarded.\n");
                            break;
                        }
                        _ => {}
                    }
                }
                continue;
            } else if slash_cmd == "/stats" || slash_cmd == "/cost" {
                let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                if !payload.is_empty() {
                    let mut stats = uneven_core::infrastructure::llm::pricing::load_stats(&root);
                    let mut is_limit_update = false;
                    let target_val = payload.strip_prefix("limit").unwrap_or(payload).trim();
                    if let Ok(val) = target_val.parse::<f64>() {
                        stats.monthly_limit_usd = val;
                        let _ = uneven_core::infrastructure::llm::pricing::save_stats(&root, &stats);
                        println!("\n  {} Monthly budget limit updated to ${:.2} USD!\n", ui::green("✓"), val);
                        is_limit_update = true;
                    }
                    if is_limit_update {
                        continue;
                    }
                }
                let _ = super::stats::run().await;
                continue;
            } else if slash_cmd == "/" {
                let menu_options = [
                    "/models     AI Models",
                    "/export     Export Environment Variable",
                    "/analyze    Database Analysis",
                    "/pentest    Active Pentest",
                    "/scan       Vulnerability Scan",
                    "/zeroday    Zero-Day Scan",
                    "/fix        Auto-Fix / Watch Settings",
                    "/urls       Documentation URLs",
                    "/skills     Custom Skills Path",
                    "/person     Custom Agent Persona",
                    "/max_tokens Max Output Tokens",
                    "/effort     Reasoning Effort Settings",
                    "/dir        External Project Path",
                    "/agents     Multi-Agent Settings",
                    "/stats      Token Usage Stats",
                    "ESC         Cancel / Exit",
                ];

                match ui::select_item("Slash Command Menu:", &menu_options) {
                    Some(0) => {
                        let list_options = [
                            "GPT 5.5                          | Cloud model",
                            "GPT 4o                           | Cloud model",
                            "Gemini 3.5 Flash                 | Cloud model",
                            "Gemini 2.5 Flash                 | Cloud model",
                            "Claude 4.8 Opus                  | Cloud model",
                            "Claude 4.6 Sonnet                | Cloud model",
                            "DeepSeek V4 Flash                | Cloud model",
                            "DeepSeek V4 Pro                  | Cloud model",
                            "Llama 3.2 1B Q4                  | Local model",
                            "Qwen 2.5 1.5B Q8                 | Local model",
                            "DeepSeek R1 Distill 8B Q4        | Local model",
                            "Phi-4 14B Q4                     | Local model",
                            "Others (Type custom spec)        | Custom model",
                        ];

                        let mut selected_opt = None;
                        if let Some(idx) = ui::select_item("Choose AI Model:", &list_options) {
                            if idx == 12 {
                                println!("\n  {} Enter custom model spec (e.g. deepseek-r1-8b-q4 or openai/gpt-4o):", ui::accent("›"));
                                print!("  › ");
                                let _ = std::io::stdout().flush();
                                let mut spec_buf = String::new();
                                if std::io::stdin().read_line(&mut spec_buf).is_ok() {
                                    let spec = spec_buf.trim().to_string();
                                    if !spec.is_empty() {
                                        match parse_multi_brains(&spec) {
                                            Err(e) => {
                                                println!("\n  {} {}\n", style("  ✗ ").red(), e);
                                            }
                                            Ok(parsed_brains) => {
                                                if !parsed_brains.is_empty() && parsed_brains[0].0.is_none() {
                                                    let (_, m, p) = &parsed_brains[0];
                                                    selected_opt = Some((m.clone(), p.clone()));
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                let mapped = match idx {
                                    0 => ("gpt-5.5", "openai"),
                                    1 => ("gpt-4o", "openai"),
                                    2 => ("gemini-3.5-flash", "gemini"),
                                    3 => ("gemini-2.5-flash", "gemini"),
                                    4 => ("claude-opus-4-8", "claude"),
                                    5 => ("claude-sonnet-4-6", "claude"),
                                    6 => ("deepseek-v4-flash", "deepseek"),
                                    7 => ("deepseek-v4-pro", "deepseek"),
                                    8 => ("llama-3.2-1b-q4", "local"),
                                    9 => ("qwen-2.5-1.5b-q8", "local"),
                                    10 => ("deepseek-r1-8b-q4", "local"),
                                    _ => ("phi-4-14b-q4", "local"),
                                };
                                selected_opt = Some((mapped.0.to_string(), mapped.1.to_string()));
                            }

                            if let Some((m, p)) = selected_opt {
                                if p == "local" {
                                    if let Err(e) = ensure_local_model_files(&m).await {
                                        println!("\n  {} {}\n", style("✗").red(), e);
                                        continue;
                                    }
                                }
                                config.brain.provider = p.clone();
                                config.brain.model = m.clone();
                                config.brains = None;
                                config.routing = None;

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                if let Some(parent) = config_path.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                    let _ = std::fs::write(&config_path, config_json);
                                }

                                let threads = config.brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
                                let _ = init_llm(&config.brain.provider, &config.brain.model, threads).await;

                                println!("\n  {} Active Brain updated to: {} / {}\n", ui::green("✓"), p, m);
                            }
                        }
                    }
                    Some(1) => {
                        println!("\n  Usage: /export <VARIABLE_NAME> [value]\n");
                        println!("  {} Enter environment variable name (or press Enter to cancel):", ui::accent("›"));
                        print!("  › ");
                        let _ = std::io::stdout().flush();
                        let mut key_buf = String::new();
                        if std::io::stdin().read_line(&mut key_buf).is_ok() {
                            let key = key_buf.trim().to_string();
                            if !key.is_empty() {
                                println!("  {} Enter value for variable {} (or press Enter to cancel):", ui::accent("›"), key);
                                print!("  › ");
                                let _ = std::io::stdout().flush();
                                let mut val_buf = String::new();
                                if std::io::stdin().read_line(&mut val_buf).is_ok() {
                                    let val = val_buf.trim().trim_matches(|c| c == '\'' || c == '"').to_string();
                                    if !val.is_empty() {
                                        let global_env_path = uneven_core::infrastructure::utils::paths::global_bin_dir().join("env");
                                        if let Some(parent) = global_env_path.parent() {
                                            let _ = std::fs::create_dir_all(parent);
                                        }

                                        let mut lines = Vec::new();
                                        if global_env_path.exists() {
                                            if let Ok(content) = std::fs::read_to_string(&global_env_path) {
                                                for line in content.lines() {
                                                    let l_trim = line.trim();
                                                    if !l_trim.starts_with(&format!("{}=", key)) && !l_trim.is_empty() {
                                                        lines.push(line.to_string());
                                                    }
                                                }
                                            }
                                        }
                                        lines.push(format!("{}={}", key, val));
                                        if std::fs::write(&global_env_path, lines.join("\n") + "\n").is_ok() {
                                            std::env::set_var(&key, val);
                                            println!("\n  {} {} successfully saved to global environment!\n", ui::green("✓"), key);
                                        } else {
                                            println!("\n  {} Failed to save key to global environment\n", style("✗").red());
                                        }
                                    } else {
                                        println!("\n  {} Operation cancelled.\n", style("  ✗ ").red());
                                    }
                                }
                            } else {
                                println!("\n  {} Operation cancelled.\n", style("  ✗ ").red());
                            }
                        }
                    }
                    Some(2) => {
                        let analyze_options = [
                            "/analyze db        Database URL",
                            "/analyze tables    Authorized Tables",
                            "/analyze mask      Column/Table Masking",
                            "ESC                Back",
                        ];

                        match ui::select_item("Database Analysis Settings (/analyze):", &analyze_options) {
                            Some(0) => {
                                println!("\n  {} Enter Database URL (or press Enter to cancel):", ui::accent("›"));
                                print!("  › ");
                                let _ = std::io::stdout().flush();
                                let mut db_buf = String::new();
                                if std::io::stdin().read_line(&mut db_buf).is_ok() {
                                    let db_url = db_buf.trim().to_string();
                                    if !db_url.is_empty() {
                                        let mut db_cfg = config.knowledge.as_ref().and_then(|k| k.db.clone()).unwrap_or_default();
                                        db_cfg.url = Some(db_url.clone());
                                        let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                                        k_cfg.db = Some(db_cfg);
                                        config.knowledge = Some(k_cfg);

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                        if let Some(parent) = config_path.parent() {
                                            let _ = std::fs::create_dir_all(parent);
                                        }
                                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                            let _ = std::fs::write(&config_path, config_json);
                                        }
                                        println!("\n  {} Database URL configured to: {}\n", ui::green("✓"), db_url);
                                    } else {
                                        println!("\n  {} Operation cancelled.\n", style("  ✗ ").red());
                                    }
                                }
                            }
                            Some(1) => {
                                println!("\n  {} Enter authorized tables separated by space (or press Enter to cancel):", ui::accent("›"));
                                print!("  › ");
                                let _ = std::io::stdout().flush();
                                let mut tbl_buf = String::new();
                                if std::io::stdin().read_line(&mut tbl_buf).is_ok() {
                                    let tables: Vec<String> = tbl_buf.split_whitespace().map(|s| s.to_string()).collect();
                                    if !tables.is_empty() {
                                        let mut db_cfg = config.knowledge.as_ref().and_then(|k| k.db.clone()).unwrap_or_default();
                                        db_cfg.allowed_tables = Some(tables.clone());
                                        let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                                        k_cfg.db = Some(db_cfg);
                                        config.knowledge = Some(k_cfg);

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                        if let Some(parent) = config_path.parent() {
                                            let _ = std::fs::create_dir_all(parent);
                                        }
                                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                            let _ = std::fs::write(&config_path, config_json);
                                        }
                                        println!("\n  {} Authorized tables updated: {}\n", ui::green("✓"), tables.join(", "));
                                    } else {
                                        println!("\n  {} Operation cancelled.\n", style("  ✗ ").red());
                                    }
                                }
                            }
                            Some(2) => {
                                let mask_options = [
                                    "Mask Columns",
                                    "Block Tables",
                                    "ESC            Back",
                                ];
                                match ui::select_item("Masking Options:", &mask_options) {
                                    Some(0) => {
                                        println!("\n  {} Enter columns to mask separated by space (or press Enter to cancel):", ui::accent("›"));
                                        print!("  › ");
                                        let _ = std::io::stdout().flush();
                                        let mut cols_buf = String::new();
                                        if std::io::stdin().read_line(&mut cols_buf).is_ok() {
                                            let columns: Vec<String> = cols_buf.split_whitespace().map(|s| s.to_string()).collect();
                                            if !columns.is_empty() {
                                                config.security.masked_columns = columns.clone();

                                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                                if let Some(parent) = config_path.parent() {
                                                    let _ = std::fs::create_dir_all(parent);
                                                }
                                                if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                                    let _ = std::fs::write(&config_path, config_json);
                                                }
                                                println!("\n  {} Masked columns updated: {}\n", ui::green("✓"), columns.join(", "));
                                            } else {
                                                println!("\n  {} Operation cancelled.\n", style("  ✗ ").red());
                                            }
                                        }
                                    }
                                    Some(1) => {
                                        println!("\n  {} Enter tables to block separated by space (or press Enter to cancel):", ui::accent("›"));
                                        print!("  › ");
                                        let _ = std::io::stdout().flush();
                                        let mut tbls_buf = String::new();
                                        if std::io::stdin().read_line(&mut tbls_buf).is_ok() {
                                            let tables: Vec<String> = tbls_buf.split_whitespace().map(|s| s.to_string()).collect();
                                            if !tables.is_empty() {
                                                config.security.blocked_tables = tables.clone();

                                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                                if let Some(parent) = config_path.parent() {
                                                    let _ = std::fs::create_dir_all(parent);
                                                }
                                                if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                                    let _ = std::fs::write(&config_path, config_json);
                                                }
                                                println!("\n  {} Blocked tables updated: {}\n", ui::green("✓"), tables.join(", "));
                                            } else {
                                                println!("\n  {} Operation cancelled.\n", style("  ✗ ").red());
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(3) => {
                        let mut temp_pentester = config.pentester.clone().unwrap_or_default();
                        loop {
                            let mode_str = match temp_pentester.mode {
                                Some(uneven_core::domain::PentesterMode::Active) => "Active",
                                _ => "Static",
                            };

                            let owasp_val = temp_pentester.static_config.as_ref()
                                .and_then(|s| s.owasp)
                                .unwrap_or(false);

                            let secrets_val = temp_pentester.static_config.as_ref()
                                .and_then(|s| s.secrets)
                                .unwrap_or(false);

                            let bruteforce_val = temp_pentester.bruteforce.as_ref()
                                .and_then(|b| b.enabled)
                                .unwrap_or(false);

                            let firewall_val = temp_pentester.firewall.as_ref()
                                .and_then(|f| f.enabled)
                                .unwrap_or(false);

                            let options = [
                                format!("Mode: {} (Toggle Static/Active)", mode_str),
                                format!("Static - OWASP: {} (Toggle)", owasp_val),
                                format!("Static - Secrets: {} (Toggle)", secrets_val),
                                format!("Bruteforce: {} (Toggle)", bruteforce_val),
                                format!("Firewall: {} (Toggle)", firewall_val),
                                "Apply and return to chat".to_string(),
                                "Cancel and return to chat".to_string(),
                            ];

                            let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                            match ui::select_item("Pentester Settings:", &opt_refs) {
                                Some(0) => {
                                    temp_pentester.mode = match temp_pentester.mode {
                                        Some(uneven_core::domain::PentesterMode::Active) => Some(uneven_core::domain::PentesterMode::Static),
                                        _ => Some(uneven_core::domain::PentesterMode::Active),
                                    };
                                }
                                Some(1) => {
                                    let mut s_cfg = temp_pentester.static_config.clone().unwrap_or_default();
                                    s_cfg.owasp = Some(!owasp_val);
                                    temp_pentester.static_config = Some(s_cfg);
                                }
                                Some(2) => {
                                    let mut s_cfg = temp_pentester.static_config.clone().unwrap_or_default();
                                    s_cfg.secrets = Some(!secrets_val);
                                    temp_pentester.static_config = Some(s_cfg);
                                }
                                Some(3) => {
                                    let mut b_cfg = temp_pentester.bruteforce.clone().unwrap_or_default();
                                    b_cfg.enabled = Some(!bruteforce_val);
                                    temp_pentester.bruteforce = Some(b_cfg);
                                }
                                Some(4) => {
                                    let mut f_cfg = temp_pentester.firewall.clone().unwrap_or_default();
                                    f_cfg.enabled = Some(!firewall_val);
                                    temp_pentester.firewall = Some(f_cfg);
                                }
                                Some(5) => {
                                    config.pentester = Some(temp_pentester);
                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }
                                    println!("\n  {} Pentester settings saved successfully!\n", ui::green("✓"));
                                    break;
                                }
                                Some(6) | None => {
                                    println!("\n  Changes discarded.\n");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(4) => {
                        println!("{}", truecolor("\n  ↳ Scanning for vulnerabilities and malicious patterns…\n", 107, 122, 150));
                        let _ = spawn_subcommand(&["scan"]).await;
                    }
                    Some(5) => {
                        println!("{}", truecolor("\n  ↳ Scanning project files for custom zero-day vulnerability flows…\n", 107, 122, 150));
                        let _ = spawn_subcommand(&["zeroday"]).await;
                    }
                    Some(6) => {
                        let mut temp_watch = config.watch.clone().unwrap_or_default();
                        if temp_watch.dirs.is_none() || temp_watch.dirs.as_ref().map(|d| d.is_empty()).unwrap_or(false) {
                            temp_watch.dirs = Some(vec!["./".to_string()]);
                        }
                        loop {
                            let w_term = temp_watch.terminal.as_deref().unwrap_or("None");
                            let w_dirs = temp_watch.dirs.as_ref()
                                .map(|d| d.join(", "))
                                .unwrap_or_else(|| "None".to_string());
                            let w_autofix = temp_watch.auto_fix.unwrap_or(false);
                            let w_confirm = temp_watch.confirm_before_fix.unwrap_or(false);

                            let options = [
                                format!("Terminal Command: {} (Click to change)", w_term),
                                format!("Directories to Watch/Fix: {} (Click to change)", w_dirs),
                                format!("Auto Fix: {} (Toggle)", w_autofix),
                                format!("Confirm Before Fix: {} (Toggle)", w_confirm),
                                "Apply and return to chat".to_string(),
                                "Cancel and return to chat".to_string(),
                            ];

                            let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                            match ui::select_item("Auto-Fix / Watch Settings:", &opt_refs) {
                                Some(0) => {
                                    println!("\n  {} Enter Watch Terminal command (or press Enter to cancel):", ui::accent("›"));
                                    print!("  › ");
                                    let _ = std::io::stdout().flush();
                                    let mut buf = String::new();
                                    if std::io::stdin().read_line(&mut buf).is_ok() {
                                        let val = buf.trim().to_string();
                                        if !val.is_empty() {
                                            temp_watch.terminal = Some(val);
                                        } else {
                                            println!("\n  {} Operation cancelled.\n", style("  ✗ ").red());
                                        }
                                    }
                                }
                                Some(1) => {
                                    println!("\n  {} Enter Watch/Fix directories separated by space (or press Enter to cancel):", ui::accent("›"));
                                    print!("  › ");
                                    let _ = std::io::stdout().flush();
                                    let mut buf = String::new();
                                    if std::io::stdin().read_line(&mut buf).is_ok() {
                                        let dirs: Vec<String> = buf.split_whitespace().map(|s| s.to_string()).collect();
                                        if !dirs.is_empty() {
                                            temp_watch.dirs = Some(dirs);
                                        } else {
                                            println!("\n  {} Operation cancelled.\n", style("  ✗ ").red());
                                        }
                                    }
                                }
                                Some(2) => {
                                    temp_watch.auto_fix = Some(!w_autofix);
                                }
                                Some(3) => {
                                    temp_watch.confirm_before_fix = Some(!w_confirm);
                                }
                                Some(4) => {
                                    config.watch = Some(temp_watch.clone());
                                    config.proactive = Some(uneven_core::domain::ProactiveConfig {
                                        enabled: Some(temp_watch.auto_fix.unwrap_or(true)),
                                        auto_fix: temp_watch.auto_fix,
                                        confirm_before_fix: temp_watch.confirm_before_fix,
                                        dirs: temp_watch.dirs.clone(),
                                        ..Default::default()
                                    });

                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }
                                    println!("\n  {} Watch and Auto-Fix settings saved successfully!\n", ui::green("✓"));
                                    break;
                                }
                                Some(5) | None => {
                                    println!("\n  Changes discarded.\n");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(7) => {
                        let mut temp_knowledge = config.knowledge.clone().unwrap_or_default();
                        loop {
                            let k_urls = temp_knowledge.urls.as_ref()
                                .map(|u| u.join(", "))
                                .unwrap_or_else(|| "None".to_string());

                            let options = [
                                format!("Documentation URLs: {} (Click to change)", k_urls),
                                "Apply and return to chat".to_string(),
                                "Cancel and return to chat".to_string(),
                            ];

                            let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                            match ui::select_item("URL Settings:", &opt_refs) {
                                Some(0) => {
                                    println!("\n  {} Enter documentation URLs separated by space (or press Enter to cancel):", ui::accent("›"));
                                    print!("  › ");
                                    let _ = std::io::stdout().flush();
                                    let mut buf = String::new();
                                    if std::io::stdin().read_line(&mut buf).is_ok() {
                                        let urls: Vec<String> = buf.split_whitespace().map(|s| s.to_string()).collect();
                                        if !urls.is_empty() {
                                            temp_knowledge.urls = Some(urls);
                                        }
                                    }
                                }
                                Some(1) => {
                                    config.knowledge = Some(temp_knowledge.clone());

                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }
                                    println!("\n  {} URL settings saved successfully!\n", ui::green("✓"));
                                    break;
                                }
                                Some(2) | None => {
                                    println!("\n  Changes discarded.\n");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(8) => {
                        let mut temp_skills = config.skills.clone();
                        loop {
                            let current_skills = temp_skills.as_deref().unwrap_or("None");

                            let options = [
                                format!("Skills Path: {} (Click to change)", current_skills),
                                "Apply and return to chat".to_string(),
                                "Cancel and return to chat".to_string(),
                            ];

                            let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                            match ui::select_item("Custom Skills Settings:", &opt_refs) {
                                Some(0) => {
                                    println!("\n  {} Enter the path to your custom skills directory (or press Enter to cancel):", ui::accent("›"));
                                    print!("  › ");
                                    let _ = std::io::stdout().flush();
                                    let mut buf = String::new();
                                    if std::io::stdin().read_line(&mut buf).is_ok() {
                                        let new_path = buf.trim().to_string();
                                        if !new_path.is_empty() {
                                            temp_skills = Some(new_path);
                                        }
                                    }
                                }
                                Some(1) => {
                                    config.skills = temp_skills;

                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }
                                    println!("\n  {} Custom skills path saved successfully!\n", ui::green("✓"));
                                    break;
                                }
                                Some(2) | None => {
                                    println!("\n  Changes discarded.\n");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(9) => {
                        let mut temp_persona = config.persona.clone();
                        loop {
                            let current_persona = temp_persona.as_deref().unwrap_or("Default (Senior Engineer)");

                            let options = [
                                format!("Agent Persona: {} (Click to change)", current_persona),
                                "Apply and return to chat".to_string(),
                                "Cancel and return to chat".to_string(),
                            ];

                            let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                            match ui::select_item("Custom Agent Persona Settings:", &opt_refs) {
                                Some(0) => {
                                    println!("\n  {} Enter your custom agent persona description (or press Enter to reset to default):", ui::accent("›"));
                                    print!("  › ");
                                    let _ = std::io::stdout().flush();
                                    let mut buf = String::new();
                                    if std::io::stdin().read_line(&mut buf).is_ok() {
                                        let new_persona = buf.trim().to_string();
                                        if !new_persona.is_empty() {
                                            temp_persona = Some(new_persona);
                                        } else {
                                            temp_persona = None;
                                        }
                                    }
                                }
                                Some(1) => {
                                    config.persona = temp_persona;

                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }
                                    println!("\n  {} Custom agent persona saved successfully!\n", ui::green("✓"));
                                    break;
                                }
                                Some(2) | None => {
                                    println!("\n  Changes discarded.\n");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(10) => {
                        let mut temp_tokens = config.brain.max_tokens;
                        loop {
                            let options = [
                                format!("Max Tokens: {} (Click to change)", temp_tokens),
                                "Apply and return to chat".to_string(),
                                "Cancel and return to chat".to_string(),
                            ];

                            let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                            match ui::select_item("Max Tokens Settings:", &opt_refs) {
                                Some(0) => {
                                    println!("\n  {} Enter maximum output tokens (default: 2048):", ui::accent("›"));
                                    print!("  › ");
                                    let _ = std::io::stdout().flush();
                                    let mut buf = String::new();
                                    if std::io::stdin().read_line(&mut buf).is_ok() {
                                        let val = buf.trim();
                                        if !val.is_empty() {
                                            if let Ok(tokens) = val.parse::<usize>() {
                                                temp_tokens = tokens;
                                            } else {
                                                println!("\n  {} Invalid number: {}\n", style("✗").red(), val);
                                            }
                                        }
                                    }
                                }
                                Some(1) => {
                                    config.brain.max_tokens = temp_tokens;

                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }
                                    println!("\n  {} Max tokens saved successfully!\n", ui::green("✓"));
                                    break;
                                }
                                Some(2) | None => {
                                    println!("\n  Changes discarded.\n");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(11) => {
                        let mut temp_effort = config.brain.reasoning_effort.clone();
                        loop {
                            let current_effort = temp_effort.as_deref().unwrap_or("Default (None / Provider Managed)");

                            let options = [
                                format!("Reasoning Effort: {} (Click to change)", current_effort),
                                "Apply and return to chat".to_string(),
                                "Cancel and return to chat".to_string(),
                            ];

                            let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                            match ui::select_item("Reasoning Effort Settings:", &opt_refs) {
                                Some(0) => {
                                    println!("\n  {} Enter reasoning effort (low, medium, high, none, or custom tokens limit):", ui::accent("›"));
                                    print!("  › ");
                                    let _ = std::io::stdout().flush();
                                    let mut buf = String::new();
                                    if std::io::stdin().read_line(&mut buf).is_ok() {
                                        let new_effort = buf.trim().to_string();
                                        if !new_effort.is_empty() && new_effort.to_lowercase() != "none" {
                                            temp_effort = Some(new_effort);
                                        } else {
                                            temp_effort = None;
                                        }
                                    }
                                }
                                Some(1) => {
                                    config.brain.reasoning_effort = temp_effort;

                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }
                                    println!("\n  {} Reasoning effort saved successfully!\n", ui::green("✓"));
                                    break;
                                }
                                Some(2) | None => {
                                    println!("\n  Changes discarded.\n");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(12) => {
                        let mut temp_dir = config.knowledge.as_ref().and_then(|k| k.external_project.clone());
                        loop {
                            let current_dir = temp_dir.as_deref().unwrap_or("None");

                            let options = [
                                format!("External Project Path: {} (Click to change)", current_dir),
                                "Apply and return to chat".to_string(),
                                "Cancel and return to chat".to_string(),
                            ];

                            let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

                            match ui::select_item("External Project Settings:", &opt_refs) {
                                Some(0) => {
                                    println!("\n  {} Enter the path to your external project directory (or press Enter to cancel):", ui::accent("›"));
                                    print!("  › ");
                                    let _ = std::io::stdout().flush();
                                    let mut buf = String::new();
                                    if std::io::stdin().read_line(&mut buf).is_ok() {
                                        let new_path_str = buf.trim().to_string();
                                        if !new_path_str.is_empty() {
                                            let path = std::path::Path::new(&new_path_str);
                                            let abs_path = if path.is_absolute() {
                                                path.to_path_buf()
                                            } else {
                                                root.join(path)
                                            };
                                            let resolved_path = abs_path.canonicalize().unwrap_or(abs_path);
                                            temp_dir = Some(resolved_path.to_string_lossy().to_string());
                                        }
                                    }
                                }
                                Some(1) => {
                                    let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                                    k_cfg.external_project = temp_dir.clone();
                                    config.knowledge = Some(k_cfg);

                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }

                                    if let Some(ref resolved_str) = temp_dir {
                                        if let Some(files) = sync_external_project(resolved_str) {
                                            external_files_list = Some(files);
                                            let ext_dir = std::path::PathBuf::from(resolved_str);
                                            tokio::spawn(async move {
                                                let _ = spawn_subcommand_silent_in_dir(&["index"], &ext_dir).await;
                                            });
                                        }
                                    } else {
                                        external_files_list = None;
                                    }

                                    println!("\n  {} External project path saved and synced successfully!\n", ui::green("✓"));
                                    break;
                                }
                                Some(2) | None => {
                                    println!("\n  Changes discarded.\n");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(13) => {
                        let mut temp_agents = config.multi_agents.unwrap_or(false);
                        loop {
                            let current_status = if temp_agents { "Enabled" } else { "Disabled" };
                            let options = [
                                format!("Multi-Agent Mode: {} (Click to toggle)", current_status),
                                "Apply and return to chat".to_string(),
                                "Cancel and return to chat".to_string(),
                            ];
                            let opt_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
                            match ui::select_item("Multi-Agent Mode Settings:", &opt_refs) {
                                Some(0) => {
                                    temp_agents = !temp_agents;
                                }
                                Some(1) => {
                                    config.multi_agents = Some(temp_agents);
                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                        let _ = std::fs::write(&config_path, config_json);
                                    }
                                    let msg = if temp_agents { "enabled" } else { "disabled" };
                                    println!("\n  {} Multi-agent mode {} successfully!\n", ui::green("✓"), msg);
                                    break;
                                }
                                Some(2) | None => {
                                    println!("\n  Changes discarded.\n");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(14) => {
                        let _ = super::stats::run().await;
                    }
                    _ => {}
                }
            } else {
                println!("\n  {} Unknown slash command. Available commands: /models, /model, /export, /analyze, /pentest, /scan, /zeroday, /fix, /urls, /skills, /person, /max_tokens, /effort, /dir, /agents, /stats\n", style("✗").red());
            }
            continue;
        }

        let tokens = tokenize_shell_input(trimmed);
        if tokens.is_empty() {
            continue;
        }

        let cmd = tokens[0].to_lowercase();

        let is_subprocess_cmd = matches!(
            cmd.as_str(),
            "init"
                | "info"
                | "start"
                | "watch"
                | "index"
                | "chat"
                | "autofix"
                | "analyze"
                | "review"
                | "pentest"
                | "scan"
                | "test"
                | "undo"
                | "restore"
                | "diff"
                | "ci"
                | "remote-shell"
                | "log"
                | "reset"
                | "license"
                | "update"
        );

        if is_subprocess_cmd {
            let args = if cmd == "analyze" {
                let input_after_analyze = trimmed.strip_prefix("analyze").unwrap_or("").trim();
                parse_analyze_input(input_after_analyze)
            } else {
                tokens.clone()
            };

            let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

            // Elegant execution message
            let action_desc = match cmd.as_str() {
                "init" => "Initializing Uneven…",
                "info" => "Loading system diagnostics…",
                "start" => "Starting Uneven agent…",
                "watch" => "Starting watcher… Press Ctrl+C to stop and return to the shell.",
                "index" => "Indexing your project…",
                "chat" => "Starting chat mode…",
                "autofix" => "Starting proactive autofix service…",
                "analyze" => "Running database analysis and BI generation…",
                "review" => "Running code review…",
                "pentest" => "Running security pentest…",
                "scan" => "Scanning for vulnerabilities and malicious patterns…",
                "test" => "Running project tests…",
                "undo" => "Reverting last fix…",
                "restore" => "Restoring file snapshot…",
                "diff" => "Reviewing pending fix suggestions…",
                "ci" => "Running CI pipeline…",
                "remote-shell" => "Starting remote shell server…",
                "log" => "Loading activity log…",
                "reset" => "Clearing memory and starting fresh…",
                "license" => "Managing Uneven license…",
                "update" => "Checking for updates…",
                _ => "Running command…",
            };

            println!("\n{}", truecolor(&format!("  ↳ {}", action_desc), 107, 122, 150));
            println!();

            let code = spawn_subcommand(&args_refs).await.unwrap_or(1);

            // Success hint if code == 0
            if code == 0 {
                let hint = match cmd.as_str() {
                    "index" => Some("Project indexed. Try \"scan\" or ask anything about your code."),
                    "scan" => Some("Scan complete. Use \"watch\" to monitor errors in real time."),
                    "watch" => Some("Watcher stopped. Back to interactive mode."),
                    "review" => Some("Review done. Use \"watch\" to catch live errors or \"undo\" to revert the last fix."),
                    "undo" => Some("Fix reverted. The code is back to its previous state."),
                    _ => None,
                };
                if let Some(h) = hint {
                    println!("\n{} {}", truecolor("  › ", 0, 255, 140), truecolor(h, 107, 122, 150));
                    println!();
                }
            }

            continue;
        }

        let is_inline_cmd = matches!(
            cmd.as_str(),
            "exit" | "quit" | "help" | "ask" | "askf" | "explain" | "docs"
        );

        let mut classifier_tokens = 0;
        let mut elapsed_classify = std::time::Duration::from_secs(0);

        let (intent, message) = if is_inline_cmd {
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
                paths: Vec::new(),
                report: false,
                tasks: None,
            };

            match cmd.as_str() {
                "exit" | "quit" => {
                    intent.action = ShellAction::Exit;
                }
                "help" => {
                    intent.action = ShellAction::Help;
                }
                "ask" => {
                    intent.action = ShellAction::Ask;
                    let raw_args = trimmed.strip_prefix("ask").unwrap_or("").trim();
                    let question = if (raw_args.starts_with('"') && raw_args.ends_with('"'))
                        || (raw_args.starts_with('\'') && raw_args.ends_with('\''))
                    {
                        if raw_args.len() >= 2 {
                            &raw_args[1..raw_args.len() - 1]
                        } else {
                            raw_args
                        }
                    } else {
                        raw_args
                    };
                    intent.question = Some(question.to_string());
                }
                "askf" => {
                    intent.action = ShellAction::Askf;
                    let raw_args = trimmed.strip_prefix("askf").unwrap_or("").trim();
                    let task = if (raw_args.starts_with('"') && raw_args.ends_with('"'))
                        || (raw_args.starts_with('\'') && raw_args.ends_with('\''))
                    {
                        if raw_args.len() >= 2 {
                            &raw_args[1..raw_args.len() - 1]
                        } else {
                            raw_args
                        }
                    } else {
                        raw_args
                    };
                    intent.task = Some(task.to_string());
                }
                "explain" => {
                    intent.action = ShellAction::Explain;
                    let raw_args = trimmed.strip_prefix("explain").unwrap_or("").trim();
                    let file = if (raw_args.starts_with('"') && raw_args.ends_with('"'))
                        || (raw_args.starts_with('\'') && raw_args.ends_with('\''))
                    {
                        if raw_args.len() >= 2 {
                            &raw_args[1..raw_args.len() - 1]
                        } else {
                            raw_args
                        }
                    } else {
                        raw_args
                    };
                    intent.file = if file.is_empty() { None } else { Some(file.to_string()) };
                }
                "docs" => {
                    intent.action = ShellAction::Docs;
                    let raw_args = trimmed.strip_prefix("docs").unwrap_or("").trim();
                    let file = if (raw_args.starts_with('"') && raw_args.ends_with('"'))
                        || (raw_args.starts_with('\'') && raw_args.ends_with('\''))
                    {
                        if raw_args.len() >= 2 {
                            &raw_args[1..raw_args.len() - 1]
                        } else {
                            raw_args
                        }
                    } else {
                        raw_args
                    };
                    intent.file = if file.is_empty() { None } else { Some(file.to_string()) };
                }
                _ => {}
            }

            (intent, String::new())
        } else {
            // Get project files map for brain classifier
            let dirs = config.knowledge.as_ref()
                .and_then(|k| k.dirs.clone())
                .unwrap_or_else(|| vec![".".to_string()]);
            let scanner = ProjectScanner::new(&root);
            let mut files = scanner.scan(dirs.clone()).unwrap_or_default();

            if let Some(extra_files) = config.knowledge.as_ref().and_then(|k| k.files.clone()) {
                for f in extra_files {
                    let normalized_f = f.replace('\\', "/");
                    let abs_path = root.join(&normalized_f);
                    if abs_path.exists() && abs_path.is_file() {
                        files.push(normalized_f);
                    }
                }
                files.sort();
                files.dedup();
            }

            let files_map = files.join("\n");
            let start_classify = std::time::Instant::now();
            let (intent, msg, tokens) = classify(trimmed, &history, &files_map, &config, external_files_list.as_deref()).await;

            elapsed_classify = start_classify.elapsed();
            classifier_tokens = tokens;
            total_session_tokens.fetch_add(tokens, std::sync::atomic::Ordering::Relaxed);
            (intent, msg)
        };

        history.push(BrainTurn { role: "user".to_string(), content: trimmed.to_string() });
        if history.len() > history_limit {
            history.drain(0..history.len() - history_limit);
        }

        if intent.action == ShellAction::Exit {
            let total = total_session_tokens.load(std::sync::atomic::Ordering::Relaxed);
            if total > 0 {
                println!("{}", truecolor(&format!("\n  [Total tokens used in this session: {}]", total), 107, 122, 150));
            }
            break;
        }
        let has_files_in_message = intent.action == ShellAction::RunCmd;
        let is_none_action = intent.action == ShellAction::None;

        if !message.is_empty() && (is_none_action || has_files_in_message) {
            let label = if intent.action == ShellAction::RunCmd {
                "  › Command summary: "
            } else {
                "  › "
            };
            println!();
            println!(
                "{}{}",
                truecolor(label, 168, 200, 255),
                truecolor(&message, 232, 237, 245)
            );
            println!("{}", truecolor(&format!("     [Tokens used: {} | Time: {}]", classifier_tokens, format_duration(elapsed_classify)), 107, 122, 150));
            println!();
            history.push(BrainTurn { role: "assistant".to_string(), content: message.clone() });
            if history.len() > history_limit {
                history.drain(0..history.len() - history_limit);
            }
        }

        let code = match intent.action {
            ShellAction::Index => {
                println!("{}", truecolor("\n  ↳ Indexing your project…\n", 107, 122, 150));
                spawn_subcommand(&["index"]).await.unwrap_or(1)
            }
            ShellAction::Scan => {
                println!("{}", truecolor("\n  ↳ Scanning for vulnerabilities and malicious patterns…\n", 107, 122, 150));
                spawn_subcommand(&["scan"]).await.unwrap_or(1)
            }
            ShellAction::Watch => {
                println!("{}", truecolor("\n  Starting watcher… Press Ctrl+C to stop and return to the shell.\n", 107, 122, 150));
                spawn_subcommand(&["watch"]).await.unwrap_or(1)
            }
            ShellAction::Log => {
                println!("{}", truecolor("\n  ↳ Loading activity log…\n", 107, 122, 150));
                spawn_subcommand(&["log"]).await.unwrap_or(1)
            }
            ShellAction::Stats => {
                println!("{}", truecolor("\n  ↳ Loading usage statistics…\n", 107, 122, 150));
                spawn_subcommand(&["stats"]).await.unwrap_or(1)
            }
            ShellAction::Undo => {
                println!("{}", truecolor("\n  ↳ Reverting last fix…\n", 107, 122, 150));
                spawn_subcommand(&["undo"]).await.unwrap_or(1)
            }
            ShellAction::Help => {
                print_shell_help();
                0
            }
            ShellAction::Exit => {
                break;
            }
            ShellAction::None => {
                0
            }
            ShellAction::Review => {
                let diff_res = review::get_git_diff(
                    intent.from.clone(),
                    intent.commit.clone(),
                    intent.staged,
                    intent.base.clone(),
                    intent.paths.clone(),
                );

                match diff_res {
                    Err(e) => {
                        let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                        println!("\n{}", style(format!("  Failed to load git diff: {}", sanitized)).red());
                        1
                    }
                    Ok((diff, diff_label, stats)) => {
                        if diff.trim().is_empty() {
                            println!(
                                "\n{}",
                                style("  [UE-03000003] ⚠ No changes found in diff — nothing to review.").yellow()
                            );
                            0
                        } else {
                            println!(
                                "\n{} {}  {} file{}, {} / {}",
                                style("✓").green().bold(),
                                style(format!("Diff loaded: {}", diff_label)).bold(),
                                stats.files_changed,
                                if stats.files_changed != 1 { "s" } else { "" },
                                style(format!("+{}", stats.lines_added)).green(),
                                style(format!("-{}", stats.lines_removed)).red()
                            );

                            let review_brain = config.get_brain_for_command("review");
                            println!();
                            println!(
                                "{}",
                                style(format!(
                                    "  ┌─ Code Review  [{}/{}]",
                                    review_brain.provider, review_brain.model
                                ))
                                .dim()
                            );
                            print!("  │\n  │  ");
                            std::io::stdout().flush().unwrap();

                            let prompt = review::build_review_prompt(&diff, "");
                            let system_instructions = laws::get_context("review");

                            match run_inference_streaming(
                                &prompt,
                                &system_instructions,
                                &config,
                                &review_brain,
                                classifier_tokens,
                                elapsed_classify,
                                total_session_tokens.clone(),
                                Some(&mut history),
                            )
                            .await
                            {
                                Ok(mut answer) => {
                                    let sanitized = safety.sanitize_response(&answer);
                                    if sanitized != answer {
                                        println!("  {}", crate::ui::rust("⚠ [CENSORSHIP]: Some potentially unsafe contents in the output were blocked for security."));
                                        answer = sanitized;
                                    }
                                    history.push(BrainTurn {
                                        role: "assistant".to_string(),
                                        content: answer.clone(),
                                    });
                                    if history.len() > history_limit {
                                        history.drain(0..history.len() - history_limit);
                                    }

                                    if intent.report {
                                        if let Err(e) = save_review_report(&root, &diff_label, &stats, &review_brain, &answer) {
                                            println!(
                                                "\n  {}",
                                                style(format!("Failed to save report: {}", e)).red()
                                            );
                                        }
                                    }

                                    0
                                }
                                Err(e) => {
                                    println!("\n{}", style(format!("  Failed to run review: {}", crate::errors::sanitize_error_msg(&e.to_string()))).red());
                                    1
                                }
                            }
                        }
                    }
                }
            }
            ShellAction::Explain => {
                let file = match intent.file {
                    Some(f) => f,
                    None => {
                        println!("\n{}", style("  Which file should I explain? e.g. \"explain src/app.ts\"").yellow());
                        "".to_string()
                    }
                };
                if !file.is_empty() {
                    let abs_path = root.join(&file);
                    if !abs_path.exists() || !abs_path.is_file() {
                        println!("\n{}", style(format!("  Failed to explain {}: File not found", file)).red());
                        1
                    } else {
                        let content = match fs::read_to_string(&abs_path) {
                            Ok(c) => c,
                            Err(e) => {
                                println!("\n{}", style(format!("  Failed to explain {}: {}", file, e)).red());
                                "".to_string()
                            }
                        };
                        if content.is_empty() {
                            1
                        } else {
                            println!("\n{}", truecolor(&format!("  ↳ Explaining {}…", file), 107, 122, 150));
                            println!();
                            let prompt = explain::build_explain_prompt(&file, &content, "");
                            let system_instructions = laws::get_context("explain");
                            let explain_brain = config.get_brain_for_command("explain");
                            match run_inference_streaming(&prompt, &system_instructions, &config, &explain_brain, classifier_tokens, elapsed_classify, total_session_tokens.clone(), Some(&mut history)).await {
                                Ok(mut answer) => {
                                    let sanitized = safety.sanitize_response(&answer);
                                    if sanitized != answer {
                                        println!("  {}", crate::ui::rust("⚠ [CENSORSHIP]: Some potentially unsafe contents in the output were blocked for security."));
                                        answer = sanitized;
                                    }
                                    history.push(BrainTurn { role: "assistant".to_string(), content: answer });
                                    if history.len() > history_limit {
                                        history.drain(0..history.len() - history_limit);
                                    }
                                    0
                                }
                                Err(e) => {
                                    println!("{}", style(format!("  Failed to explain: {}", crate::errors::sanitize_error_msg(&e.to_string()))).red());
                                    1
                                }
                            }
                        }
                    }
                } else {
                    1
                }
            }
            ShellAction::Docs => {
                let file = match intent.file {
                    Some(f) => f,
                    None => {
                        println!("\n{}", style("  Which file should I document? e.g. \"docs src/parser.ts\"").yellow());
                        "".to_string()
                    }
                };
                if !file.is_empty() {
                    let abs_path = root.join(&file);
                    if !abs_path.exists() || !abs_path.is_file() {
                        println!("\n{}", style(format!("  Failed to generate documentation for {}: File not found", file)).red());
                        1
                    } else {
                        let content = match fs::read_to_string(&abs_path) {
                            Ok(c) => c,
                            Err(e) => {
                                println!("\n{}", style(format!("  Failed to generate documentation for {}: {}", file, e)).red());
                                "".to_string()
                            }
                        };
                        if content.is_empty() {
                            1
                        } else {
                            println!("\n{}", truecolor(&format!("  ↳ Generating documentation for {}…", file), 107, 122, 150));
                            println!();
                            let prompt = docs::build_docs_prompt(&file, &content, "", "md");
                            let system_instructions = laws::get_context("docs");
                            let docs_brain = config.get_brain_for_command("docs");
                            match run_inference_streaming(&prompt, &system_instructions, &config, &docs_brain, classifier_tokens, elapsed_classify, total_session_tokens.clone(), Some(&mut history)).await {
                                Ok(mut answer) => {
                                    let sanitized = safety.sanitize_response(&answer);
                                    if sanitized != answer {
                                        println!("  {}", crate::ui::rust("⚠ [CENSORSHIP]: Some potentially unsafe contents in the output were blocked for security."));
                                        answer = sanitized;
                                    }
                                    history.push(BrainTurn { role: "assistant".to_string(), content: answer });
                                    if history.len() > history_limit {
                                        history.drain(0..history.len() - history_limit);
                                    }
                                    0
                                }
                                Err(e) => {
                                    println!("{}", style(format!("  Failed to document: {}", crate::errors::sanitize_error_msg(&e.to_string()))).red());
                                    1
                                }
                            }
                        }
                    }
                } else {
                    1
                }
            }
            ShellAction::Ask => {
                let question = match intent.question {
                    Some(q) => q,
                    None => "".to_string(),
                };
                if !question.is_empty() {
                    if let Err(err) = safety.validate_intent(&question) {
                        println!("\n{} {}", truecolor_bold("⚠️ [SECURITY POLICY]:", 255, 107, 53), err);
                        1
                    } else {
                        println!();
                        let context_frames = [
                            format!("{} {}", style("⠋").cyan(), style("Looking up context.  ").dim()),
                            format!("{} {}", style("⠙").cyan(), style("Looking up context.. ").dim()),
                            format!("{} {}", style("⠹").cyan(), style("Looking up context...")),
                            format!("{} {}", style("⠸").cyan(), style("Looking up context.  ").dim()),
                            format!("{} {}", style("⠼").cyan(), style("Looking up context.. ").dim()),
                            format!("{} {}", style("⠴").cyan(), style("Looking up context...")),
                            format!("{} {}", style("⠦").cyan(), style("Looking up context.  ").dim()),
                            format!("{} {}", style("⠧").cyan(), style("Looking up context.. ").dim()),
                            format!("{} {}", style("⠇").cyan(), style("Looking up context...")),
                            format!("{} {}", style("⠏").cyan(), style("Looking up context.  ").dim()),
                        ];
                        let context_frames_ref: Vec<&str> = context_frames.iter().map(|s| s.as_str()).collect();

                        let context_spinner = ProgressBar::new_spinner();
                        context_spinner.set_style(
                            ProgressStyle::default_spinner()
                                .tick_strings(&context_frames_ref)
                                .template("  ↳ {spinner}").unwrap(),
                        );
                        context_spinner.enable_steady_tick(std::time::Duration::from_millis(80));

                        let ask_brain = config.get_brain_for_command("ask");
                        let context = ContextAssembler::assemble(&question, &config, &ask_brain, &root, Some(intent.paths.clone()), &history).await
                            .unwrap_or_else(|_| "(No relevant context found)\n\n".to_string());

                        context_spinner.finish_and_clear();

                        // Cache 2 (loaded_files) and Cache 3 (file_content_cache) initialization
                        let mut loaded_files = parse_loaded_files_from_context(&context);
                        let mut file_content_cache = std::collections::HashMap::new();
                        for path in &loaded_files {
                            if let Some(formatted) = load_and_format_file(&root, path) {
                                file_content_cache.insert(path.clone(), formatted);
                            }
                        }

                        let mut attempts = 3;
                        let mut force_answer = false;
                        let base_system_instructions = laws::get_context("ask");
                        let chat_brain = config.get_brain_for_command("chat");
                        let mut exit_code = 0;

                        while attempts > 0 {
                            let mut context_block = String::new();
                            context_block.push_str("## RELEVANT CONTEXT (KNOWLEDGE MAP)\n\n");
                            if file_content_cache.is_empty() {
                                context_block.push_str("(No relevant context found or identified)\n\n");
                            } else {
                                for content in file_content_cache.values() {
                                    context_block.push_str(content);
                                }
                            }

                            let user_content = format!("{}\nQuestion: {}", context_block, question);

                            let system_instructions = if force_answer {
                                format!(
                                    "{}\n\n[SYSTEM WARNING: You already have access to the files: {:?}. \
                                    Do NOT request any more files using [NEED_FILE]. Answer the question now using ONLY the current context.]",
                                    base_system_instructions,
                                    loaded_files
                                )
                            } else {
                                base_system_instructions.clone()
                            };

                            match run_inference_streaming(
                                &user_content,
                                &system_instructions,
                                &config,
                                &chat_brain,
                                classifier_tokens,
                                elapsed_classify,
                                total_session_tokens.clone(),
                                Some(&mut history)
                            ).await {
                                Ok(mut answer) => {
                                    let sanitized = safety.sanitize_response(&answer);
                                    if sanitized != answer {
                                        println!("  {}", crate::ui::rust("⚠ [CENSORSHIP]: Some potentially unsafe contents in the output were blocked for security."));
                                        answer = sanitized;
                                    }

                                    let requested_files = parse_all_need_file_tags(&answer);
                                    let requested_searches = parse_all_web_search_tags(&answer);
                                    let requested_urls = parse_all_web_url_tags(&answer);

                                    if requested_files.is_empty() && requested_searches.is_empty() && requested_urls.is_empty() {
                                        history.push(BrainTurn { role: "assistant".to_string(), content: answer });
                                        if history.len() > history_limit {
                                            history.drain(0..history.len() - history_limit);
                                        }
                                        exit_code = 0;
                                        break;
                                    }

                                    let new_files: std::collections::HashSet<String> = requested_files.difference(&loaded_files).cloned().collect();
                                    let new_searches: std::collections::HashSet<String> = requested_searches.difference(&loaded_files).cloned().collect();
                                    let new_urls: std::collections::HashSet<String> = requested_urls.difference(&loaded_files).cloned().collect();

                                    if new_files.is_empty() && new_searches.is_empty() && new_urls.is_empty() {
                                        // Loop detected: model only requested files it already had
                                        clear_last_response_block(&answer);
                                        force_answer = true;
                                        attempts = 1;
                                        continue;
                                    }

                                    // Clear the printed [NEED_FILE: ...] response before loading message
                                    clear_last_response_block(&answer);

                                    println!();
                                    for path in &new_files {
                                        println!("  ↳ Loading additional file: {}...", path);
                                        if let Some(formatted) = load_and_format_file(&root, path) {
                                            file_content_cache.insert(path.clone(), formatted);
                                            loaded_files.insert(path.clone());
                                        }
                                    }
                                    for query in &new_searches {
                                        println!("  ↳ 🔍 Searching the web for: \"{}\"...", query);
                                        if let Some(formatted) = run_and_format_web_search(query, &root).await {
                                            file_content_cache.insert(format!("web_search_{}", query), formatted);
                                            loaded_files.insert(query.clone());
                                        }
                                    }
                                    for url in &new_urls {
                                        println!("  ↳ 🌐 Scraping website content: {}...", url);
                                        if let Some(formatted) = load_and_format_web_page(url).await {
                                            file_content_cache.insert(format!("web_url_{}", url), formatted);
                                            loaded_files.insert(url.clone());
                                        }
                                    }
                                    println!();

                                    attempts -= 1;
                                }
                                Err(e) => {
                                    println!("{}", style(format!("  Failed to answer: {}", crate::errors::sanitize_error_msg(&e.to_string()))).red());
                                    exit_code = 1;
                                    break;
                                }
                            }
                        }
                        exit_code
                    }
                } else {
                    1
                }
            }
            ShellAction::Askf => {
                let task = match intent.task {
                    Some(t) => t,
                    None => "".to_string(),
                };
                if !task.is_empty() {
                    let session_id = interactive_session_id.clone();

                    let mut files = askf::parse_files_with_id(&message, &session_id);
                    if files.is_empty() {
                        files = askf::parse_files(&message);
                    }

                    let has_files = !files.is_empty();
                    if has_files {
                        let _ = askf::write_parsed_files(&root, files);
                    }
                    if has_files {

                        let strip_pattern = format!(
                            r#"(?i)FILE_{}[\"\']?\s*[:=]?\s*[\"\']?[<\[\{{\(]?[^\s\]\[}}{{\"\',;<>\(\)]+[\"\']?\s*[>\]}})]?\s*\n\s*```[\s\S]*?```\s*END_(?:FILE_)?{}\n?"#,
                            session_id, session_id
                        );
                        let mut explanation = message.clone();
                        if let Ok(re) = regex::Regex::new(&strip_pattern) {
                            explanation = re.replace_all(&explanation, "").to_string();
                        }
                        if let Some(ref re) = static_strip_re {
                            explanation = re.replace_all(&explanation, "").to_string();
                        }

                        let explanation = explanation.trim().to_string();
                        if !explanation.is_empty() {
                            println!("{} {}", truecolor("  › ", 168, 200, 255), truecolor("Summary:", 107, 122, 150));
                            for line in explanation.lines() {
                                println!("    {}", truecolor(line, 107, 122, 150));
                            }
                            println!();
                        }

                        println!("{}", truecolor(&format!("  [Tokens used: {} | Time: {}]", classifier_tokens, format_duration(elapsed_classify)), 107, 122, 150));
                        println!();

                        history.push(BrainTurn { role: "assistant".to_string(), content: message.clone() });
                        if history.len() > history_limit {
                            history.drain(0..history.len() - history_limit);
                        }

                        0
                    } else {
                        let frames = [
                            format!("{} {}", style("⠋").cyan(), style("Thinking.  ").dim()),
                            format!("{} {}", style("⠙").cyan(), style("Thinking.. ").dim()),
                            format!("{} {}", style("⠹").cyan(), style("Thinking...").dim()),
                            format!("{} {}", style("⠸").cyan(), style("Thinking.  ").dim()),
                            format!("{} {}", style("⠼").cyan(), style("Thinking.. ").dim()),
                            format!("{} {}", style("⠴").cyan(), style("Thinking...").dim()),
                            format!("{} {}", style("⠦").cyan(), style("Thinking.  ").dim()),
                            format!("{} {}", style("⠧").cyan(), style("Thinking.. ").dim()),
                            format!("{} {}", style("⠇").cyan(), style("Thinking...").dim()),
                            format!("{} {}", style("⠏").cyan(), style("Thinking.  ").dim()),
                        ];
                        let frames_ref: Vec<&str> = frames.iter().map(|s| s.as_str()).collect();

                        let spinner = ProgressBar::new_spinner();
                        spinner.set_style(
                            ProgressStyle::default_spinner()
                                .tick_strings(&frames_ref)
                                .template("  ↳ {spinner}").unwrap(),
                        );
                        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

                        let prompt = askf::build_prompt(&task, &session_id);
                        let askf_brain = config.get_brain_for_command("askf");
                        let threads = askf_brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
                        let llm_init_res = init_llm(&askf_brain.provider, &askf_brain.model, threads).await;

                        if let Err(e) = llm_init_res {
                            spinner.finish_and_clear();
                            let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                            println!("{}", style(format!("  Failed to initialize LLM: {}", sanitized)).red());
                            1
                        } else {

                            // Clone the history excluding the last user turn (which is the current prompt),
                            // mirroring the same pattern used by run_inference_streaming for other intents.
                            let past_history = {
                                let mut turns = history.clone();
                                if let Some(last) = turns.last() {
                                    if last.role == "user" {
                                        turns.pop();
                                    }
                                }
                                turns
                            };

                            let api_history = if past_history.is_empty() {
                                None
                            } else {
                                Some(
                                    past_history
                                        .iter()
                                        .map(|turn| (turn.role.clone(), turn.content.clone()))
                                        .collect::<Vec<(String, String)>>(),
                                )
                            };

                            // For local models, history and system instructions aren't sent as separate
                            // API fields — they need to be folded into the prompt text via the model's
                            // chat template, mirroring what run_inference_streaming does for other intents.
                            let chat_template = detect_template(&askf_brain.model);

                            let (prompt, system_param, final_history) = if askf_brain.provider == "local" {
                                let system_instructions = laws::get_context("askf").replace("{session_id}", &session_id);
                                let chat_history: Vec<ChatTurn> = past_history
                                    .iter()
                                    .map(|turn| ChatTurn {
                                        role: turn.role.clone(),
                                        content: turn.content.clone(),
                                    })
                                    .collect();

                                let templated_prompt = apply_template(&chat_template, &system_instructions, &chat_history, &prompt);

                                // Both are already embedded in templated_prompt now, so clear them
                                // to avoid sending the same context twice.
                                (templated_prompt, None, None)
                            } else {
                                (
                                    prompt,
                                    Some(laws::get_context("askf").replace("{session_id}", &session_id)),
                                    api_history,
                                )
                            };


                            let params = InferParams {
                                model: askf_brain.model.clone(),
                                threads,
                                max_tokens: askf_brain.max_tokens,
                                temperature: askf_brain.temperature as f64,
                                repeat_penalty: 1.1,
                                max_context: 4096,
                                grammar: None,
                                provider: Some(askf_brain.provider.clone()),
                                system: system_param,
                                history: final_history,
                            };

                            let start_time = std::time::Instant::now();
                            let spinner_clone = spinner.clone();
                            let first_token = Arc::new(AtomicBool::new(true));
                            let first_token_clone = first_token.clone();

                            let infer_res = infer_with_params_streaming(&prompt, &params, move |_token| {
                                 if first_token_clone.swap(false, Ordering::Relaxed) {
                                     let generating_frames = [
                                         format!("{} {}", style("⠋").cyan(), style("Generating files.  ").dim()),
                                         format!("{} {}", style("⠙").cyan(), style("Generating files.. ").dim()),
                                         format!("{} {}", style("⠹").cyan(), style("Generating files...").dim()),
                                         format!("{} {}", style("⠸").cyan(), style("Generating files.  ").dim()),
                                         format!("{} {}", style("⠼").cyan(), style("Generating files.. ").dim()),
                                         format!("{} {}", style("⠴").cyan(), style("Generating files...").dim()),
                                         format!("{} {}", style("⠦").cyan(), style("Generating files.  ").dim()),
                                         format!("{} {}", style("⠧").cyan(), style("Generating files.. ").dim()),
                                         format!("{} {}", style("⠇").cyan(), style("Generating files...").dim()),
                                         format!("{} {}", style("⠏").cyan(), style("Generating files.  ").dim()),
                                     ];
                                     let generating_frames_ref: Vec<&str> = generating_frames.iter().map(|s| s.as_str()).collect();
                                     spinner_clone.set_style(
                                         ProgressStyle::default_spinner()
                                             .tick_strings(&generating_frames_ref)
                                             .template("  ↳ {spinner}").unwrap(),
                                     );
                                 }
                             }).await;
                            let elapsed = start_time.elapsed() + elapsed_classify;
                            spinner.finish_and_clear();
                            match infer_res {
                                Ok(res) => {
                                    total_session_tokens.fetch_add(res.tokens_used, Ordering::Relaxed);
                                    let answer = res.content;
                                    history.push(BrainTurn { role: "assistant".to_string(), content: answer.clone() });
                                    if history.len() > history_limit {
                                        history.drain(0..history.len() - history_limit);
                                    }

                                    let mut files = askf::parse_files_with_id(&answer, &session_id);
                                    if files.is_empty() {
                                        files = askf::parse_files(&answer);
                                    }

                                    let has_files = !files.is_empty();
                                    if has_files {
                                        let _ = askf::write_parsed_files(&root, files);
                                    }
                                    if has_files {

                                        let strip_pattern = format!(
                                            r#"(?i)FILE_{}[\"\']?\s*[:=]?\s*[\"\']?[<\[\{{\(]?[^\s\]\[}}{{\"\',;<>\(\)]+[\"\']?\s*[>\]}})]?\s*\n\s*```[\s\S]*?```\s*END_(?:FILE_)?{}\n?"#,
                                            session_id, session_id
                                        );
                                        let mut explanation = answer.clone();
                                        if let Ok(re) = regex::Regex::new(&strip_pattern) {
                                            explanation = re.replace_all(&explanation, "").to_string();
                                        }
                                        if let Some(ref re) = static_strip_re {
                                            explanation = re.replace_all(&explanation, "").to_string();
                                        }

                                        let explanation = explanation.trim().to_string();
                                        if !explanation.is_empty() {
                                            println!("{} {}", truecolor("  › ", 168, 200, 255), truecolor("Summary:", 107, 122, 150));
                                            for line in explanation.lines() {
                                                println!("    {}", truecolor(line, 107, 122, 150));
                                            }
                                            println!();
                                        }

                                        let total_tokens = res.tokens_used + classifier_tokens;
                                        let (cost, saved) = uneven_core::infrastructure::llm::pricing::calculate_cost_and_saved(
                                            &res.model,
                                            res.prompt_tokens,
                                            res.completion_tokens,
                                        );
                                        println!(
                                            "{}",
                                            truecolor(
                                                &format!(
                                                    "  [Tokens used: {} | Time: {} | Cost: ${:.5} | Saved: ${:.5}]",
                                                    total_tokens, format_duration(elapsed), cost, saved
                                                ),
                                                107, 122, 150
                                            )
                                        );
                                        println!();
                                    } else {
                                        println!("{}", truecolor("  › The LLM did not declare any files.", 107, 122, 150));
                                        println!("{}", truecolor("  › Try: \"create a file called index.html with a login form\"", 107, 122, 150));
                                        println!();
                                        let total_tokens = res.tokens_used + classifier_tokens;
                                        let (cost, saved) = uneven_core::infrastructure::llm::pricing::calculate_cost_and_saved(
                                            &res.model,
                                            res.prompt_tokens,
                                            res.completion_tokens,
                                        );
                                        println!(
                                            "{}",
                                            truecolor(
                                                &format!(
                                                    "  [Tokens used: {} | Time: {} | Cost: ${:.5} | Saved: ${:.5}]",
                                                    total_tokens, format_duration(elapsed), cost, saved
                                                ),
                                                107, 122, 150
                                            )
                                        );
                                        println!();
                                    }
                                    0
                                }
                                Err(e) => {
                                    let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                                    println!("{}", style(format!("  Failed to run task: {}", sanitized)).red());
                                    1
                                }
                            }
                        }
                    }
                } else {
                    1
                }
            }
            ShellAction::RunCmd => {
                let cmd_to_run = match intent.question {
                    Some(ref q) => q.clone(),
                    None => "".to_string(),
                };
                if cmd_to_run.is_empty() {
                    println!("\n{}", style("  No command suggested to run.").yellow());
                    0
                } else {
                    if let Err(err) = safety.validate_command(&cmd_to_run) {
                        println!("\n{} {}", truecolor_bold("⚠️ [SECURITY POLICY]:", 255, 107, 53), err);
                        let feedback = format!(
                            "[System: Command `{}` was BLOCKED by the security policy. Reason: {}]",
                            cmd_to_run, err
                        );
                        auto_prompt = Some(feedback);
                        0
                    } else {
                        let answer = if always_approve_commands {
                            "y".to_string()
                        } else {
                            let ans = loop {
                                print!(
                                    "\n{} Run command: {}? [y/n/a] › ",
                                    truecolor("  ? ", 255, 180, 0),
                                    truecolor(&cmd_to_run, 232, 237, 245)
                                );
                                std::io::stdout().flush().unwrap();

                                let input_confirm = match tokio::task::spawn_blocking(|| {
                                    let mut buf = String::new();
                                    std::io::stdin().read_line(&mut buf).map(|_| buf)
                                }).await {
                                    Ok(Ok(line)) => line.trim().to_lowercase(),
                                    _ => "a".to_string(),
                                };

                                if input_confirm == "y" || input_confirm == "yes" {
                                    break "y".to_string();
                                } else if input_confirm == "n" || input_confirm == "no" {
                                    break "n".to_string();
                                } else if input_confirm == "a" || input_confirm == "abort" || input_confirm == "all" || input_confirm == "always" {
                                    break "a".to_string();
                                }
                            };
                            if ans == "a" {
                                always_approve_commands = true;
                                "y".to_string()
                            } else {
                                ans
                            }
                        };

                        if answer == "y" {
                            println!("\n{}", truecolor(&format!("  ↳ Executing: {}", cmd_to_run), 107, 122, 150));
                            println!("{}", truecolor("  ┌─ Output", 107, 122, 150));

                            let (success, output) = execute_terminal_command_streaming(&cmd_to_run).await;

                            println!("{}", truecolor("  └─ End of Output", 107, 122, 150));
                            let status_str = if success { "success" } else { "failure" };
                            println!("  ↳ Command finished with {}.", status_str);
                            println!();

                            log_command_execution(&root, &config, &cmd_to_run, &message, status_str);

                            let feedback = format!(
                                "[System: Command `{}` finished with status: {}. Output:\n{}]",
                                cmd_to_run, status_str, output
                            );
                            auto_prompt = Some(feedback);
                            0
                        } else if answer == "n" {
                            println!("\n  ↳ Command execution skipped by user.\n");

                            log_command_execution(&root, &config, &cmd_to_run, &message, "skipped");

                            let feedback = format!(
                                "[System: User declined execution of command `{}`.]",
                                cmd_to_run
                            );
                            auto_prompt = Some(feedback);
                            0
                        } else {
                            println!("\n  ↳ Execution aborted.\n");
                            0
                        }
                    }
                }
            }
            ShellAction::MultiAgents => {
                let tasks = match intent.tasks {
                    Some(ref t) => t.clone(),
                    None => Vec::new(),
                };
                let cached_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("multi_agent_tasks_interactive.json");
                let has_cached = cached_path.exists();

                if !tasks.is_empty() || has_cached {
                    let config_brain_max_tokens = config.brain.max_tokens;
                    match crate::commands::multi_agents::run(&tasks, config_brain_max_tokens, &config, "interactive").await {
                        Ok(reports) => {
                            let mut report_msg = String::new();
                            report_msg.push_str("Multi-agent task execution completed.\n\n");
                            for (i, (task_desc, status)) in reports.iter().enumerate() {
                                report_msg.push_str(&format!("Agent {} [Task: {}]:\n{}\n\n", i + 1, task_desc, status));
                            }

                            auto_prompt = Some(format!(
                                "[System: All subagent tasks executed. Results are as follows. Please review them, make any adjustments if needed, and give a unified final explanation to the user:\n\n{}]",
                                report_msg
                            ));
                            0
                        }
                        Err(e) => {
                            let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                            println!("{}", style(format!("  Failed to run multi-agent tasks: {}", sanitized)).red());
                            1
                        }
                    }
                } else {
                    println!("{}", style("  No tasks specified for multi-agents.").yellow());
                    1
                }
            }
            ShellAction::Unknown => {
                println!(
                    "\n{} {}\n{}{}",
                    truecolor("  Not sure what you meant. ", 255, 235, 59),
                    truecolor("I can help with:", 107, 122, 150),
                    truecolor("  index · scan · watch · explain <file> · review · docs <file>\n", 107, 122, 150),
                    truecolor("  questions about your project · log · undo · help · exit\n", 107, 122, 150)
                );
                1
            }
        };

        if code == 0 {
            if let Some(hint) = get_hint(&intent.action) {
                println!();
                println!("{} {}", truecolor("  › ", 0, 255, 140), truecolor(hint, 107, 122, 150));
                println!();
            }
        }
    }

    restore_terminal_ui();
    let total = total_session_tokens.load(std::sync::atomic::Ordering::Relaxed);
    if total > 0 {
        println!("{}", truecolor(&format!("\n  [Total tokens used in this session: {}]", total), 107, 122, 150));
    }
    println!("{}", truecolor("\n  Goodbye!\n", 107, 122, 150));
    {
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    std::process::exit(0);
}

fn restore_terminal_ui() {
    let term = console::Term::stdout();
    let (rows, _) = term.size();
    if rows > 3 {
        print!("\x1b[r");
        print!("\x1b[{};1H\x1b[2K", rows);
        if rows > 5 {
            print!("\x1b[{};1H\x1b[2K", rows - 1);
            print!("\x1b[{};1H\x1b[2K", rows - 2);
        }
        print!("\x1b[{};1H", rows - 3);
        let _ = std::io::stdout().flush();
    }
}

fn save_review_report(
    root: &std::path::Path,
    diff_label: &str,
    stats: &review::DiffStats,
    brain: &uneven_core::domain::config::BrainConfig,
    review_text: &str,
) -> Result<()> {
    let reports_dir = root.join(".uneven").join("reports").join("reviews");
    std::fs::create_dir_all(&reports_dir)?;
    let date_str = chrono::Utc::now().to_rfc3339().replace(':', "-");
    let report_path = reports_dir.join(format!("review-{}.md", date_str));

    let header = format!(
        "# Code Review — {}\n**Date:** {}\n**Files:** {}  **+{}** / **-{}**\n**Model:** {}/{}\n\n---\n\n",
        diff_label,
        chrono::Utc::now().to_rfc3339(),
        stats.files_changed,
        stats.lines_added,
        stats.lines_removed,
        brain.provider,
        brain.model
    );

    std::fs::write(&report_path, header + review_text)?;
    println!(
        "\n  {}",
        style(format!("Report saved: {}", report_path.to_string_lossy())).cyan()
    );
    Ok(())
}

fn log_command_execution(root: &std::path::Path, config: &UnevenConfig, cmd: &str, summary: &str, status: &str) {
    let log_path = config.log.as_ref()
        .and_then(|l| l.path.as_ref())
        .cloned()
        .unwrap_or_else(|| ".uneven/logs/main.md".to_string());
    let log_abs_path = root.join(&log_path);
    if let Some(parent) = log_abs_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if !log_abs_path.exists() {
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(&log_abs_path) {
            let header = format!("# Uneven Log — {}\n\n", chrono::Utc::now().format("%Y-%m-%d"));
            let _ = file.write_all(header.as_bytes());
        }
    }

    if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(&log_abs_path) {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let entry = format!(
            "## [{}] 💻 Command Executed\n\n**Command:** `{}`\n\n**Summary:** {}\n\n**Status:** {}\n\n---\n\n",
            timestamp, cmd, summary, status
        );
        let _ = file.write_all(entry.as_bytes());
    }
}

async fn execute_terminal_command_streaming(cmd: &str) -> (bool, String) {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use std::sync::{Arc, Mutex};

    let child = if cfg!(target_os = "windows") {
        tokio::process::Command::new("cmd")
            .args(["/C", cmd])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    } else {
        tokio::process::Command::new("sh")
            .args(["-c", cmd])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    };

    let mut child = match child {
        Ok(c) => c,
        Err(e) => return (false, format!("Failed to execute command: {}", e)),
    };

    let stdout_handle = child.stdout.take().unwrap();
    let stderr_handle = child.stderr.take().unwrap();

    let output_data = Arc::new(Mutex::new(String::new()));
    let output_data_clone1 = output_data.clone();
    let output_data_clone2 = output_data.clone();

    let t1 = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout_handle).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            println!("  │  {}", line);
            let mut data = output_data_clone1.lock().unwrap();
            data.push_str(&line);
            data.push('\n');
        }
    });

    let t2 = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr_handle).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            eprintln!("  │  {}", line);
            let mut data = output_data_clone2.lock().unwrap();
            data.push_str(&line);
            data.push('\n');
        }
    });

    let status = child.wait().await;
    let _ = tokio::join!(t1, t2);

    let success = status.map(|s| s.success()).unwrap_or(false);
    let final_output = {
        let data = output_data.lock().unwrap();
        data.clone()
    };

    (success, final_output)
}

fn parse_loaded_files_from_context(context: &str) -> std::collections::HashSet<String> {
    let mut files = std::collections::HashSet::new();
    for line in context.lines() {
        if line.starts_with("**File: ") && line.ends_with("**") {
            let file = &line[8..line.len() - 2];
            files.insert(file.trim().to_string());
        }
    }
    files
}

pub(crate) fn load_and_format_file(root: &std::path::Path, rel_path: &str) -> Option<String> {
    let p = std::path::Path::new(rel_path);
    let abs_path = if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    };

    let norm_path = super::askf::normalize_path(&abs_path);
    let config = loader::load_config(root).ok();
    let ext_project = config.as_ref()
        .and_then(|c| c.knowledge.as_ref())
        .and_then(|k| k.external_project.as_ref())
        .map(std::path::PathBuf::from);

    let mut is_safe = norm_path.starts_with(super::askf::normalize_path(root));
    if !is_safe {
        if let Some(ref ext) = ext_project {
            if norm_path.starts_with(super::askf::normalize_path(ext)) {
                is_safe = true;
            }
        }
    }

    if !is_safe {
        return None;
    }

    if abs_path.exists() && abs_path.is_file() {
        if is_binary_file(&abs_path) {
            return None;
        }
        if let Ok(content) = std::fs::read_to_string(&abs_path) {
            let char_limit = 100_000;
            let (display_content, truncated) = if content.len() > char_limit {
                (format!("{}\n", &content[..char_limit]), true)
            } else {
                (content, false)
            };
            let numbered = uneven_core::infrastructure::utils::formatter::format_with_line_numbers(&display_content);
            let mut formatted = format!("**File: {}**\n```\n{}", rel_path, numbered);
            if truncated {
                formatted.push_str("... (rest of file truncated to save context window)\n");
            }
            formatted.push_str("```\n\n");
            return Some(formatted);
        }
    }
    None
}

pub(crate) fn is_binary_file(path: &std::path::Path) -> bool {
    use std::io::Read;
    if let Ok(mut file) = std::fs::File::open(path) {
        let mut buffer = [0; 1024];
        if let Ok(n) = file.read(&mut buffer) {
            for &b in buffer.iter().take(n) {
                if b == 0 {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn parse_all_need_file_tags(text: &str) -> std::collections::HashSet<String> {
    let mut files = std::collections::HashSet::new();
    if let Ok(re) = regex::Regex::new(r#"(?i)NEED_FILE[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?([^\s\]\[\}\{\"\',;<>\(\)]+)"#) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let mut path = m.as_str().trim().to_string();
                while path.ends_with('.') || path.ends_with('?') || path.ends_with('!') {
                    path.pop();
                }
                files.insert(path);
            }
        }
    }
    files
}

fn sync_external_project(ext_project_path: &str) -> Option<Vec<String>> {
    let ext_path = std::path::Path::new(ext_project_path);
    if !ext_path.exists() || !ext_path.is_dir() {
        return None;
    }
    let canonical = ext_path.canonicalize().unwrap_or_else(|_| ext_path.to_path_buf());

    // scan using ProjectScanner
    let scanner = ProjectScanner::new(&canonical);
    let mut files = scanner.scan(vec![".".to_string()]).unwrap_or_default();
    files.sort();
    files.dedup();

    let cache_dir = uneven_core::infrastructure::utils::paths::project_cache_dir(&canonical);
    let _ = std::fs::create_dir_all(&cache_dir);
    let cache_path = cache_dir.join("file-list-cache.json");
    if let Ok(json_content) = serde_json::to_string(&files) {
        let _ = std::fs::write(&cache_path, json_content);
    }

    Some(files)
}

fn match_model_string(s: &str) -> Result<(String, String), String> {
    let clean = s.to_lowercase().replace(' ', "-");

    let known_providers = [
        "gemini", "gpt", "openai", "claude", "sonnet", "haiku", "opus", "deepseek", "mistral", "groq", "ollama", "llama", "qwen"
    ];

    let has_known_keyword = known_providers.iter().any(|&k| clean.contains(k));
    let has_slash = clean.contains('/');

    if !has_known_keyword && !has_slash {
        let mut suggestion = "";
        if clean.contains("hemini") || clean.contains("gemii") || clean.contains("gemin") {
            suggestion = "gemini";
        } else if clean.contains("clyde") || clean.contains("claud") {
            suggestion = "claude";
        } else if clean.contains("gbt") || clean.contains("gtp") {
            suggestion = "gpt";
        } else if clean.contains("depseek") || clean.contains("deepsek") {
            suggestion = "deepseek";
        }

        if !suggestion.is_empty() {
            return Err(format!("Unknown model provider in \"{}\". Did you mean \"{}\"?", s, suggestion));
        } else {
            return Err(format!("Unknown model provider in \"{}\". Please use a known provider (gemini, gpt, claude, deepseek, llama, qwen) or use the provider/model format (e.g. custom/model).", s));
        }
    }

    if clean.contains("gemini") {
        if clean.contains("3.5") {
            Ok(("gemini-3.5-flash".to_string(), "gemini".to_string()))
        } else {
            Ok(("gemini-2.5-flash".to_string(), "gemini".to_string()))
        }
    } else if clean.contains("gpt") || clean.contains("openai") {
        if clean.contains("5.5") || clean.contains("5") {
            if clean.contains("pro") {
                Ok(("gpt-5.5-pro".to_string(), "openai".to_string()))
            } else {
                Ok(("gpt-5.5".to_string(), "openai".to_string()))
            }
        } else if clean.contains("4o") && clean.contains("mini") {
            Ok(("gpt-4o-mini".to_string(), "openai".to_string()))
        } else if clean.contains("3.5") {
            Ok(("gpt-3.5-turbo".to_string(), "openai".to_string()))
        } else {
            Ok(("gpt-4o".to_string(), "openai".to_string()))
        }
    } else if clean.contains("claude") || clean.contains("sonnet") || clean.contains("haiku") || clean.contains("opus") {
        if clean.contains("haiku") {
            Ok(("claude-haiku-4-5".to_string(), "claude".to_string()))
        } else if clean.contains("opus") {
            if clean.contains("4.8") || clean.contains("4") || clean.contains("4.6") {
                Ok(("claude-opus-4".to_string(), "claude".to_string()))
            } else {
                Ok(("claude-3-opus-20240229".to_string(), "claude".to_string()))
            }
        } else {
            Ok(("claude-3-5-sonnet-latest".to_string(), "claude".to_string()))
        }
    } else if clean.contains("deepseek") {
        if clean.contains("r1") {
            Ok(("deepseek-r1".to_string(), "deepseek".to_string()))
        } else {
            Ok(("deepseek-v4-flash".to_string(), "deepseek".to_string()))
        }
    } else if clean.contains("mistral") {
        Ok(("mistral-large-latest".to_string(), "mistral".to_string()))
    } else if clean.contains("groq") {
        Ok(("llama-3.3-70b-versatile".to_string(), "groq".to_string()))
    } else if clean.contains("ollama") {
        Ok(("llama3.2".to_string(), "ollama".to_string()))
    } else if clean.contains("llama") {
        Ok(("llama-3.2-1b-q4".to_string(), "local".to_string()))
    } else if clean.contains("qwen") {
        Ok(("qwen-2.5-1.5b-q8".to_string(), "local".to_string()))
    } else {
        if let Some((p, m)) = clean.split_once('/') {
            Ok((m.to_string(), p.to_string()))
        } else {
            Ok((clean, "local".to_string()))
        }
    }
}

fn parse_multi_brains(trimmed_args: &str) -> Result<Vec<(Option<String>, String, String)>, String> {
    let words: Vec<&str> = trimmed_args.split_whitespace().collect();
    if words.is_empty() {
        return Ok(Vec::new());
    }

    let command_keywords = vec![
        "explain", "explan", "pentest", "scan", "watch", "autofix", "ask", "askf", "docs", "review", "chat", "analyze", "zeroday"
    ];
    let provider_keywords = vec![
        "gemini", "gpt", "claude", "opus", "sonnet", "haiku", "deepseek", "mistral", "groq", "ollama", "llama", "qwen"
    ];

    let has_command_keywords = words.iter().any(|w| command_keywords.contains(&w.to_lowercase().as_str()));

    let mut results = Vec::new();

    if has_command_keywords {
        let mut current_command = None;
        let mut current_model_tokens = Vec::new();

        for word in words {
            let lower_word = word.to_lowercase();
            if command_keywords.contains(&lower_word.as_str()) {
                let cmd_name = if lower_word == "explan" {
                    "explain".to_string()
                } else {
                    lower_word
                };
                if let Some(cmd) = current_command {
                    if !current_model_tokens.is_empty() {
                        let model_str = current_model_tokens.join(" ");
                        let (m, p) = match_model_string(&model_str)?;
                        results.push((Some(cmd), m, p));
                    }
                }
                current_command = Some(cmd_name);
                current_model_tokens = Vec::new();
            } else {
                current_model_tokens.push(word.to_string());
            }
        }
        if let Some(cmd) = current_command {
            if !current_model_tokens.is_empty() {
                let model_str = current_model_tokens.join(" ");
                let (m, p) = match_model_string(&model_str)?;
                results.push((Some(cmd), m, p));
            }
        }
    } else {
        let mut current_model_tokens = Vec::new();

        for word in words {
            let lower_word = word.to_lowercase();
            let starts_new_model = provider_keywords.iter().any(|pk| lower_word.contains(pk));

            if starts_new_model && !current_model_tokens.is_empty() {
                let model_str = current_model_tokens.join(" ");
                let (m, p) = match_model_string(&model_str)?;
                results.push((None, m, p));
                current_model_tokens = Vec::new();
            }
            current_model_tokens.push(word.to_string());
        }

        if !current_model_tokens.is_empty() {
            let model_str = current_model_tokens.join(" ");
            let (m, p) = match_model_string(&model_str)?;
            results.push((None, m, p));
        }
    }

    Ok(results)
}

fn allocate_brains(
    parsed: Vec<(Option<String>, String, String)>,
    config: &mut UnevenConfig,
) {
    if parsed.is_empty() {
        return;
    }

    let mut brains_list = Vec::new();
    let mut routing_map = std::collections::HashMap::new();

    for (opt_cmd, m, p) in parsed.clone() {
        let brain_id = format!("{}-{}", p, m);
        let mut local_cfg = None;
        if p == "local" {
            let hw = uneven_core::infrastructure::utils::hardware_detector::HardwareDetector::new()
                .detect_system()
                .ok();
            let cpu_cores = hw.as_ref().map(|h| h.cpu_cores).unwrap_or(4);
            let threads = std::cmp::max(2, if cpu_cores > 2 { cpu_cores - 1 } else { cpu_cores }) as u32;
            let gpu_layers = hw.as_ref().map(|h| h.gpu.gpu_layers).unwrap_or(0);

            local_cfg = Some(uneven_core::domain::LocalBrainConfig {
                threads: Some(threads),
                gpu_layers: Some(gpu_layers),
                unload_timeout: Some(10),
            });
        }

        let brain_cfg = uneven_core::domain::BrainConfig {
            id: Some(brain_id.clone()),
            provider: p.clone(),
            model: m.clone(),
            api_key: None,
            temperature: 0.3,
            max_tokens: 2048,
            local: local_cfg,
            reasoning_effort: None,
        };

        if !brains_list.iter().any(|b: &uneven_core::domain::BrainConfig| b.id == brain_cfg.id) {
            brains_list.push(brain_cfg);
        }

        if let Some(cmd) = opt_cmd {
            routing_map.insert(cmd, brain_id);
        }
    }

    let no_explicit_commands = parsed.iter().all(|(opt_cmd, _, _)| opt_cmd.is_none());
    if no_explicit_commands && !brains_list.is_empty() {
        if brains_list.len() == 1 {
            config.brain = brains_list[0].clone();
        } else {
            let score = |b: &uneven_core::domain::BrainConfig| -> i32 {
                let name = b.model.to_lowercase();
                if name.contains("opus") || name.contains("r1") || name.contains("gpt-5") {
                    100
                } else if name.contains("sonnet") || name.contains("gpt-4o") || name.contains("gpt-4-") {
                    if name.contains("mini") { 40 } else { 80 }
                } else if name.contains("flash") || name.contains("haiku") || name.contains("mini") {
                    30
                } else {
                    10
                }
            };

            let mut sorted_brains = brains_list.clone();
            sorted_brains.sort_by_key(|b| -score(b));

            config.brain = sorted_brains[0].clone();

            if sorted_brains.len() == 2 {
                let best_id = sorted_brains[0].id.clone().unwrap_or_default();
                let fast_id = sorted_brains[1].id.clone().unwrap_or_default();

                for cmd in &["askf", "watch", "autofix", "pentest", "review", "scan", "zeroday"] {
                    routing_map.insert(cmd.to_string(), best_id.clone());
                }
                for cmd in &["ask", "explain", "docs", "chat", "analyze"] {
                    routing_map.insert(cmd.to_string(), fast_id.clone());
                }
            } else if sorted_brains.len() >= 3 {
                let best_id = sorted_brains[0].id.clone().unwrap_or_default();
                let mid_id = sorted_brains[1].id.clone().unwrap_or_default();
                let fast_id = sorted_brains.last().unwrap().id.clone().unwrap_or_default();

                for cmd in &["askf", "watch", "autofix"] {
                    routing_map.insert(cmd.to_string(), best_id.clone());
                }
                for cmd in &["pentest", "review", "scan", "zeroday"] {
                    routing_map.insert(cmd.to_string(), mid_id.clone());
                }
                for cmd in &["ask", "explain", "docs", "chat", "analyze"] {
                    routing_map.insert(cmd.to_string(), fast_id.clone());
                }
            }
        }
    }

    if !brains_list.is_empty() {
        config.brains = Some(brains_list);
        config.routing = Some(routing_map);
    }
}

fn clear_last_response_block(content: &str) {
    let mut lines_count = 4; // surrounding lines: header, empty-line, done, tokens info
    lines_count += content.chars().filter(|&c| c == '\n').count();
    for _ in 0..=lines_count {
        print!("\x1b[1A\x1b[2K");
    }
    let _ = std::io::stdout().flush();
}

async fn ensure_local_model_files(model: &str) -> Result<(), String> {
    let global_models_dir = uneven_core::infrastructure::utils::paths::global_models_dir();
    let gguf_path = global_models_dir.join(format!("{}.gguf", model));
    let tokenizer_path = global_models_dir.join(format!("{}-tokenizer.json", model));

    if gguf_path.exists() && tokenizer_path.exists() {
        return Ok(());
    }

    // Find the catalog entry for this model
    let catalog_entry = uneven_core::domain::consts::models::SUPPORTED_LOCAL_MODELS.iter()
        .find(|m| m.id == model);

    match catalog_entry {
        Some(entry) => {
            println!("\n  {} Local model files are missing for '{}'. Starting automatic download...", ui::accent("›"), model);
            for file_entry in entry.files {
                let suffix = file_entry.dest.strip_prefix(".uneven/models/").unwrap_or(file_entry.dest);
                let dest = global_models_dir.join(suffix);
                let label = dest.file_name().unwrap_or_default().to_string_lossy();
                if dest.exists() {
                    continue;
                }
                if let Err(e) = super::init::download_file(file_entry.url, &dest, &label).await {
                    return Err(format!("Failed to download {}: {}", label, e));
                }
            }
            println!();
            Ok(())
        }
        None => {
            Err(format!(
                "Model '{}' is not in the supported local catalog for auto-download.\n  \
                 Please place the GGUF file manually in {} as '{}.gguf' and tokenizer as '{}-tokenizer.json'.",
                model, global_models_dir.display(), model, model
            ))
        }
    }
}

pub(crate) fn parse_all_web_search_tags(text: &str) -> std::collections::HashSet<String> {
    let mut queries = std::collections::HashSet::new();
    if let Ok(re) = regex::Regex::new(r#"(?i)\[WEB_SEARCH:\s*([^\]]+)\]"#) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let q = m.as_str().trim().to_string();
                if !q.is_empty() {
                    queries.insert(q);
                }
            }
        }
    }
    if let Ok(re) = regex::Regex::new(r#"(?i)WEB_SEARCH:\s*["']([^"']+)["']"#) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let q = m.as_str().trim().to_string();
                if !q.is_empty() {
                    queries.insert(q);
                }
            }
        }
    }
    queries
}

pub(crate) fn parse_all_web_url_tags(text: &str) -> std::collections::HashSet<String> {
    let mut urls = std::collections::HashSet::new();
    if let Ok(re) = regex::Regex::new(r#"(?i)\[WEB_URL:\s*([^\]]+)\]"#) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let u = m.as_str().trim().to_string();
                if !u.is_empty() {
                    urls.insert(u);
                }
            }
        }
    }
    if let Ok(re) = regex::Regex::new(r#"(?i)WEB_URL:\s*["']?([^\s\]}'"']+)["']?"#) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let u = m.as_str().trim().to_string();
                if !u.is_empty() {
                    urls.insert(u);
                }
            }
        }
    }
    urls
}

pub(crate) async fn load_and_format_web_page(url: &str) -> Option<String> {
    let scraper = uneven_core::infrastructure::knowledge::web_loader::WebScraper::new();
    if let Ok(doc) = scraper.fetch_url(url).await {
        let mut formatted = String::new();
        formatted.push_str(&format!("\n### WEBSITE CONTENT FROM URL: {}\n", url));
        formatted.push_str(&format!("Title: {}\n\n", doc.title));
        formatted.push_str(&doc.content);
        formatted.push_str("\n\n");
        Some(formatted)
    } else {
        None
    }
}

pub(crate) async fn run_and_format_web_search(query: &str, project_root: &std::path::Path) -> Option<String> {
    let scraper = uneven_core::infrastructure::knowledge::web_loader::WebScraper::new();
    let mut search_res = Vec::new();
    if let Ok(results) = scraper.search_web(query, project_root).await {
        search_res.extend(results);
    }
    if let Ok(results) = scraper.search_wikipedia(query).await {
        search_res.extend(results);
    }
    if search_res.is_empty() {
        return None;
    }
    let mut formatted = String::new();
    formatted.push_str(&format!("\n### SEARCH RESULTS FOR QUERY: \"{}\"\n\n", query));
    for (i, res) in search_res.iter().enumerate() {
        formatted.push_str(&format!("{}. {}\n", i + 1, res.title));
        formatted.push_str(&format!("   URL: {}\n", res.url));
        formatted.push_str(&format!("   Snippet: {}\n\n", res.snippet));
    }

    // Scan local sitemaps cache directory
    let sitemaps_dir = uneven_core::infrastructure::utils::paths::project_cache_dir(project_root).join("web-sitemaps");
    if sitemaps_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&sitemaps_dir) {
            let mut sitemap_hints = Vec::new();
            for entry in entries.filter_map(Result::ok) {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if let Ok(cached) = serde_json::from_str::<uneven_core::infrastructure::knowledge::web_loader::CachedSitemap>(&content) {
                        let query_words: Vec<&str> = query.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric())).filter(|w| !w.is_empty()).collect();
                        let mut matches = false;
                        for word in &query_words {
                            if cached.domain.to_lowercase().contains(&word.to_lowercase()) {
                                matches = true;
                                break;
                            }
                            for u in cached.urls.iter().take(50) {
                                if u.to_lowercase().contains(&word.to_lowercase()) {
                                    matches = true;
                                    break;
                                }
                            }
                        }
                        if matches {
                            sitemap_hints.push(cached);
                        }
                    }
                }
            }
            if !sitemap_hints.is_empty() {
                formatted.push_str("### KNOWN CACHED PAGES (SITEMAPS) FOR DOMAINS MATCHING QUERY:\n");
                for hint in sitemap_hints {
                    formatted.push_str(&format!("* Domain: {} (Sitemap URLs cached: {})\n", hint.domain, hint.urls.len()));
                    let query_words: Vec<&str> = query.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric())).filter(|w| !w.is_empty()).collect();
                    let mut shown = 0;
                    for url in &hint.urls {
                        let mut url_match = false;
                        for word in &query_words {
                            if url.to_lowercase().contains(&word.to_lowercase()) {
                                url_match = true;
                                break;
                            }
                        }
                        if url_match {
                            formatted.push_str(&format!("  - {}\n", url));
                            shown += 1;
                            if shown >= 5 {
                                break;
                            }
                        }
                    }
                }
                formatted.push('\n');
            }
        }
    }

    Some(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_classify_intent_utility() {
        assert_eq!(classify_intent("exit").action, ShellAction::Exit);
        assert_eq!(classify_intent("help").action, ShellAction::Help);
        assert_eq!(classify_intent("log").action, ShellAction::Log);
        assert_eq!(classify_intent("undo").action, ShellAction::Undo);
        assert_eq!(classify_intent("index").action, ShellAction::Index);
        assert_eq!(classify_intent("scan").action, ShellAction::Scan);
        assert_eq!(classify_intent("watch").action, ShellAction::Watch);
    }

    #[test]
    fn test_classify_intent_explain_and_docs() {
        let explain_intent = classify_intent("explain src/app.ts");
        assert_eq!(explain_intent.action, ShellAction::Explain);
        assert_eq!(explain_intent.file, Some("src/app.ts".to_string()));

        let explain_bare = classify_intent("src/app.ts");
        assert_eq!(explain_bare.action, ShellAction::Explain);
        assert_eq!(explain_bare.file, Some("src/app.ts".to_string()));

        let docs_intent = classify_intent("docs src/utils.js");
        assert_eq!(docs_intent.action, ShellAction::Docs);
        assert_eq!(docs_intent.file, Some("src/utils.js".to_string()));
    }

    #[test]
    fn test_classify_intent_review() {
        let review1 = classify_intent("review --commit abc1234");
        assert_eq!(review1.action, ShellAction::Review);
        assert_eq!(review1.commit, Some("abc1234".to_string()));

        let review2 = classify_intent("review --from main --staged");
        assert_eq!(review2.action, ShellAction::Review);
        assert_eq!(review2.from, Some("main".to_string()));
        assert!(review2.staged);

        let review3 = classify_intent("review --base main src/main.rs src/utils.rs");
        assert_eq!(review3.action, ShellAction::Review);
        assert_eq!(review3.base, Some("main".to_string()));
        assert_eq!(review3.paths, vec!["src/main.rs".to_string(), "src/utils.rs".to_string()]);
    }

    #[test]
    fn test_classify_intent_fallback() {
        // Natural language query without prefix -> Ask
        let ask_intent = classify_intent("como configurar o banco?");
        assert_eq!(ask_intent.action, ShellAction::Ask);
        assert_eq!(ask_intent.question, Some("como configurar o banco?".to_string()));

        let ask_ola = classify_intent("ola");
        assert_eq!(ask_ola.action, ShellAction::Ask);

        let ask_natural = classify_intent("crie uma rota de autenticação com jwt");
        assert_eq!(ask_natural.action, ShellAction::Ask);

        // With askf prefix -> Askf
        let askf_intent = classify_intent("askf crie uma rota de autenticação com jwt");
        assert_eq!(askf_intent.action, ShellAction::Askf);
        assert_eq!(askf_intent.task, Some("crie uma rota de autenticação com jwt".to_string()));
    }

    #[test]
    fn test_tokenize_shell_input() {
        let tokens = tokenize_shell_input("scan -d \"./src dir\" --report");
        assert_eq!(tokens, vec!["scan", "-d", "./src dir", "--report"]);

        let tokens_single = tokenize_shell_input("explain 'src/utils/math.rs'");
        assert_eq!(tokens_single, vec!["explain", "src/utils/math.rs"]);

        let escaped = tokenize_shell_input("hello\\ world");
        assert_eq!(escaped, vec!["hello world"]);
    }

    #[test]
    fn test_parse_analyze_input() {
        // Pure prompt without flags
        let parsed = parse_analyze_input("como está o faturamento?");
        assert_eq!(
            parsed,
            vec![
                "analyze".to_string(),
                "--prompt".to_string(),
                "como está o faturamento?".to_string()
            ]
        );

        // Mixed flags and prompt
        let parsed_mixed = parse_analyze_input("qual tabela tem mais registros --theme light --report html");
        assert_eq!(
            parsed_mixed,
            vec![
                "analyze".to_string(),
                "--theme".to_string(),
                "light".to_string(),
                "--report".to_string(),
                "html".to_string(),
                "--prompt".to_string(),
                "qual tabela tem mais registros".to_string()
            ]
        );

        // Flags only
        let parsed_only_flags = parse_analyze_input("--clear-cache");
        assert_eq!(parsed_only_flags, vec!["analyze".to_string(), "--clear-cache".to_string()]);
    }

    #[test]
    fn test_parse_all_need_file_tags() {
        let text = r#"
            Hello, please check [NEED_FILE: src/main.rs]
            Also need_file:src/utils.rs and unclosed [NEED_FILE: src/app.ts
            Maybe json {"need_file": "src/theme.rs"} or single quote NEED_FILE:'src/config.rs'
            And trailing comma need_file:src/db.rs, thanks!
            Sentence ending with dot need_file:src/router.rs.
            Or questioning need_file:src/auth.rs?
            With equals sign NEED_FILE=src/services.ts and angle brackets NEED_FILE: <src/handler.go>
            With parenthesis NEED_FILE (src/models.py) and spaces NEED_FILE src/views.py
        "#;
        let tags = parse_all_need_file_tags(text);
        assert!(tags.contains("src/main.rs"));
        assert!(tags.contains("src/utils.rs"));
        assert!(tags.contains("src/app.ts"));
        assert!(tags.contains("src/theme.rs"));
        assert!(tags.contains("src/config.rs"));
        assert!(tags.contains("src/db.rs"));
        assert!(tags.contains("src/router.rs"));
        assert!(tags.contains("src/auth.rs"));
        assert!(tags.contains("src/services.ts"));
        assert!(tags.contains("src/handler.go"));
        assert!(tags.contains("src/models.py"));
        assert!(tags.contains("src/views.py"));
        assert_eq!(tags.len(), 12);
    }

    #[test]
    fn test_parse_all_web_search_and_url_tags() {
        let text = r#"
            Please search Google for [WEB_SEARCH: actix-web 4.5 routing examples]
            Also try WEB_SEARCH: "rust tokio tutorial" and WEB_SEARCH: 'uneven agent'
            And check the documentation at [WEB_URL: https://actix.rs/docs/routing/]
            Also WEB_URL: https://github.com/uneven-agent/uneven
        "#;
        let searches = parse_all_web_search_tags(text);
        assert!(searches.contains("actix-web 4.5 routing examples"));
        assert!(searches.contains("rust tokio tutorial"));
        assert!(searches.contains("uneven agent"));
        assert_eq!(searches.len(), 3);

        let urls = parse_all_web_url_tags(text);
        assert!(urls.contains("https://actix.rs/docs/routing/"));
        assert!(urls.contains("https://github.com/uneven-agent/uneven"));
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn test_parse_multi_brains_no_explicit() {
        let parsed = parse_multi_brains("gemini 3.5 flash opus 4.6 gpt 5.5").unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], (None, "gemini-3.5-flash".to_string(), "gemini".to_string()));
        assert_eq!(parsed[1], (None, "claude-opus-4".to_string(), "claude".to_string()));
        assert_eq!(parsed[2], (None, "gpt-5.5".to_string(), "openai".to_string()));
    }

    #[test]
    fn test_parse_multi_brains_explicit_with_typo() {
        let parsed = parse_multi_brains("explan gemini 3.5 flash pentest sonnet 3").unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], (Some("explain".to_string()), "gemini-3.5-flash".to_string(), "gemini".to_string()));
        assert_eq!(parsed[1], (Some("pentest".to_string()), "claude-3-5-sonnet-latest".to_string(), "claude".to_string()));
    }

    #[test]
    fn test_parse_multi_brains_with_typo() {
        let parsed = parse_multi_brains("hemini 3.5 flash");
        assert!(parsed.is_err());
        let err = parsed.unwrap_err();
        assert!(err.contains("Unknown model provider in \"hemini 3.5 flash\". Did you mean \"gemini\"?"));
    }

    #[test]
    fn test_allocate_brains_smart_routing() {
        let mut config = UnevenConfig::default();
        let parsed = parse_multi_brains("gemini 3.5 flash opus 4.6 gpt 5.5").unwrap();
        allocate_brains(parsed, &mut config);

        let brains = config.brains.as_ref().unwrap();
        assert_eq!(brains.len(), 3);

        assert_eq!(config.get_brain_for_command("watch").id.unwrap(), "claude-claude-opus-4");
        assert_eq!(config.get_brain_for_command("review").id.unwrap(), "openai-gpt-5.5");
        assert_eq!(config.get_brain_for_command("explain").id.unwrap(), "gemini-gemini-3.5-flash");
        assert_eq!(config.get_brain_for_command("chat").id.unwrap(), "gemini-gemini-3.5-flash");
        assert_eq!(config.get_brain_for_command("analyze").id.unwrap(), "gemini-gemini-3.5-flash");
        assert_eq!(config.get_brain_for_command("scan").id.unwrap(), "openai-gpt-5.5");
        assert_eq!(config.get_brain_for_command("zeroday").id.unwrap(), "openai-gpt-5.5");

        // Assert dynamic temperature mappings
        assert_eq!(config.get_brain_for_command("chat").temperature, 0.7);
        assert_eq!(config.get_brain_for_command("explain").temperature, 0.2);
        assert_eq!(config.get_brain_for_command("watch").temperature, 0.2);
    }

    #[test]
    fn test_allocate_brains_local_models() {
        let mut config = UnevenConfig::default();
        let parsed = parse_multi_brains("llama qwen").unwrap();
        allocate_brains(parsed, &mut config);

        let brains = config.brains.as_ref().unwrap();
        assert_eq!(brains.len(), 2);
        assert_eq!(brains[0].provider, "local");
        assert_eq!(brains[0].model, "llama-3.2-1b-q4");
        assert!(brains[0].local.is_some());

        assert_eq!(brains[1].provider, "local");
        assert_eq!(brains[1].model, "qwen-2.5-1.5b-q8");
        assert!(brains[1].local.is_some());
    }

    #[test]
    fn test_slash_analyze_parsing_logic() {
        let mut config = UnevenConfig::default();

        // 1. db url
        let db_url = "postgres://localhost/test_db";
        let mut db_cfg = config.knowledge.as_ref().and_then(|k| k.db.clone()).unwrap_or_default();
        db_cfg.url = Some(db_url.to_string());
        let mut k_cfg = config.knowledge.clone().unwrap_or_default();
        k_cfg.db = Some(db_cfg);
        config.knowledge = Some(k_cfg);

        assert_eq!(config.knowledge.as_ref().unwrap().db.as_ref().unwrap().url.as_ref().unwrap(), "postgres://localhost/test_db");

        // 2. allowed tables
        let tables = vec!["users".to_string(), "orders".to_string()];
        let mut db_cfg = config.knowledge.as_ref().and_then(|k| k.db.clone()).unwrap_or_default();
        db_cfg.allowed_tables = Some(tables.clone());
        let mut k_cfg = config.knowledge.clone().unwrap_or_default();
        k_cfg.db = Some(db_cfg);
        config.knowledge = Some(k_cfg);

        assert_eq!(config.knowledge.as_ref().unwrap().db.as_ref().unwrap().allowed_tables.as_ref().unwrap(), &vec!["users".to_string(), "orders".to_string()]);

        // 3. mask columns and tables
        let m_tables = vec!["salary".to_string()];
        let m_columns = vec!["cpf".to_string(), "password".to_string()];
        config.security.blocked_tables = m_tables;
        config.security.masked_columns = m_columns;

        assert_eq!(config.security.blocked_tables, vec!["salary".to_string()]);
        assert_eq!(config.security.masked_columns, vec!["cpf".to_string(), "password".to_string()]);
    }

    #[test]
    fn test_slash_pentest_parsing_logic() {
        let mut config = UnevenConfig::default();

        let mut p_cfg = config.pentester.clone().unwrap_or_default();
        p_cfg.target = Some("http://localhost:3000".to_string());
        config.pentester = Some(p_cfg);

        assert_eq!(config.pentester.as_ref().unwrap().target.as_ref().unwrap(), "http://localhost:3000");

        let mut p_cfg = config.pentester.clone().unwrap_or_default();
        p_cfg.mode = Some(uneven_core::domain::PentesterMode::Active);

        let mut s_cfg = p_cfg.static_config.clone().unwrap_or_default();
        s_cfg.owasp = Some(true);
        s_cfg.secrets = Some(false);
        p_cfg.static_config = Some(s_cfg);

        let mut b_cfg = p_cfg.bruteforce.clone().unwrap_or_default();
        b_cfg.enabled = Some(true);
        p_cfg.bruteforce = Some(b_cfg);

        let mut f_cfg = p_cfg.firewall.clone().unwrap_or_default();
        f_cfg.enabled = Some(false);
        p_cfg.firewall = Some(f_cfg);

        config.pentester = Some(p_cfg);

        let saved_p = config.pentester.as_ref().unwrap();
        assert_eq!(saved_p.mode, Some(uneven_core::domain::PentesterMode::Active));
        assert_eq!(saved_p.static_config.as_ref().unwrap().owasp, Some(true));
        assert_eq!(saved_p.static_config.as_ref().unwrap().secrets, Some(false));
        assert_eq!(saved_p.bruteforce.as_ref().unwrap().enabled, Some(true));
        assert_eq!(saved_p.firewall.as_ref().unwrap().enabled, Some(false));
    }
}
