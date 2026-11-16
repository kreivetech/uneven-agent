use crate::domain::{UnevenConfig, UnevenError};
use std::fs;
use std::path::{Path, PathBuf};

const TS_CONFIG_FILE: &str = "uneven.config.ts";

/// Ascending search for project root.
/// Looks for .uneven/ directory or uneven.config.ts.
/// Falls back to process.cwd() (current directory) if not found.
pub fn find_project_root(start_dir: Option<&str>) -> PathBuf {
    let start = match start_dir {
        Some(d) => PathBuf::from(d),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let mut current = fs::canonicalize(&start).unwrap_or(start);

    loop {
        let uneven_dir = current.join(".uneven");
        let ts_config = current.join(TS_CONFIG_FILE);

        if uneven_dir.is_dir() || ts_config.is_file() {
            return current;
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn get_defaults() -> UnevenConfig {
    UnevenConfig {
        project_root: ".".to_string(),
        brain: crate::domain::config::BrainConfig {
            id: None,
            provider: "gemini".to_string(),
            model: "gemini-2.5-flash".to_string(),
            api_key: None,
            temperature: 0.3,
            max_tokens: 2048,
            local: None,
            reasoning_effort: None,
        },
        brains: None,
        routing: None,
        knowledge: Some(crate::domain::config::KnowledgeConfig {
            dirs: Some(vec!["./".to_string()]),
            files: Some(vec![]),
            db: None,
            urls: Some(vec![]),
            external_project: None,
        }),
        watch: Some(crate::domain::config::WatchConfig {
            terminal: Some("npm run dev".to_string()),
            terminals: None,
            ignore_patterns: None,
            auto_fix: Some(true),
            confirm_before_fix: Some(false),
            alert_only: Some(false),
            dirs: Some(vec!["./".to_string()]),
            only_use_strategies: Some(false),
            low_cpu_mode: Some(false),
        }),
        pentester: None,
        log: Some(crate::domain::config::LogConfig {
            path: Some("./.uneven/log.md".to_string()),
            append: Some(true),
            include_timestamp: Some(true),
            include_diff: Some(true),
        }),
        notifications: None,
        security: crate::domain::config::DataSecurityConfig {
            mask: None,
            block: None,
            blocked_columns: vec![],
            blocked_tables: vec![],
            masked_columns: vec![],
            report_redactions: Some(true),
        },
        proactive: Some(crate::domain::config::ProactiveConfig {
            enabled: Some(true),
            auto_fix: Some(true),
            confirm_before_fix: Some(false),
            dirs: Some(vec!["./".to_string()]),
            ignore: Some(vec![]),
            vendor: Some(vec![]),
            only_use_strategies: Some(false),
            low_cpu_mode: Some(false),
        }),
        zeroday: Some(crate::domain::config::ZeroDayConfig {
            enabled: Some(true),
            dirs: Some(vec!["./".to_string()]),
            low_cpu_mode: Some(false),
        }),
        skills: None,
        persona: None,
        multi_agents: None,
    }
}

/// Recursively overrides keys in `a` with those in `b`.
fn override_json_values(a: &mut serde_json::Value, b: &serde_json::Value) {
    match (a, b) {
        (serde_json::Value::Object(a_map), serde_json::Value::Object(b_map)) => {
            for (k, v) in b_map {
                if let Some(a_val) = a_map.get_mut(k) {
                    override_json_values(a_val, v);
                } else {
                    a_map.insert(k.clone(), v.clone());
                }
            }
        }
        (a_val, b_val) => {
            *a_val = b_val.clone();
        }
    }
}

/// Synchronizes uneven.config.ts properties into .uneven/config.json
pub fn sync_config_from_ts(project_root: &Path) -> Result<(), UnevenError> {
    let overlay = parse_config_ts(project_root);
    if overlay.is_none() {
        return Ok(());
    }
    let overlay_val = overlay.unwrap();

    let config_path =
        crate::infrastructure::utils::paths::project_cache_dir(project_root).join("config.json");
    let mut current_json = String::new();
    let current_opt = if let Ok(content) = fs::read_to_string(&config_path) {
        current_json = content;
        serde_json::from_str::<serde_json::Value>(&current_json).ok()
    } else {
        None
    };

    let defaults_val = serde_json::to_value(get_defaults())?;
    let mut merged = defaults_val;

    if let Some(current_val) = current_opt {
        override_json_values(&mut merged, &current_val);
    }
    override_json_values(&mut merged, &overlay_val);

    let merged_json = serde_json::to_string_pretty(&merged)?;
    if merged_json != current_json {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = config_path.with_extension("tmp");
        fs::write(&tmp_path, &merged_json)?;
        fs::rename(&tmp_path, &config_path)?;
    }

    Ok(())
}

/// Loads configuration from project root
pub fn load_config(project_root: &Path) -> Result<UnevenConfig, UnevenError> {
    let _ = sync_config_from_ts(project_root);

    let config_path =
        crate::infrastructure::utils::paths::project_cache_dir(project_root).join("config.json");
    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: UnevenConfig = serde_json::from_str(&content)?;
        Ok(config)
    } else {
        Ok(get_defaults())
    }
}

/// Persists configuration to project cache config.json
pub fn save_config(project_root: &Path, config: &UnevenConfig) -> Result<(), UnevenError> {
    let config_path =
        crate::infrastructure::utils::paths::project_cache_dir(project_root).join("config.json");
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    let tmp_path = config_path.with_extension("tmp");
    fs::write(&tmp_path, content)?;
    fs::rename(&tmp_path, &config_path)?;
    Ok(())
}

// ─── uneven.config.ts regex-based parser ──────────────────────────────────────

fn extract_section(src: &str, key: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(r"\b{}\s*:\s*\{{", key)).ok()?;
    let m = re.find(src)?;
    let start_idx = m.end() - 1; // '{'

    let mut depth = 0;
    let mut in_str = false;
    let mut str_char = ' ';
    let chars: Vec<char> = src.chars().collect();
    let mut char_idx_map = Vec::new();
    let mut byte_idx = 0;
    for c in src.chars() {
        char_idx_map.push(byte_idx);
        byte_idx += c.len_utf8();
    }

    let start_char_pos = src[..start_idx].chars().count();

    for i in start_char_pos..chars.len() {
        let ch = chars[i];
        if in_str {
            if ch == str_char {
                let mut backslashes = 0;
                let mut k = (i as isize) - 1;
                while k >= 0 && chars[k as usize] == '\\' {
                    backslashes += 1;
                    k -= 1;
                }
                if backslashes % 2 == 0 {
                    in_str = false;
                }
            }
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            in_str = true;
            str_char = ch;
            continue;
        }
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                let end_idx = char_idx_map[i];
                return Some(src[start_idx + 1..end_idx].to_string());
            }
        }
    }
    None
}

fn extract_blocks(src: &str, key: &str) -> Vec<String> {
    let re = match regex::Regex::new(&format!(r"\b{}\s*:\s*\[", key)) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let m = match re.find(src) {
        Some(m) => m,
        None => return Vec::new(),
    };
    let start_idx = m.end() - 1; // '['

    let mut blocks = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let mut in_str = false;
    let mut str_char = ' ';
    let chars: Vec<char> = src.chars().collect();
    let mut char_idx_map = Vec::new();
    let mut byte_idx = 0;
    for c in src.chars() {
        char_idx_map.push(byte_idx);
        byte_idx += c.len_utf8();
    }

    let start_char_pos = src[..start_idx].chars().count();

    for i in start_char_pos..chars.len() {
        let ch = chars[i];
        if in_str {
            if ch == str_char {
                let mut backslashes = 0;
                let mut k = (i as isize) - 1;
                while k >= 0 && chars[k as usize] == '\\' {
                    backslashes += 1;
                    k -= 1;
                }
                if backslashes % 2 == 0 {
                    in_str = false;
                }
            }
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            in_str = true;
            str_char = ch;
            continue;
        }
        if ch == '{' {
            if depth == 0 {
                start = char_idx_map[i] + 1;
            }
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                let end_idx = char_idx_map[i];
                blocks.push(src[start..end_idx].to_string());
            }
        } else if ch == ']' && depth == 0 {
            break;
        }
    }
    blocks
}

fn str_val(section: &str, key: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(r#"\b{}\s*:\s*["'`]([^"'`]*)["'`]"#, key)).ok()?;
    let caps = re.captures(section);
    let c = caps?;
    Some(c.get(1)?.as_str().to_string())
}

fn num_val(section: &str, key: &str) -> Option<f64> {
    let re = regex::Regex::new(&format!(r#"\b{}\s*:\s*([\d.]+)"#, key)).ok()?;
    let caps = re.captures(section)?;
    caps.get(1)?.as_str().parse::<f64>().ok()
}

fn bool_val(section: &str, key: &str) -> Option<bool> {
    let re = regex::Regex::new(&format!(r#"\b{}\s*:\s*(true|false)"#, key)).ok()?;
    let caps = re.captures(section)?;
    Some(caps.get(1)?.as_str() == "true")
}

fn map_val(section: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Ok(re) = regex::Regex::new(r#"\b(\w+)\s*:\s*["'`]([^"'`]*)["'`]"#) {
        for caps in re.captures_iter(section) {
            if let (Some(k), Some(v)) = (caps.get(1), caps.get(2)) {
                map.insert(k.as_str().to_string(), v.as_str().to_string());
            }
        }
    }
    map
}

fn arr_val(section: &str, key: &str) -> Option<Vec<String>> {
    let re = regex::Regex::new(&format!(r#"\b{}\s*:\s*\[([^\]]*)\]"#, key)).ok()?;
    let caps = re.captures(section)?;
    let val_str = caps.get(1)?.as_str();
    let list = val_str
        .split(',')
        .map(|s| {
            s.trim()
                .trim_matches(|c| c == '\'' || c == '"' || c == '`')
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();
    Some(list)
}

fn parse_config_ts(project_root: &Path) -> Option<serde_json::Value> {
    let ts_path = project_root.join(TS_CONFIG_FILE);
    let src = fs::read_to_string(ts_path).ok()?;
    let mut map = serde_json::Map::new();

    // brain (single or array)
    let mut brain_blocks = extract_blocks(&src, "brains");
    if brain_blocks.is_empty() {
        brain_blocks = extract_blocks(&src, "brain");
    }
    if !brain_blocks.is_empty() {
        let mut brains_vec = Vec::new();
        for block in &brain_blocks {
            let mut brain_map = serde_json::Map::new();
            if let Some(id) = str_val(block, "id") {
                brain_map.insert("id".to_string(), serde_json::Value::String(id));
            }
            if let Some(provider) = str_val(block, "provider") {
                brain_map.insert("provider".to_string(), serde_json::Value::String(provider));
            }
            if let Some(model) = str_val(block, "model") {
                brain_map.insert("model".to_string(), serde_json::Value::String(model));
            }
            if let Some(temp) = num_val(block, "temperature") {
                brain_map.insert("temperature".to_string(), serde_json::json!(temp));
            }
            if let Some(max_tok) = num_val(block, "maxTokens") {
                brain_map.insert("maxTokens".to_string(), serde_json::json!(max_tok as usize));
            }
            if let Some(re) =
                str_val(block, "reasoningEffort").or_else(|| str_val(block, "reasoning_effort"))
            {
                brain_map.insert("reasoningEffort".to_string(), serde_json::Value::String(re));
            }
            if let Some(local_src) = extract_section(block, "local") {
                let mut local_map = serde_json::Map::new();
                if let Some(t) = num_val(&local_src, "threads") {
                    local_map.insert("threads".to_string(), serde_json::json!(t as u32));
                }
                if let Some(gl) = num_val(&local_src, "gpuLayers") {
                    local_map.insert("gpuLayers".to_string(), serde_json::json!(gl as u32));
                }
                if let Some(ut) = num_val(&local_src, "unloadTimeout") {
                    local_map.insert("unloadTimeout".to_string(), serde_json::json!(ut as u64));
                }
                brain_map.insert("local".to_string(), serde_json::Value::Object(local_map));
            }
            brains_vec.push(serde_json::Value::Object(brain_map));
        }
        map.insert(
            "brains".to_string(),
            serde_json::Value::Array(brains_vec.clone()),
        );
        if !brains_vec.is_empty() {
            map.insert("brain".to_string(), brains_vec[0].clone());
        }
    } else if let Some(brain_src) = extract_section(&src, "brain") {
        let mut brain_map = serde_json::Map::new();
        if let Some(provider) = str_val(&brain_src, "provider") {
            brain_map.insert("provider".to_string(), serde_json::Value::String(provider));
        }
        if let Some(model) = str_val(&brain_src, "model") {
            brain_map.insert("model".to_string(), serde_json::Value::String(model));
        }
        if let Some(temp) = num_val(&brain_src, "temperature") {
            brain_map.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(max_tok) = num_val(&brain_src, "maxTokens") {
            brain_map.insert("maxTokens".to_string(), serde_json::json!(max_tok as usize));
        }
        if let Some(re) = str_val(&brain_src, "reasoningEffort")
            .or_else(|| str_val(&brain_src, "reasoning_effort"))
        {
            brain_map.insert("reasoningEffort".to_string(), serde_json::Value::String(re));
        }
        if let Some(local_src) = extract_section(&brain_src, "local") {
            let mut local_map = serde_json::Map::new();
            if let Some(t) = num_val(&local_src, "threads") {
                local_map.insert("threads".to_string(), serde_json::json!(t as u32));
            }
            if let Some(gl) = num_val(&local_src, "gpuLayers") {
                local_map.insert("gpuLayers".to_string(), serde_json::json!(gl as u32));
            }
            if let Some(ut) = num_val(&local_src, "unloadTimeout") {
                local_map.insert("unloadTimeout".to_string(), serde_json::json!(ut as u64));
            }
            brain_map.insert("local".to_string(), serde_json::Value::Object(local_map));
        }
        map.insert("brain".to_string(), serde_json::Value::Object(brain_map));
    }

    // routing
    if let Some(routing_src) = extract_section(&src, "routing") {
        let routing_map = map_val(&routing_src);
        let mut json_routing = serde_json::Map::new();
        for (k, v) in routing_map {
            json_routing.insert(k, serde_json::Value::String(v));
        }
        map.insert(
            "routing".to_string(),
            serde_json::Value::Object(json_routing),
        );
    }

    // knowledge
    if let Some(knowledge_src) = extract_section(&src, "knowledge") {
        let mut k_map = serde_json::Map::new();
        if let Some(dirs) = arr_val(&knowledge_src, "dirs") {
            k_map.insert("dirs".to_string(), serde_json::json!(dirs));
        }
        if let Some(files) = arr_val(&knowledge_src, "files") {
            k_map.insert("files".to_string(), serde_json::json!(files));
        }
        if let Some(urls) = arr_val(&knowledge_src, "urls") {
            k_map.insert("urls".to_string(), serde_json::json!(urls));
        }
        if let Some(db_src) = extract_section(&knowledge_src, "db") {
            let mut db_map = serde_json::Map::new();
            if let Some(url) = str_val(&db_src, "url") {
                db_map.insert("url".to_string(), serde_json::Value::String(url));
            }
            if let Some(tables) = arr_val(&db_src, "tables") {
                db_map.insert("allowedTables".to_string(), serde_json::json!(tables));
            }
            k_map.insert("db".to_string(), serde_json::Value::Object(db_map));
        }
        map.insert("knowledge".to_string(), serde_json::Value::Object(k_map));
    }

    // watch
    if let Some(watch_src) = extract_section(&src, "watch") {
        let mut w_map = serde_json::Map::new();
        if let Some(terminal) = str_val(&watch_src, "terminal") {
            w_map.insert("terminal".to_string(), serde_json::Value::String(terminal));
        }
        if let Some(terminals) = arr_val(&watch_src, "terminals") {
            w_map.insert("terminals".to_string(), serde_json::json!(terminals));
        }
        if let Some(ignore) = arr_val(&watch_src, "ignorePatterns") {
            w_map.insert("ignorePatterns".to_string(), serde_json::json!(ignore));
        }
        if let Some(af) = bool_val(&watch_src, "autoFix") {
            w_map.insert("autoFix".to_string(), serde_json::json!(af));
        }
        if let Some(cbf) = bool_val(&watch_src, "confirmBeforeFix") {
            w_map.insert("confirmBeforeFix".to_string(), serde_json::json!(cbf));
        }
        if let Some(ao) = bool_val(&watch_src, "alertOnly") {
            w_map.insert("alertOnly".to_string(), serde_json::json!(ao));
        }
        if let Some(dirs) = arr_val(&watch_src, "dirs") {
            w_map.insert("dirs".to_string(), serde_json::json!(dirs));
        }
        if let Some(ous) = bool_val(&watch_src, "onlyUseStrategies") {
            w_map.insert("onlyUseStrategies".to_string(), serde_json::json!(ous));
        }
        if let Some(lcm) = bool_val(&watch_src, "lowCpuMode") {
            w_map.insert("lowCpuMode".to_string(), serde_json::json!(lcm));
        }
        map.insert("watch".to_string(), serde_json::Value::Object(w_map));
    }

    // proactive
    if let Some(proactive_src) = extract_section(&src, "proactive") {
        let mut p_map = serde_json::Map::new();
        if let Some(enabled) = bool_val(&proactive_src, "enabled") {
            p_map.insert("enabled".to_string(), serde_json::json!(enabled));
        }
        if let Some(af) = bool_val(&proactive_src, "autoFix") {
            p_map.insert("autoFix".to_string(), serde_json::json!(af));
        }
        if let Some(cbf) = bool_val(&proactive_src, "confirmBeforeFix") {
            p_map.insert("confirmBeforeFix".to_string(), serde_json::json!(cbf));
        }
        if let Some(dirs) = arr_val(&proactive_src, "dirs") {
            p_map.insert("dirs".to_string(), serde_json::json!(dirs));
        }
        if let Some(ignore) = arr_val(&proactive_src, "ignore") {
            p_map.insert("ignore".to_string(), serde_json::json!(ignore));
        }
        if let Some(ous) = bool_val(&proactive_src, "onlyUseStrategies") {
            p_map.insert("onlyUseStrategies".to_string(), serde_json::json!(ous));
        }
        if let Some(lcm) = bool_val(&proactive_src, "lowCpuMode") {
            p_map.insert("lowCpuMode".to_string(), serde_json::json!(lcm));
        }
        map.insert("proactive".to_string(), serde_json::Value::Object(p_map));
    }

    // zeroday
    if let Some(zeroday_src) = extract_section(&src, "zeroday") {
        let mut zd_map = serde_json::Map::new();
        if let Some(enabled) = bool_val(&zeroday_src, "enabled") {
            zd_map.insert("enabled".to_string(), serde_json::json!(enabled));
        }
        if let Some(dirs) = arr_val(&zeroday_src, "dirs") {
            zd_map.insert("dirs".to_string(), serde_json::json!(dirs));
        }
        if let Some(lcm) = bool_val(&zeroday_src, "lowCpuMode") {
            zd_map.insert("lowCpuMode".to_string(), serde_json::json!(lcm));
        }
        map.insert("zeroday".to_string(), serde_json::Value::Object(zd_map));
    }

    // log
    if let Some(log_src) = extract_section(&src, "log") {
        let mut l_map = serde_json::Map::new();
        if let Some(p) = str_val(&log_src, "path") {
            l_map.insert("path".to_string(), serde_json::Value::String(p));
        }
        if let Some(a) = bool_val(&log_src, "append") {
            l_map.insert("append".to_string(), serde_json::json!(a));
        }
        if let Some(it) = bool_val(&log_src, "includeTimestamp") {
            l_map.insert("includeTimestamp".to_string(), serde_json::json!(it));
        }
        if let Some(id) = bool_val(&log_src, "includeDiff") {
            l_map.insert("includeDiff".to_string(), serde_json::json!(id));
        }
        map.insert("log".to_string(), serde_json::Value::Object(l_map));
    }

    // notifications
    if let Some(notifications_src) = extract_section(&src, "notifications") {
        let mut n_map = serde_json::Map::new();
        if let Some(url) = str_val(&notifications_src, "webhookUrl") {
            n_map.insert("webhookUrl".to_string(), serde_json::Value::String(url));
        }
        if let Some(sev) = str_val(&notifications_src, "minSeverity") {
            n_map.insert("minSeverity".to_string(), serde_json::Value::String(sev));
        }
        map.insert(
            "notifications".to_string(),
            serde_json::Value::Object(n_map),
        );
    }

    // security
    if let Some(security_src) = extract_section(&src, "security") {
        let mut s_map = serde_json::Map::new();
        if let Some(mask) = arr_val(&security_src, "mask") {
            s_map.insert("mask".to_string(), serde_json::json!(mask));
        }
        if let Some(block) = arr_val(&security_src, "block") {
            s_map.insert("block".to_string(), serde_json::json!(block));
        }
        if let Some(blocked_columns) = arr_val(&security_src, "blockedColumns") {
            s_map.insert(
                "blockedColumns".to_string(),
                serde_json::json!(blocked_columns),
            );
        }
        if let Some(masked_columns) = arr_val(&security_src, "maskedColumns") {
            s_map.insert(
                "maskedColumns".to_string(),
                serde_json::json!(masked_columns),
            );
        }

        if let Some(blocked_tables) = arr_val(&security_src, "blockedTables") {
            s_map.insert(
                "blockedTables".to_string(),
                serde_json::json!(blocked_tables),
            );
        }
        if let Some(rr) = bool_val(&security_src, "reportRedactions") {
            s_map.insert("reportRedactions".to_string(), serde_json::json!(rr));
        }
        map.insert("security".to_string(), serde_json::Value::Object(s_map));
    }

    // pentester
    if let Some(pentester_src) = extract_section(&src, "pentester") {
        let mut p_map = serde_json::Map::new();
        if let Some(enabled) = bool_val(&pentester_src, "enabled") {
            p_map.insert("enabled".to_string(), serde_json::json!(enabled));
        }
        if let Some(mode) = str_val(&pentester_src, "mode") {
            p_map.insert("mode".to_string(), serde_json::Value::String(mode));
        }
        if let Some(target) = str_val(&pentester_src, "target") {
            p_map.insert("target".to_string(), serde_json::Value::String(target));
        }
        if let Some(rt) = bool_val(&pentester_src, "realtime") {
            p_map.insert("realtime".to_string(), serde_json::json!(rt));
        }
        if let Some(sev) = str_val(&pentester_src, "severity") {
            p_map.insert("severity".to_string(), serde_json::Value::String(sev));
        }

        // nested bruteforce
        if let Some(bf_src) = extract_section(&pentester_src, "bruteforce") {
            let mut bf_map = serde_json::Map::new();
            if let Some(bfe) = bool_val(&bf_src, "enabled") {
                bf_map.insert("enabled".to_string(), serde_json::json!(bfe));
            }
            if let Some(eps) = arr_val(&bf_src, "endpoints") {
                bf_map.insert("endpoints".to_string(), serde_json::json!(eps));
            }
            if let Some(wl) = str_val(&bf_src, "wordlist") {
                bf_map.insert("wordlist".to_string(), serde_json::Value::String(wl));
            }
            if let Some(drl) = bool_val(&bf_src, "detectRateLimit") {
                bf_map.insert("detectRateLimit".to_string(), serde_json::json!(drl));
            }
            if let Some(jw) = bool_val(&bf_src, "jwtWeak") {
                bf_map.insert("jwtWeak".to_string(), serde_json::json!(jw));
            }
            if let Some(se) = bool_val(&bf_src, "sessionExpiry") {
                bf_map.insert("sessionExpiry".to_string(), serde_json::json!(se));
            }
            if let Some(rl) = num_val(&bf_src, "rateLimit") {
                bf_map.insert("rateLimit".to_string(), serde_json::json!(rl as u32));
            }
            p_map.insert("bruteforce".to_string(), serde_json::Value::Object(bf_map));
        }

        // nested firewall
        if let Some(fw_src) = extract_section(&pentester_src, "firewall") {
            let mut fw_map = serde_json::Map::new();
            if let Some(fwe) = bool_val(&fw_src, "enabled") {
                fw_map.insert("enabled".to_string(), serde_json::json!(fwe));
            }
            if let Some(ps) = bool_val(&fw_src, "portScan") {
                fw_map.insert("portScan".to_string(), serde_json::json!(ps));
            }
            if let Some(cors) = bool_val(&fw_src, "cors") {
                fw_map.insert("cors".to_string(), serde_json::json!(cors));
            }
            if let Some(csp) = bool_val(&fw_src, "csp") {
                fw_map.insert("csp".to_string(), serde_json::json!(csp));
            }
            if let Some(ssl) = bool_val(&fw_src, "ssl") {
                fw_map.insert("ssl".to_string(), serde_json::json!(ssl));
            }
            if let Some(hsts) = bool_val(&fw_src, "hsts") {
                fw_map.insert("hsts".to_string(), serde_json::json!(hsts));
            }
            if let Some(xfo) = bool_val(&fw_src, "xFrameOptions") {
                fw_map.insert("xFrameOptions".to_string(), serde_json::json!(xfo));
            }
            p_map.insert("firewall".to_string(), serde_json::Value::Object(fw_map));
        }

        // nested static
        if let Some(st_src) = extract_section(&pentester_src, "static") {
            let mut st_map = serde_json::Map::new();
            if let Some(ow) = bool_val(&st_src, "owasp") {
                st_map.insert("owasp".to_string(), serde_json::json!(ow));
            }
            if let Some(sec) = bool_val(&st_src, "secrets") {
                st_map.insert("secrets".to_string(), serde_json::json!(sec));
            }
            if let Some(dep) = bool_val(&st_src, "dependencies") {
                st_map.insert("dependencies".to_string(), serde_json::json!(dep));
            }
            if let Some(inj) = bool_val(&st_src, "injections") {
                st_map.insert("injections".to_string(), serde_json::json!(inj));
            }
            if let Some(h) = bool_val(&st_src, "headers") {
                st_map.insert("headers".to_string(), serde_json::json!(h));
            }
            p_map.insert("static".to_string(), serde_json::Value::Object(st_map));
        }

        // nested active
        if let Some(ac_src) = extract_section(&pentester_src, "active") {
            let mut ac_map = serde_json::Map::new();
            if let Some(sqli) = bool_val(&ac_src, "sqlInjection") {
                ac_map.insert("sqlInjection".to_string(), serde_json::json!(sqli));
            }
            if let Some(xss) = bool_val(&ac_src, "xssReflected") {
                ac_map.insert("xssReflected".to_string(), serde_json::json!(xss));
            }
            if let Some(idor) = bool_val(&ac_src, "idor") {
                ac_map.insert("idor".to_string(), serde_json::json!(idor));
            }
            if let Some(dt) = bool_val(&ac_src, "directoryTraversal") {
                ac_map.insert("directoryTraversal".to_string(), serde_json::json!(dt));
            }
            if let Some(hr) = bool_val(&ac_src, "hiddenRoutes") {
                ac_map.insert("hiddenRoutes".to_string(), serde_json::json!(hr));
            }
            p_map.insert("active".to_string(), serde_json::Value::Object(ac_map));
        }

        // nested network
        if let Some(nw_src) = extract_section(&pentester_src, "network") {
            let mut nw_map = serde_json::Map::new();
            if let Some(to) = num_val(&nw_src, "timeout") {
                nw_map.insert("timeout".to_string(), serde_json::json!(to as u32));
            }
            if let Some(fr) = bool_val(&nw_src, "followRedirects") {
                nw_map.insert("followRedirects".to_string(), serde_json::json!(fr));
            }
            if let Some(rl) = num_val(&nw_src, "rateLimit") {
                nw_map.insert("rateLimit".to_string(), serde_json::json!(rl as u32));
            }
            p_map.insert("network".to_string(), serde_json::Value::Object(nw_map));
        }

        map.insert("pentester".to_string(), serde_json::Value::Object(p_map));
    }

    // remoteShell
    if let Some(rs_src) = extract_section(&src, "remoteShell") {
        let mut rs_map = serde_json::Map::new();
        if let Some(p) = num_val(&rs_src, "port") {
            rs_map.insert("port".to_string(), serde_json::json!(p as u16));
        }
        if let Some(sec) = str_val(&rs_src, "secret") {
            rs_map.insert("secret".to_string(), serde_json::Value::String(sec));
        }

        // nested https
        if let Some(https_src) = extract_section(&rs_src, "https") {
            let mut h_map = serde_json::Map::new();
            if let Some(c) = str_val(&https_src, "cert") {
                h_map.insert("cert".to_string(), serde_json::Value::String(c));
            }
            if let Some(k) = str_val(&https_src, "key") {
                h_map.insert("key".to_string(), serde_json::Value::String(k));
            }
            rs_map.insert("https".to_string(), serde_json::Value::Object(h_map));
        }

        // nested rateLimit
        if let Some(rl_src) = extract_section(&rs_src, "rateLimit") {
            let mut rl_map = serde_json::Map::new();
            if let Some(rpm) = num_val(&rl_src, "requestsPerMinute") {
                rl_map.insert(
                    "requestsPerMinute".to_string(),
                    serde_json::json!(rpm as u32),
                );
            }
            if let Some(b) = num_val(&rl_src, "burst") {
                rl_map.insert("burst".to_string(), serde_json::json!(b as u32));
            }
            rs_map.insert("rateLimit".to_string(), serde_json::Value::Object(rl_map));
        }

        // nested cors
        if let Some(cors_src) = extract_section(&rs_src, "cors") {
            let mut c_map = serde_json::Map::new();
            if let Some(origins) = arr_val(&cors_src, "allowedOrigins") {
                c_map.insert("allowedOrigins".to_string(), serde_json::json!(origins));
            }
            rs_map.insert("cors".to_string(), serde_json::Value::Object(c_map));
        }

        map.insert("remoteShell".to_string(), serde_json::Value::Object(rs_map));
    }

    if let Some(persona) = str_val(&src, "persona") {
        map.insert("persona".to_string(), serde_json::Value::String(persona));
    }
    if let Some(skills) = str_val(&src, "skills") {
        map.insert("skills".to_string(), serde_json::Value::String(skills));
    }

    Some(serde_json::Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_find_project_root_default() {
        let root = find_project_root(None);
        assert!(root.exists());
    }

    #[test]
    fn test_find_project_root_nested() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();

        // Create indicator in parent dir
        fs::create_dir(dir.path().join(".uneven")).unwrap();

        let root = find_project_root(Some(nested.to_str().unwrap()));
        let canonical_root = fs::canonicalize(dir.path()).unwrap();
        assert_eq!(root, canonical_root);
    }

    #[test]
    fn test_parse_config_ts_minimal() {
        let dir = tempdir().unwrap();
        let ts_content = r#"
            export default {
                brain: {
                    provider: "openai",
                    model: "gpt-4",
                    temperature: 0.7,
                },
                knowledge: {
                    dirs: ["./docs", "./wiki"]
                }
            }
        "#;
        fs::write(dir.path().join("uneven.config.ts"), ts_content).unwrap();

        let val = parse_config_ts(dir.path()).unwrap();

        let brain = val.get("brain").unwrap();
        assert_eq!(brain.get("provider").unwrap().as_str().unwrap(), "openai");
        assert_eq!(brain.get("model").unwrap().as_str().unwrap(), "gpt-4");
        assert_eq!(brain.get("temperature").unwrap().as_f64().unwrap(), 0.7);

        let knowledge = val.get("knowledge").unwrap();
        let dirs = knowledge.get("dirs").unwrap().as_array().unwrap();
        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0].as_str().unwrap(), "./docs");
        assert_eq!(dirs[1].as_str().unwrap(), "./wiki");
    }

    #[test]
    fn test_parse_config_ts_brains_array() {
        let dir = tempdir().unwrap();
        let ts_content = r#"
            export default {
                brains: [
                    {
                        id: "deepseek",
                        provider: "openai",
                        model: "deepseek-v4-flash",
                        temperature: 0.5,
                        maxTokens: 4096
                    },
                    {
                        id: "gemini",
                        provider: "gemini",
                        model: "gemini-3.1-pro",
                        temperature: 0.2
                    }
                ],
                routing: {
                    ask: "deepseek",
                    chat: "gemini"
                }
            }
        "#;
        fs::write(dir.path().join("uneven.config.ts"), ts_content).unwrap();

        let val = parse_config_ts(dir.path()).unwrap();

        let brains = val.get("brains").unwrap().as_array().unwrap();
        assert_eq!(brains.len(), 2);

        let b0 = &brains[0];
        assert_eq!(b0.get("id").unwrap().as_str().unwrap(), "deepseek");
        assert_eq!(b0.get("provider").unwrap().as_str().unwrap(), "openai");
        assert_eq!(
            b0.get("model").unwrap().as_str().unwrap(),
            "deepseek-v4-flash"
        );
        assert_eq!(b0.get("temperature").unwrap().as_f64().unwrap(), 0.5);
        assert_eq!(b0.get("maxTokens").unwrap().as_u64().unwrap(), 4096);

        let b1 = &brains[1];
        assert_eq!(b1.get("id").unwrap().as_str().unwrap(), "gemini");
        assert_eq!(b1.get("provider").unwrap().as_str().unwrap(), "gemini");
        assert_eq!(b1.get("model").unwrap().as_str().unwrap(), "gemini-3.1-pro");
        assert_eq!(b1.get("temperature").unwrap().as_f64().unwrap(), 0.2);

        let routing = val.get("routing").unwrap().as_object().unwrap();
        assert_eq!(routing.get("ask").unwrap().as_str().unwrap(), "deepseek");
        assert_eq!(routing.get("chat").unwrap().as_str().unwrap(), "gemini");
    }

    #[test]
    fn test_sync_config_from_ts() {
        let dir = tempdir().unwrap();
        let ts_content = r#"
            export default {
                brain: {
                    provider: "openai",
                    model: "gpt-4",
                    maxTokens: 1000
                },
                security: {
                    blockedColumns: ["pass", "salt"]
                }
            }
        "#;
        fs::write(dir.path().join("uneven.config.ts"), ts_content).unwrap();

        sync_config_from_ts(dir.path()).unwrap();

        let config_json_path =
            crate::infrastructure::utils::paths::project_cache_dir(dir.path()).join("config.json");
        assert!(config_json_path.exists());

        let loaded = load_config(dir.path()).unwrap();
        assert_eq!(loaded.brain.provider, "openai");
        assert_eq!(loaded.brain.model, "gpt-4");
        assert_eq!(loaded.brain.max_tokens, 1000);
        assert_eq!(loaded.security.blocked_columns, vec!["pass", "salt"]);
    }

    #[test]
    fn test_parse_config_ts_persona_and_reasoning_effort() {
        let dir = tempdir().unwrap();
        let ts_content = r#"
            export default {
                persona: "Custom Assistant Persona",
                brain: {
                    provider: "gemini",
                    model: "gemini-2.5-flash",
                    reasoningEffort: "medium"
                }
            }
        "#;
        fs::write(dir.path().join("uneven.config.ts"), ts_content).unwrap();

        let val = parse_config_ts(dir.path()).unwrap();
        assert_eq!(
            val.get("persona").unwrap().as_str().unwrap(),
            "Custom Assistant Persona"
        );

        let brain = val.get("brain").unwrap();
        assert_eq!(
            brain.get("reasoningEffort").unwrap().as_str().unwrap(),
            "medium"
        );

        let loaded = load_config(dir.path()).unwrap();
        assert_eq!(loaded.persona.as_deref(), Some("Custom Assistant Persona"));
        assert_eq!(loaded.brain.reasoning_effort.as_deref(), Some("medium"));
    }
}
