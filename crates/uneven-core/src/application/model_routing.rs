use crate::domain::config::UnevenConfig;
use crate::domain::{BrainConfig, LocalBrainConfig};
use crate::infrastructure::utils::hardware_detector::HardwareDetector;
use std::collections::HashMap;

pub fn match_model_string(s: &str) -> Result<(String, String), String> {
    let clean = s.to_lowercase().replace(' ', "-");

    let known_providers = [
        "gemini", "gpt", "openai", "claude", "sonnet", "haiku", "opus", "deepseek", "mistral",
        "groq", "ollama", "llama", "qwen",
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
            return Err(format!(
                "Unknown model provider in \"{}\". Did you mean \"{}\"?",
                s, suggestion
            ));
        } else {
            return Err(format!(
                "Unknown model provider in \"{}\". Please use a known provider (gemini, gpt, claude, deepseek, llama, qwen) or use the provider/model format (e.g. custom/model).",
                s
            ));
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
    } else if clean.contains("claude")
        || clean.contains("sonnet")
        || clean.contains("haiku")
        || clean.contains("opus")
    {
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
    } else if let Some((p, m)) = clean.split_once('/') {
        Ok((m.to_string(), p.to_string()))
    } else {
        Ok((clean, "local".to_string()))
    }
}

pub fn parse_multi_brains(
    trimmed_args: &str,
) -> Result<Vec<(Option<String>, String, String)>, String> {
    let words: Vec<&str> = trimmed_args.split_whitespace().collect();
    if words.is_empty() {
        return Ok(Vec::new());
    }

    let command_keywords = vec![
        "explain", "explan", "pentest", "scan", "watch", "autofix", "ask", "askf", "docs", "chat",
        "analyze", "zeroday",
    ];
    let provider_keywords = vec![
        "gemini", "gpt", "openai", "claude", "sonnet", "haiku", "opus", "deepseek", "mistral",
        "groq", "ollama", "llama", "qwen",
    ];

    let mut parsed = Vec::new();
    let mut current_command: Option<String> = None;
    let mut current_model_parts = Vec::new();

    for word in words {
        let clean_word = word.to_lowercase();

        let is_cmd = command_keywords.iter().any(|&c| clean_word == c);
        let has_slash = clean_word.contains('/');
        let is_provider = provider_keywords.iter().any(|&p| clean_word.contains(p));

        if is_cmd {
            if !current_model_parts.is_empty() {
                let model_str = current_model_parts.join(" ");
                let (m, p) = match_model_string(&model_str)?;
                parsed.push((current_command.take(), m, p));
                current_model_parts.clear();
            }
            let canonical_cmd = match clean_word.as_str() {
                "explan" => "explain",
                other => other,
            };
            current_command = Some(canonical_cmd.to_string());
        } else if has_slash || is_provider {
            if !current_model_parts.is_empty() {
                let model_str = current_model_parts.join(" ");
                let (m, p) = match_model_string(&model_str)?;
                parsed.push((current_command.take(), m, p));
                current_model_parts.clear();
            }
            current_model_parts.push(word);
        } else {
            current_model_parts.push(word);
        }
    }

    if !current_model_parts.is_empty() {
        let model_str = current_model_parts.join(" ");
        let (m, p) = match_model_string(&model_str)?;
        parsed.push((current_command.take(), m, p));
    }

    Ok(parsed)
}

pub fn allocate_brains(
    parsed: Vec<(Option<String>, String, String)>,
    config: &mut UnevenConfig,
) {
    if parsed.is_empty() {
        return;
    }

    let mut brains_list = Vec::new();
    let mut routing_map = HashMap::new();

    for (opt_cmd, m, p) in parsed.clone() {
        let brain_id = format!("{}-{}", p, m);
        let mut local_cfg = None;
        if p == "local" {
            let hw = HardwareDetector::new().detect_system().ok();
            let cpu_cores = hw.as_ref().map(|h| h.cpu_cores).unwrap_or(4);
            let threads = std::cmp::max(2, if cpu_cores > 2 { cpu_cores - 1 } else { cpu_cores }) as u32;
            let gpu_layers = hw.as_ref().map(|h| h.gpu.gpu_layers).unwrap_or(0);

            local_cfg = Some(LocalBrainConfig {
                threads: Some(threads),
                gpu_layers: Some(gpu_layers),
                unload_timeout: Some(10),
            });
        }

        let brain_cfg = BrainConfig {
            id: Some(brain_id.clone()),
            provider: p.clone(),
            model: m.clone(),
            api_key: None,
            temperature: 0.3,
            max_tokens: 2048,
            local: local_cfg,
            reasoning_effort: None,
        };

        if !brains_list.iter().any(|b: &BrainConfig| b.id == brain_cfg.id) {
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
            let score = |b: &BrainConfig| -> i32 {
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

                for cmd in &["askf", "watch", "autofix", "pentest", "scan", "zeroday"] {
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
                for cmd in &["pentest", "scan", "zeroday"] {
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
