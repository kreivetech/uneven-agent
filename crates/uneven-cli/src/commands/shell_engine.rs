use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uneven_core::UnevenConfig;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::security::laws;
use uneven_core::infrastructure::llm::{InferParams, init as init_llm, infer_with_params};
use uneven_core::infrastructure::llm::history_manager::optimize_context;

pub fn handle_slash_command(trimmed: &str, config: &UnevenConfig, root: &std::path::Path) -> Result<String> {
    let mut updated_config = config.clone();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return Err(anyhow::anyhow!("Empty slash command"));
    }
    let cmd = parts[0].to_lowercase();
    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();

    let reply = match cmd.as_str() {
        "/model" | "/models" => {
            if payload.is_empty() {
                format!("Active Brain: {} / {}", config.brain.provider, config.brain.model)
            } else {
                let (provider, model) = if payload.contains('/') {
                    let p: Vec<&str> = payload.splitn(2, '/').collect();
                    (p[0].to_string(), p[1].to_string())
                } else {
                    let p: Vec<&str> = payload.split_whitespace().collect();
                    if p.len() >= 2 {
                        (p[0].to_string(), p[1].to_string())
                    } else {
                        let m = p[0].to_lowercase();
                        let inferred_provider = if m.contains("gpt") || m.contains("o1") || m.contains("o3") {
                            "openai"
                        } else if m.contains("claude") {
                            "claude"
                        } else if m.contains("gemini") {
                            "gemini"
                        } else if m.contains("deepseek") {
                            "deepseek"
                        } else if m.contains("llama") || m.contains("qwen") || m.contains("phi") {
                            "local"
                        } else {
                            "openai"
                        };
                        (inferred_provider.to_string(), p[0].to_string())
                    }
                };

                updated_config.brain.provider = provider.clone();
                updated_config.brain.model = model.clone();
                updated_config.brains = None;
                updated_config.routing = None;
                format!("Active Brain updated to: {} / {}", provider, model)
            }
        }
        "/person" | "/persona" => {
            if payload.is_empty() {
                format!("Custom Agent Persona: {}", config.persona.as_deref().unwrap_or("Default (Senior Engineer)"))
            } else if payload.to_lowercase() == "default" || payload.to_lowercase() == "reset" {
                updated_config.persona = None;
                "Custom agent persona reset to default (Senior Engineer).".to_string()
            } else {
                updated_config.persona = Some(payload.to_string());
                format!("Custom agent persona updated: {}", payload)
            }
        }
        "/effort" | "/reasoning" => {
            if payload.is_empty() {
                format!("Reasoning Effort: {}", config.brain.reasoning_effort.as_deref().unwrap_or("Default (None / Provider Managed)"))
            } else if payload.to_lowercase() == "default" || payload.to_lowercase() == "reset" || payload.to_lowercase() == "none" {
                updated_config.brain.reasoning_effort = None;
                "Reasoning effort reset to default.".to_string()
            } else {
                updated_config.brain.reasoning_effort = Some(payload.to_string());
                format!("Reasoning effort updated: {}", payload)
            }
        }
        "/max_tokens" | "/max-tokens" | "/maxtokens" => {
            if payload.is_empty() {
                format!("Max Tokens: {}", config.brain.max_tokens)
            } else if let Ok(tokens) = payload.parse::<usize>() {
                updated_config.brain.max_tokens = tokens;
                format!("Max tokens updated: {}", tokens)
            } else {
                return Err(anyhow::anyhow!("Invalid token number: {}", payload));
            }
        }
        "/skills" => {
            if payload.is_empty() {
                format!("Skills Path: {}", config.skills.as_deref().unwrap_or("None"))
            } else {
                updated_config.skills = Some(payload.to_string());
                format!("Custom skills path updated: {}", payload)
            }
        }
        "/dir" => {
            if payload.is_empty() {
                format!("External Project Path: {}", config.knowledge.as_ref().and_then(|k| k.external_project.as_deref()).unwrap_or("None"))
            } else {
                let mut k_cfg = updated_config.knowledge.clone().unwrap_or_default();
                k_cfg.external_project = Some(payload.to_string());
                updated_config.knowledge = Some(k_cfg);
                format!("External project path updated: {}", payload)
            }
        }
        "/agents" | "/multi_agents" | "/multi-agents" => {
            if payload.is_empty() {
                let active = !config.multi_agents.unwrap_or(false);
                updated_config.multi_agents = Some(active);
                if active {
                    "Multi-agent mode enabled.".to_string()
                } else {
                    "Multi-agent mode disabled.".to_string()
                }
            } else {
                let lower = payload.to_lowercase();
                if lower == "on" || lower == "true" || lower == "enable" {
                    updated_config.multi_agents = Some(true);
                    "Multi-agent mode enabled.".to_string()
                } else if lower == "off" || lower == "false" || lower == "disable" {
                    updated_config.multi_agents = Some(false);
                    "Multi-agent mode disabled.".to_string()
                } else {
                    return Err(anyhow::anyhow!("Invalid option for multi-agent mode: {}. Use 'on' or 'off'.", payload));
                }
            }
        }
        _ => return Err(anyhow::anyhow!("Unknown slash command: {}", cmd)),
    };

    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(config_json) = serde_json::to_string_pretty(&updated_config) {
        let _ = std::fs::write(&config_path, config_json);
    }

    Ok(reply)
}

pub type ChatSessions = Arc<Mutex<HashMap<String, Vec<(String, String)>>>>;

pub async fn handle_chat_message(
    session_id: &str,
    user_message: &str,
    config: &UnevenConfig,
    sessions: ChatSessions,
) -> Result<String> {
    let root = loader::find_project_root(None);
    
    let trimmed = user_message.trim();
    if trimmed.starts_with('/') {
        let reply = handle_slash_command(trimmed, config, &root)?;
        return Ok(reply);
    }

    // Automatically run silent index to keep knowledge base up-to-date
    let _ = super::index::run(None, false, false, false, None, false).await;

    let brain = config.get_brain_for_command("chat");

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
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);

    // Initialize LLM
    init_llm(&brain.provider, &brain.model, threads).await?;

    // Get a copy of session history
    let mut history = {
        let mut map = sessions.lock().unwrap();
        map.entry(session_id.to_string()).or_default().clone()
    };

    let mut loaded_files: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut file_content_cache: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut attempts = 3;
    let mut force_answer = false;
    let mut final_raw_response_content = String::new();
    let base_history = history.clone();

    while attempts > 0 {
        // 1. Build context block from Cache 3
        let mut context_block = String::new();
        if !file_content_cache.is_empty() {
            context_block.push_str("## RELEVANT CONTEXT (KNOWLEDGE MAP)\n\n");
            for content in file_content_cache.values() {
                context_block.push_str(content);
            }
        }

        // Attach context block to user message
        let prompt_with_context = if context_block.is_empty() {
            user_message.to_string()
        } else {
            format!("{}\n{}", context_block, user_message)
        };

        let mut prompt_to_use = prompt_with_context.clone();
        let mut system_instructions_to_use = if force_answer {
            format!(
                "{}\n\n[SYSTEM WARNING: You already have access to the files: {:?}. \
                Do NOT request any more files using [NEED_FILE]. Answer the question now using ONLY the current context.]",
                system_instructions,
                loaded_files
            )
        } else {
            system_instructions.clone()
        };

        // If not local, optimize context using history
        let mut iter_history = base_history.clone();
        if brain.provider != "local" {
            // Pre-push user's turn with context to iter_history
            iter_history.push(("user".to_string(), prompt_with_context.clone()));

            let files_map = super::interactive::get_project_files_map(config, &root);
            prompt_to_use = optimize_context(
                &mut iter_history,
                &prompt_with_context,
                &system_instructions_to_use,
                Some(&files_map),
            );

            // Update the last turn in history to the optimized prompt
            if let Some(last_turn) = iter_history.last_mut() {
                if last_turn.0 == "user" {
                    last_turn.1 = prompt_to_use.clone();
                }
            }

            system_instructions_to_use = String::new();
        }

        let chat_template = uneven_core::infrastructure::llm::inference::chat_template::detect_template(&brain.model);
        let final_prompt = if brain.provider == "local" {
            let past_turns = if iter_history.len() > 1 {
                &iter_history[..iter_history.len() - 1]
            } else {
                &[]
            };
            let local_chat_history = past_turns.iter().map(|turn| {
                uneven_core::infrastructure::llm::inference::chat_template::ChatTurn {
                    role: turn.0.clone(),
                    content: turn.1.clone(),
                }
            }).collect::<Vec<_>>();
            uneven_core::infrastructure::llm::inference::chat_template::apply_template(
                &chat_template,
                &system_instructions_to_use,
                &local_chat_history,
                &prompt_to_use,
            )
        } else {
            prompt_to_use
        };

        let system_param = if system_instructions_to_use.is_empty() {
            None
        } else {
            Some(system_instructions_to_use)
        };

        // Exclude the current user turn from `history` parameter since it's passed as `prompt`
        let api_history = if brain.provider != "local" {
            if iter_history.len() > 1 {
                Some(iter_history[..iter_history.len() - 1].to_vec())
            } else {
                None
            }
        } else {
            None
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

        let response = infer_with_params(&final_prompt, &params).await?;

        // Parse tags
        let requested_files = super::interactive::parse_all_need_file_tags(&response.content);
        let requested_searches = super::interactive::parse_all_web_search_tags(&response.content);
        let requested_urls = super::interactive::parse_all_web_url_tags(&response.content);

        if requested_files.is_empty() && requested_searches.is_empty() && requested_urls.is_empty() {
            final_raw_response_content = response.content.clone();
            history = iter_history;
            if brain.provider == "local" {
                history.push(("user".to_string(), final_prompt.clone()));
            }
            break;
        }

        let new_files: std::collections::HashSet<String> = requested_files.difference(&loaded_files).cloned().collect();
        let new_searches: std::collections::HashSet<String> = requested_searches.difference(&loaded_files).cloned().collect();
        let new_urls: std::collections::HashSet<String> = requested_urls.difference(&loaded_files).cloned().collect();

        if new_files.is_empty() && new_searches.is_empty() && new_urls.is_empty() {
            // Loop detected
            force_answer = true;
            attempts = 1;
            continue;
        }

        // Load new resources
        for path in &new_files {
            if let Some(formatted) = super::interactive::load_and_format_file(&root, path) {
                file_content_cache.insert(path.clone(), formatted);
                loaded_files.insert(path.clone());
            }
        }
        for query in &new_searches {
            if let Some(formatted) = super::interactive::run_and_format_web_search(query, &root).await {
                file_content_cache.insert(format!("web_search_{}", query), formatted);
                loaded_files.insert(query.clone());
            }
        }
        for url in &new_urls {
            if let Some(formatted) = super::interactive::load_and_format_web_page(url).await {
                file_content_cache.insert(format!("web_url_{}", url), formatted);
                loaded_files.insert(url.clone());
            }
        }

        attempts -= 1;
    }

    // Check if the response contains multi_agents command
    if let Some(parsed) = extract_json_local(&final_raw_response_content) {
        if parsed.command == "multi_agents" || parsed.command == "multi-agents" || parsed.command == "multi_agent" {
            let config_brain_max_tokens = config.brain.max_tokens;
            if let Ok(reports) = crate::commands::multi_agents::run(&parsed.args, config_brain_max_tokens, config, session_id).await {
                let mut report_msg = String::new();
                report_msg.push_str("Multi-agent task execution completed.\n\n");
                for (i, (task_desc, status)) in reports.iter().enumerate() {
                    report_msg.push_str(&format!("Agent {} [Task: {}]:\n{}\n\n", i + 1, task_desc, status));
                }
                
                let feedback = format!(
                    "[System: All subagent tasks executed. Results are as follows. Please review them, make any adjustments if needed, and give a unified final explanation to the user:\n\n{}]",
                    report_msg
                );
                
                history.push(("user".to_string(), feedback.clone()));
                
                let params = InferParams {
                    model: brain.model.clone(),
                    threads,
                    max_tokens: brain.max_tokens,
                    temperature: brain.temperature as f64,
                    repeat_penalty: 1.1,
                    max_context: 4096,
                    grammar: None,
                    provider: Some(brain.provider.clone()),
                    system: Some(system_instructions.clone()),
                    history: Some(history.clone()),
                };
                
                let final_prompt = if brain.provider == "local" {
                    let local_turns = history.iter().map(|turn| {
                        uneven_core::infrastructure::llm::inference::chat_template::ChatTurn {
                            role: turn.0.clone(),
                            content: turn.1.clone(),
                        }
                    }).collect::<Vec<_>>();
                    let chat_template = uneven_core::infrastructure::llm::inference::chat_template::detect_template(&brain.model);
                    uneven_core::infrastructure::llm::inference::chat_template::apply_template(
                        &chat_template,
                        "", // System prompt is inside params
                        &local_turns[..local_turns.len()-1],
                        &feedback,
                    )
                } else {
                    feedback.clone()
                };
                
                if let Ok(new_res) = infer_with_params(&final_prompt, &params).await {
                    final_raw_response_content = new_res.content;
                }
            }
        }
    }

    let mut clean_response = final_raw_response_content.clone();

    // Check if the response contains any files generated using the FILE_SESSIONID format
    // Try to parse files using any alphanumeric 6-character session ID or generic formats
    let mut files = Vec::new();
    if let Ok(re_session) = regex::Regex::new(r"(?i)FILE_([a-zA-Z0-9]{6})") {
        if let Some(cap) = re_session.captures(&final_raw_response_content) {
            let session_id = cap[1].to_string();
            files = super::askf::parse_files_with_id(&final_raw_response_content, &session_id);
            if files.is_empty() {
                // Try to parse using a regex that allows END_SESSIONID
                let pattern = format!(
                    r#"(?i)FILE_{}[\"\']?\s*[:=]?\s*[\"\']?[<\[\{{\(]?([^\s\]\[}}{{\"\',;<>\(\)]+)[\"\']?\s*[>\]}})]?\s*\n\s*```[\w]*\n([\s\S]*?)```\s*END_(?:FILE_)?{}"#,
                    session_id, session_id
                );
                if let Ok(re) = regex::Regex::new(&pattern) {
                    for cap in re.captures_iter(&final_raw_response_content) {
                        let filename = cap[1].trim().to_string();
                        let content = cap[2].to_string();
                        if super::askf::is_safe_filename(&filename) {
                            files.push(super::askf::ParsedFile {
                                filename,
                                content,
                                patches: Vec::new(),
                                is_patch: false,
                                delete: false,
                                rename_to: None,
                            });
                        }
                    }
                }
            }
        }
    }

    if files.is_empty() {
        files = super::askf::parse_files(&final_raw_response_content);
    }

    if !files.is_empty() {
        let _ = super::askf::write_parsed_files(&root, files);

        // Strip FILE_SESSIONID blocks
        if let Ok(re) = regex::Regex::new(r#"(?i)FILE_[a-zA-Z0-9]{6}[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n\s*```[\s\S]*?```\s*END_(?:FILE_)?[a-zA-Z0-9]{6}\n?"#) {
            clean_response = re.replace_all(&clean_response, "").to_string();
        }
        // Strip generic FILE: blocks
        if let Ok(re) = regex::Regex::new(r#"(?i)FILE[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n```[\s\S]*?```\s*END_FILE\n?"#) {
            clean_response = re.replace_all(&clean_response, "").to_string();
        }
        if let Ok(re) = regex::Regex::new(r#"(?i)FILE[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n[\s\S]*?END_FILE\n?"#) {
            clean_response = re.replace_all(&clean_response, "").to_string();
        }
        if let Ok(re) = regex::Regex::new(r#"(?i)FILE_[A-Z0-9]+[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n\s*```[\s\S]*?```\s*END_(?:FILE_)?[A-Z0-9]+\n?"#) {
            clean_response = re.replace_all(&clean_response, "").to_string();
        }
        if let Ok(re) = regex::Regex::new(r#"(?i)FILE_[A-Z0-9]+[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n\s*PATCH\n[\s\S]*?END_(?:FILE_)?[A-Z0-9]+\n?"#) {
            clean_response = re.replace_all(&clean_response, "").to_string();
        }
        if let Ok(re) = regex::Regex::new(r#"(?i)FILE_[A-Z0-9]+[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n\s*DELETE\n\s*END_(?:FILE_)?[A-Z0-9]+\n?"#) {
            clean_response = re.replace_all(&clean_response, "").to_string();
        }
        if let Ok(re) = regex::Regex::new(r#"(?i)FILE_[A-Z0-9]+[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?[^\s\]\[\}\{\"\',;<>\(\)]+[\"\']?\s*[>\]\}\)]?\s*\n\s*RENAME:\s*\S+\n\s*END_(?:FILE_)?[A-Z0-9]+\n?"#) {
            clean_response = re.replace_all(&clean_response, "").to_string();
        }
        clean_response = clean_response.trim().to_string();
    }

    // Save updated history
    {
        let mut map = sessions.lock().unwrap();
        let session_history = map.entry(session_id.to_string()).or_default();
        
        *session_history = history;
        session_history.push(("assistant".to_string(), final_raw_response_content.clone()));

        let history_limit = if brain.provider == "local" { 8 } else { 14 };
        if session_history.len() > history_limit {
            session_history.drain(0..session_history.len() - history_limit);
        }
    }

    Ok(clean_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_handle_slash_command_person() {
        let dir = tempdir().unwrap();
        let config = UnevenConfig {
            persona: None,
            ..Default::default()
        };

        let reply = handle_slash_command("/person custom-persona", &config, dir.path()).unwrap();
        assert!(reply.contains("Custom agent persona updated"));

        let loaded = loader::load_config(dir.path()).unwrap();
        assert_eq!(loaded.persona.as_deref(), Some("custom-persona"));

        let reply = handle_slash_command("/person reset", &loaded, dir.path()).unwrap();
        assert!(reply.contains("persona reset to default"));
        
        let loaded_reset = loader::load_config(dir.path()).unwrap();
        assert!(loaded_reset.persona.is_none());
    }

    #[test]
    fn test_handle_slash_command_effort() {
        let dir = tempdir().unwrap();
        let mut config = UnevenConfig::default();
        config.brain.reasoning_effort = None;

        let reply = handle_slash_command("/effort high", &config, dir.path()).unwrap();
        assert!(reply.contains("Reasoning effort updated"));

        let loaded = loader::load_config(dir.path()).unwrap();
        assert_eq!(loaded.brain.reasoning_effort.as_deref(), Some("high"));
    }
}

#[derive(serde::Deserialize, Debug)]
struct BrainResponse {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    _message: String,
    #[serde(default)]
    _paths: Vec<String>,
}

fn extract_json_local(text: &str) -> Option<BrainResponse> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if start > end {
        return None;
    }
    let json_str = &text[start..=end];
    serde_json::from_str(json_str).ok()
}
