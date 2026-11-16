use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use uneven_core::infrastructure::utils::paths::{binary_root_dir, global_bin_dir, global_uneven_dir};

pub const PROVIDER_KEY_VARS: &[(&str, &str, &str, &str)] = &[
    (
        "GOOGLE_API_KEY",
        "Google Gemini",
        "AIzaSy...",
        "Fast multimodal and code reasoning engine",
    ),
    (
        "ANTHROPIC_API_KEY",
        "Anthropic Claude",
        "sk-ant-...",
        "High-level architectural audit and synthesis engine",
    ),
    (
        "OPENAI_API_KEY",
        "OpenAI (GPT / o-series)",
        "sk-proj-...",
        "General reasoning and coding models",
    ),
    (
        "DEEPSEEK_API_KEY",
        "DeepSeek Cloud",
        "sk-...",
        "Deep mathematical reasoning and code analysis",
    ),
    (
        "GROQ_API_KEY",
        "Groq Cloud",
        "gsk_...",
        "Ultra-low latency Llama inference",
    ),
    (
        "MISTRAL_API_KEY",
        "Mistral AI",
        "secret-...",
        "Codestral and Mistral Large models",
    ),
    (
        "OLLAMA_API",
        "Ollama (Local)",
        "http://localhost:11434",
        "Self-hosted local offline model host",
    ),
];

pub const DATABASE_KEY_VARS: &[(&str, &str, &str, &str)] = &[
    (
        "DATABASE_URL",
        "Database Connection URI",
        "postgres://user:password@localhost:5432/dbname",
        "Connection string for PostgreSQL, MySQL, SQLite, or MariaDB",
    ),
    (
        "DB_API_KEY",
        "Database Access Token / API Key",
        "eyJh... or service token",
        "Authentication bearer key for Supabase, Neon, Turso, or PlanetScale",
    ),
];

/// Returns all candidate global `.env` file locations in prioritized order.
pub fn global_env_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // 1. Permanent global binary directory: ~/.uneven/bin/.env
    let bin_dir = global_bin_dir();
    candidates.push(bin_dir.join(".env"));

    // 2. Permanent global uneven directory: ~/.uneven/.env
    let uneven_dir = global_uneven_dir();
    if uneven_dir != bin_dir {
        candidates.push(uneven_dir.join(".env"));
    }

    // 3. Binary execution directory: e.g. target/debug/.env or /usr/local/bin/.env
    let exec_dir = binary_root_dir();
    if exec_dir != bin_dir && exec_dir != uneven_dir {
        candidates.push(exec_dir.join(".env"));
    }

    // 4. Current working directory .env (if present)
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_env = cwd.join(".env");
        if !candidates.contains(&cwd_env) {
            candidates.push(cwd_env);
        }
    }

    candidates
}

/// Returns the primary global `.env` file location inside Uneven's global directory.
#[allow(dead_code)]
pub fn global_env_file() -> PathBuf {
    let bin_env = global_bin_dir().join(".env");
    let uneven_env = global_uneven_dir().join(".env");
    if bin_env.exists() {
        return bin_env;
    }
    if uneven_env.exists() {
        return uneven_env;
    }
    let dir = global_bin_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join(".env")
}

/// Reads all credentials from all candidate global `.env` files, project `.env`, and active process environment.
pub fn load_all_env_keys(root: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();

    // 1. Read from global candidate .env files in reverse order so higher-priority candidates override
    for cand in global_env_candidates().iter().rev() {
        if cand.exists() && let Ok(content) = fs::read_to_string(cand) {
            parse_env_lines_into_map(&content, &mut map);
        }
    }

    // 2. Read from active project root .env (if present and valid)
    let project_env = root.join(".env");
    if project_env.exists() && let Ok(content) = fs::read_to_string(&project_env) {
        parse_env_lines_into_map(&content, &mut map);
    }

    // 3. Fallback to process environment variables
    for (var_name, _, _, _) in PROVIDER_KEY_VARS.iter().chain(DATABASE_KEY_VARS.iter()) {
        if !map.contains_key(*var_name)
            && let Ok(val) = std::env::var(var_name) {
                let trimmed = val.trim().to_string();
                if !trimmed.is_empty() {
                    map.insert(var_name.to_string(), trimmed);
                }
            }
    }

    map
}

/// Initializes process environment variables from global .env files upon app startup.
pub fn init_process_env() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let all_keys = load_all_env_keys(&cwd);
    for (k, v) in all_keys {
        if !v.trim().is_empty() && std::env::var(&k).is_err() {
            unsafe {
                std::env::set_var(&k, v.trim());
            }
        }
    }
}

fn parse_env_lines_into_map(content: &str, map: &mut HashMap<String, String>) {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let clean_k = k.trim().to_string();
            let clean_v = v.trim().trim_matches('"').trim_matches('\'').to_string();
            if !clean_v.is_empty() {
                map.insert(clean_k, clean_v);
            }
        }
    }
}

/// Saves credentials securely into both global `.env` files and immediately applies to the process.
pub fn save_all_env_keys(keys: &HashMap<String, String>) -> io::Result<()> {
    let target_paths = [
        global_bin_dir().join(".env"),
        global_uneven_dir().join(".env"),
        binary_root_dir().join(".env"),
    ];

    for (k, v) in keys {
        let trimmed_v = v.trim();
        // Apply immediately to current running process
        unsafe {
            if trimmed_v.is_empty() {
                std::env::remove_var(k);
            } else {
                std::env::set_var(k, trimmed_v);
            }
        }
    }

    for env_path in &target_paths {
        if let Some(parent) = env_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let mut existing_lines: Vec<String> = if env_path.exists() {
            fs::read_to_string(env_path)
                .unwrap_or_default()
                .lines()
                .map(|l| l.to_string())
                .collect()
        } else {
            vec![
                "# Uneven Global Environment & Credentials".to_string(),
                "# Automatically synchronized across Uneven runtime directories".to_string(),
            ]
        };

        for (k, v) in keys {
            let trimmed_v = v.trim();
            let prefix = format!("{}=", k);
            let mut found = false;
            for line in existing_lines.iter_mut() {
                if line.trim_start().starts_with(&prefix) {
                    if trimmed_v.is_empty() {
                        *line = String::new();
                    } else {
                        *line = format!("{}={}", k, trimmed_v);
                    }
                    found = true;
                    break;
                }
            }
            if !found && !trimmed_v.is_empty() {
                existing_lines.push(format!("{}={}", k, trimmed_v));
            }
        }

        existing_lines.retain(|l| !l.trim().is_empty());
        let _ = fs::write(env_path, existing_lines.join("\n") + "\n");
    }

    Ok(())
}

/// Backward compatibility helper
#[allow(dead_code)]
pub fn load_provider_keys(root: &Path) -> HashMap<String, String> {
    load_all_env_keys(root)
}

/// Backward compatibility helper
#[allow(dead_code)]
pub fn save_provider_keys(_root: &Path, keys: &HashMap<String, String>) -> io::Result<()> {
    save_all_env_keys(keys)
}
