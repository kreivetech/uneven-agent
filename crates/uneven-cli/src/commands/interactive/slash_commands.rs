use std::io::Write;
use console::style;
use uneven_core::UnevenConfig;
use uneven_core::application::model_routing::{allocate_brains, match_model_string, parse_multi_brains};
use uneven_core::infrastructure::knowledge::context_builder::sync_external_project;
use uneven_core::infrastructure::llm::init as init_llm;
use crate::ui::{self, truecolor};
use super::terminal_runner::{ensure_local_model_files, spawn_subcommand, spawn_subcommand_silent_in_dir};

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub enum SlashOutcome {
    Handled,
    Exit,
    Unknown,
}

#[allow(dead_code)]
pub fn save_config(root: &std::path::Path, config: &UnevenConfig) {
    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(config_json) = serde_json::to_string_pretty(config) {
        let _ = std::fs::write(&config_path, config_json);
    }
}

pub async fn handle_slash_command(
    trimmed: &str,
    root: &std::path::Path,
    config: &mut UnevenConfig,
    external_files_list: &mut Option<Vec<String>>,
) -> SlashOutcome {
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return SlashOutcome::Handled;
    }
    let slash_cmd = parts[0].to_lowercase();


        match slash_cmd.as_str() {
                "/export" => {
                    if parts.len() < 2 {
                        println!("\n  {} Usage: /export <VARIABLE_NAME> [value]\n", style("  ✗ ").red());
                        return SlashOutcome::Handled;
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
                        return SlashOutcome::Handled;
                    }

                    let global_env_path = uneven_core::infrastructure::utils::paths::global_bin_dir().join("env");
                    if let Some(parent) = global_env_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }

                    let mut lines = Vec::new();
                    if global_env_path.exists()
                        && let Ok(content) = std::fs::read_to_string(&global_env_path) {
                            for line in content.lines() {
                                let l_trim = line.trim();
                                if !l_trim.starts_with(&format!("{}=", key)) && !l_trim.is_empty() {
                                    lines.push(line.to_string());
                                }
                            }
                        }
                    lines.push(format!("{}={}", key, val_clean));
                    if std::fs::write(&global_env_path, lines.join("\n") + "\n").is_ok() {
                        unsafe { std::env::set_var(key, val_clean) };
                        println!("\n  {} {} saved successfully to the global environment!\n", ui::green("✓"), key);
                    } else {
                        println!("\n  {} Failed to save variable to the global environment\n", style("✗").red());
                    }
                }
                "/models" | "model" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    let mut selected_opt = None;

                    if !payload.is_empty() {
                        match parse_multi_brains(payload) {
                            Err(e) => {
                                println!("\n  {} {}\n", style("  ✗ ").red(), e);
                                return SlashOutcome::Handled;
                            }
                            Ok(parsed_brains) => {
                                if !parsed_brains.is_empty() {
                                    if slash_cmd == "/model" && parsed_brains.len() == 1 && parsed_brains[0].0.is_none() {
                                        let (_, m, p) = &parsed_brains[0];
                                        selected_opt = Some((m.clone(), p.clone()));
                                    } else {
                                        allocate_brains(parsed_brains, config);

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                                        return SlashOutcome::Handled;
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
                                                return SlashOutcome::Handled;
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
                        if p == "local"
                            && let Err(e) = ensure_local_model_files(&m).await {
                                println!("\n  {} {}\n", style("✗").red(), e);
                                return SlashOutcome::Handled;
                            }
                        config.brain.provider = p.clone();
                        config.brain.model = m.clone();
                        config.brains = None;
                        config.routing = None;

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                "/analyze" => {
                    if parts.len() < 2 {
                        println!("\n  {} Usage:\n", style("  ✗ ").red());
                        println!("     - /analyze db <url>               : Configure database URL");
                        println!("     - /analyze tables <t1> <t2> ...   : Configure allowed tables for reading");
                        println!("     - /analyze mask <c1> <c2> ...     : Mask values of specified columns");
                        println!("     - /analyze mask tables <t1> ... columns <c1> ... : Mask tables and columns separately\n");
                        return SlashOutcome::Handled;
                    }

                    let sub_action = parts[1].to_lowercase();

                    match sub_action.as_str() {
                        "db" => {
                            if parts.len() < 3 {
                                println!("\n  {} Please provide the database URL. Ex: /analyze db postgres://localhost/db\n", style("  ✗ ").red());
                                return SlashOutcome::Handled;
                            }
                            let db_url = parts[2..].join(" ");

                            let mut db_cfg = config.knowledge.as_ref()
                                .and_then(|k| k.db.clone())
                                .unwrap_or_default();
                            db_cfg.url = Some(db_url.clone());

                            let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                            k_cfg.db = Some(db_cfg);
                            config.knowledge = Some(k_cfg);

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            println!("\n  {} Database configured to: {}\n", ui::green("✓"), db_url);
                        }
                        "tables" => {
                            if parts.len() < 3 {
                                println!("\n  {} Please provide the table names. Ex: /analyze tables products sales\n", style("  ✗ ").red());
                                return SlashOutcome::Handled;
                            }
                            let tables: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();

                            let mut db_cfg = config.knowledge.as_ref()
                                .and_then(|k| k.db.clone())
                                .unwrap_or_default();
                            db_cfg.allowed_tables = Some(tables.clone());

                            let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                            k_cfg.db = Some(db_cfg);
                            config.knowledge = Some(k_cfg);

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                            if let Some(parent) = config_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                let _ = std::fs::write(&config_path, config_json);
                            }
                            println!("\n  {} Allowed tables updated: {}\n", ui::green("✓"), tables.join(", "));
                        }
                        "mask" => {
                            if parts.len() < 3 {
                                println!("\n  {} Please provide items to mask. Ex: /analyze mask cpf password\n", style("  ✗ ").red());
                                return SlashOutcome::Handled;
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

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                                if let Some(parent) = config_path.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                    let _ = std::fs::write(&config_path, config_json);
                                }
                                println!("\n  {} Masked columns updated: {}\n", ui::green("✓"), columns.join(", "));
                            }
                        }
                        _ => {
                            println!("\n  {} Unknown subcommand. Options: db, file, tables, mask\n", style("  ✗ ").red());
                        }
                    }

                }
                "/pentest" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    if !payload.is_empty() {
                        let sub_parts: Vec<&str> = payload.split_whitespace().collect();
                        if sub_parts[0].to_lowercase() == "target" {
                            if sub_parts.len() < 2 {
                                println!("\n  {} Usage: /pentest target <target_url> [scope_file]\n", style("  ✗ ").red());
                                return SlashOutcome::Handled;
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

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                        return SlashOutcome::Handled;
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
                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                }
                "/scan" => {
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
                    return SlashOutcome::Handled;
                }
                "/zeroday" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    let is_deep = payload.contains("--deep");
                    let clean_payload = payload.replace("--deep", "").trim().to_string();
                    let dir_arg = if !clean_payload.is_empty() {
                        Some(clean_payload)
                    } else {
                        None
                    };
                    let mode_label = if is_deep { "deep autonomous Think audit" } else { "scan" };
                    println!("{}", truecolor(&format!("\n  ↳ Scanning project files for custom zero-day vulnerability flows ({})... \n", mode_label), 107, 122, 150));
                    let _ = crate::commands::zeroday::run(dir_arg, None, is_deep).await;
                    return SlashOutcome::Handled;
                }
                "/fix" => {
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
                            return SlashOutcome::Handled;
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
                                return SlashOutcome::Handled;
                            }
                        } else if action_lower == "confirm" {
                            if let Ok(val) = sub_parts[val_idx].parse::<bool>() {
                                w_cfg.confirm_before_fix = Some(val);
                            } else {
                                println!("\n  {} Please provide true or false.\n", style("  ✗ ").red());
                                return SlashOutcome::Handled;
                            }
                        } else {
                            println!("\n  {} Unknown action. Options: terminal, dirs, autofix, confirm\n", style("  ✗ ").red());
                            return SlashOutcome::Handled;
                        }

                        config.watch = Some(w_cfg.clone());
                        config.proactive = Some(uneven_core::domain::ProactiveConfig {
                            enabled: Some(w_cfg.auto_fix.unwrap_or(true)),
                            auto_fix: w_cfg.auto_fix,
                            confirm_before_fix: w_cfg.confirm_before_fix,
                            dirs: w_cfg.dirs.clone(),
                            ..Default::default()
                        });

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }
                        println!("\n  {} Watch and Auto-Fix settings updated successfully!\n", ui::green("✓"));
                        return SlashOutcome::Handled;
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

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                    return SlashOutcome::Handled;
                }
                "/urls" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    if !payload.is_empty() {
                        let urls: Vec<String> = payload.split_whitespace().map(|s| s.to_string()).collect();
                        let mut k_cfg = config.knowledge.clone().unwrap_or_default();
                        k_cfg.urls = Some(urls.clone());
                        config.knowledge = Some(k_cfg);

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }
                        println!("\n  {} Documentation URLs updated: {}\n", ui::green("✓"), urls.join(", "));
                        return SlashOutcome::Handled;
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

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                    return SlashOutcome::Handled;
                }
                "/dir" | "/directory" | "/folder" => {
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

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }

                        if let Some(files) = sync_external_project(&resolved_str) {
                            *external_files_list = Some(files);
                            let ext_dir = resolved_path.clone();
                            tokio::spawn(async move {
                                let _ = spawn_subcommand_silent_in_dir(&["index"], &ext_dir).await;
                            });
                            println!("\n  {} External project path updated and synced: {}\n", ui::green("✓"), resolved_str);
                        } else {
                            println!("\n  {} Failed to sync external project: {}\n", style("✗").red(), resolved_str);
                        }
                        return SlashOutcome::Handled;
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

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                                if let Some(parent) = config_path.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                    let _ = std::fs::write(&config_path, config_json);
                                }

                                if let Some(ref resolved_str) = temp_dir {
                                    if let Some(files) = sync_external_project(resolved_str) {
                                        *external_files_list = Some(files);
                                        let ext_dir = std::path::PathBuf::from(resolved_str);
                                        tokio::spawn(async move {
                                            let _ = spawn_subcommand_silent_in_dir(&["index"], &ext_dir).await;
                                        });
                                    }
                                } else {
                                    *external_files_list = None;
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
                    return SlashOutcome::Handled;
                }
                "/skills" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    if !payload.is_empty() {
                        config.skills = Some(payload.to_string());

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }
                        println!("\n  {} Custom skills path updated: {}\n", ui::green("✓"), payload);
                        return SlashOutcome::Handled;
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

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                    return SlashOutcome::Handled;
                }
                "/person" | "/persona" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    if !payload.is_empty() {
                        if payload.to_lowercase() == "default" || payload.to_lowercase() == "reset" {
                            config.persona = None;
                            println!("\n  {} Custom agent persona reset to default (Senior Engineer).\n", ui::green("✓"));
                        } else {
                            config.persona = Some(payload.to_string());
                            println!("\n  {} Custom agent persona updated: {}\n", ui::green("✓"), payload);
                        }

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }
                        return SlashOutcome::Handled;
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

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                    return SlashOutcome::Handled;
                }
                "/max-tokens" | "/max_tokens" | "/maxtokens" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    if !payload.is_empty() {
                        if let Ok(tokens) = payload.parse::<usize>() {
                            config.brain.max_tokens = tokens;

                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                        return SlashOutcome::Handled;
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

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                    return SlashOutcome::Handled;
                }
                "/effort" | "/reasoning" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    if !payload.is_empty() {
                        if payload.to_lowercase() == "default" || payload.to_lowercase() == "reset" || payload.to_lowercase() == "none" {
                            config.brain.reasoning_effort = None;
                            println!("\n  {} Reasoning effort reset to default.\n", ui::green("✓"));
                        } else {
                            config.brain.reasoning_effort = Some(payload.to_string());
                            println!("\n  {} Reasoning effort updated: {}\n", ui::green("✓"), payload);
                        }

                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }
                        return SlashOutcome::Handled;
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

                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                    return SlashOutcome::Handled;
                }
                "/agents" | "/multi_agents" | "/multi-agents" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    if !payload.is_empty() {
                        let lower = payload.to_lowercase();
                        let active = if lower == "on" || lower == "true" || lower == "enable" {
                            true
                        } else if lower == "off" || lower == "false" || lower == "disable" {
                            false
                        } else {
                            println!("\n  {} Invalid option for multi-agent mode: {}. Use 'on' or 'off'.\n", style("✗").red(), payload);
                            return SlashOutcome::Handled;
                        };

                        config.multi_agents = Some(active);
                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(&config_path, config_json);
                        }
                        let msg = if active { "enabled" } else { "disabled" };
                        println!("\n  {} Multi-agent mode {} successfully!\n", ui::green("✓"), msg);
                        return SlashOutcome::Handled;
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
                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                    return SlashOutcome::Handled;
                }
                "/stats" | "/cost" | "/tokens" | "/usage" => {
                    let payload = trimmed.strip_prefix(parts[0]).unwrap().trim();
                    if !payload.is_empty() {
                        let mut stats = uneven_core::infrastructure::llm::pricing::load_stats(root);
                        let mut is_limit_update = false;
                        let target_val = payload.strip_prefix("limit").unwrap_or(payload).trim();
                        if let Ok(val) = target_val.parse::<f64>() {
                            stats.monthly_limit_usd = val;
                            let _ = uneven_core::infrastructure::llm::pricing::save_stats(root, &stats);
                            println!("\n  {} Monthly budget limit updated to ${:.2} USD!\n", ui::green("✓"), val);
                            is_limit_update = true;
                        }
                        if is_limit_update {
                            return SlashOutcome::Handled;
                        }
                    }
                    let _ = crate::commands::stats::run().await;
                    return SlashOutcome::Handled;
                }
                "/" => {
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
                        "/tokens     Token Usage & Quotas",
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
                                    if p == "local"
                                        && let Err(e) = ensure_local_model_files(&m).await {
                                            println!("\n  {} {}\n", style("✗").red(), e);
                                            return SlashOutcome::Handled;
                                        }
                                    config.brain.provider = p.clone();
                                    config.brain.model = m.clone();
                                    config.brains = None;
                                    config.routing = None;

                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                                            if global_env_path.exists()
                                                && let Ok(content) = std::fs::read_to_string(&global_env_path) {
                                                    for line in content.lines() {
                                                        let l_trim = line.trim();
                                                        if !l_trim.starts_with(&format!("{}=", key)) && !l_trim.is_empty() {
                                                            lines.push(line.to_string());
                                                        }
                                                    }
                                                }
                                            lines.push(format!("{}={}", key, val));
                                            if std::fs::write(&global_env_path, lines.join("\n") + "\n").is_ok() {
                                                unsafe { std::env::set_var(&key, val) };
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

                                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                            let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                                    let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                            let _ = crate::commands::zeroday::run(None, None, false).await;
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

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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

                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
                                        if let Some(parent) = config_path.parent() {
                                            let _ = std::fs::create_dir_all(parent);
                                        }
                                        if let Ok(config_json) = serde_json::to_string_pretty(&config) {
                                            let _ = std::fs::write(&config_path, config_json);
                                        }

                                        if let Some(ref resolved_str) = temp_dir {
                                            if let Some(files) = sync_external_project(resolved_str) {
                                                *external_files_list = Some(files);
                                                let ext_dir = std::path::PathBuf::from(resolved_str);
                                                tokio::spawn(async move {
                                                    let _ = spawn_subcommand_silent_in_dir(&["index"], &ext_dir).await;
                                                });
                                            }
                                        } else {
                                            *external_files_list = None;
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
                                        let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(root).join("config.json");
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
                            let _ = crate::commands::stats::run().await;
                        }
                        _ => {}
                    }
                }
                _ => {
                    println!("\n  {} Unknown slash command. Available commands: /models, /model, /export, /analyze, /pentest, /scan, /zeroday, /fix, /urls, /skills, /person, /max_tokens, /effort, /dir, /agents, /tokens, /stats\n", style("✗").red());
                    return SlashOutcome::Unknown;
                }
            }

    SlashOutcome::Handled
}
