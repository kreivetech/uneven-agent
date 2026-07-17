use crate::ui;
use anyhow::{anyhow, Result};
use futures::StreamExt;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use uneven_core::domain::consts::models::{
    EMBEDDING_FILES, LOCAL_MODEL_CATALOG, SUPPORTED_LOCAL_MODELS,
};
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::utils::hardware_detector::HardwareDetector;

const TYPES_D_TS: &str = include_str!("../../../../types/index.d.ts");

pub async fn run() -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    ui::box_header(version);

    // 1. Hardware scan
    let detector = HardwareDetector::new();
    let hardware = detector.detect_system()?;

    let recommended_threads = hardware.cpu_cores.clamp(2, 8);
    let recommended_gpu_layers = if hardware.gpu.detected {
        hardware.gpu.gpu_layers
    } else {
        0
    };

    let recommended_model = SUPPORTED_LOCAL_MODELS
        .iter()
        .find(|m| m.id == hardware.recommended_model_id);

    if hardware.is_weak_system {
        ui::warn("Low hardware detected!");
        ui::info(&format!(
            "CPU Cores: {} | RAM: {}GB",
            hardware.cpu_cores,
            (hardware.ram_total_mb as f64 / 1024.0).round()
        ));
        let model_label = recommended_model.map(|m| m.label).unwrap_or("Llama 3.2 1B");
        ui::info(&format!(
            "Recommended: {} (Safe mode)",
            ui::bright(model_label)
        ));
        ui::blank();
    } else {
        ui::ok("Hardware diagnostic: System meets recommended standards.");
        let gpu_info = if hardware.gpu.detected {
            hardware.gpu.name.as_deref().unwrap_or("Detected")
        } else {
            "None detected"
        };
        ui::info(&format!(
            "CPU Cores: {} | RAM: {}GB | GPU: {}",
            hardware.cpu_cores,
            (hardware.ram_total_mb as f64 / 1024.0).round(),
            gpu_info
        ));
        let model_label = recommended_model.map(|m| m.label).unwrap_or("Llama 3.2 1B");
        ui::info(&format!(
            "Best Match:  {} (Optimal for your hardware)",
            ui::bright(model_label)
        ));
        ui::blank();
    }

    println!(
        "{}",
        ui::crystal("  Welcome! Let's set up Uneven for your project.")
    );
    println!("{}", ui::dim("  This will only take a moment."));
    println!();

    // 2. Prepares workspace
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message(ui::dim("Preparing workspace..."));
    let global_models_dir = uneven_core::infrastructure::utils::paths::global_models_dir();
    fs::create_dir_all(global_models_dir.join("embeddings"))?;
    fs::create_dir_all(".uneven/logs")?;
    fs::create_dir_all(".uneven/obsolete")?;
    fs::create_dir_all(".uneven/reports")?;
    fs::write(".uneven/types.d.ts", TYPES_D_TS)?;
    spinner.finish_and_clear();
    println!(
        "{} {} {}",
        ui::green("  ✓  "),
        ui::crystal("Workspace ready"),
        ui::dim("  (.uneven/)")
    );
    println!();

    // 3. Selection of brain type
    ui::section("Which brain type should power Uneven?");
    println!(
        "{}  {}         {}",
        ui::dim("  1"),
        ui::accent("  Snatchy"),
        ui::dim("— Native engine, local & offline, runs on your hardware")
    );
    println!(
        "{}  {} {}",
        ui::dim("  2"),
        ui::accent("  Cloud Connection"),
        ui::dim("— Remote models or Ollama, requires API/network")
    );
    ui::blank();

    print!("{}", ui::accent("  Selection [1-2] (default: 1): "));
    io::stdout().flush()?;
    let mut brain_choice = String::new();
    io::stdin().read_line(&mut brain_choice)?;
    let brain_choice = brain_choice.trim();

    let provider;
    let model_id;

    if brain_choice == "2" {
        ui::section("Select your Provider");
        let remote_providers = [
            ("1", "gemini", "Gemini", "Google AI — Best value"),
            ("2", "openai", "GPT", "OpenAI — Industry standard"),
            ("3", "claude", "Claude", "Anthropic — High intelligence"),
            ("4", "deepseek", "DeepSeek", "Advanced efficiency"),
            ("5", "mistral", "Mistral", "European open-weight"),
            ("6", "groq", "Groq", "Ultra-fast inference"),
            ("7", "ollama", "Ollama", "Local API server"),
        ];
        for p in &remote_providers {
            println!(
                "{}  {}  {}",
                ui::dim(&format!("  {}", p.0)),
                ui::accent(&format!("{:width$}", p.2, width = 12)),
                ui::dim(&format!("— {}", p.3))
            );
        }
        ui::blank();

        print!("{}", ui::accent("  Selection [1-7] (default: 1): "));
        io::stdout().flush()?;
        let mut prov_choice = String::new();
        io::stdin().read_line(&mut prov_choice)?;
        let prov_choice = prov_choice.trim();

        provider = match prov_choice {
            "2" => "openai",
            "3" => "claude",
            "4" => "deepseek",
            "5" => "mistral",
            "6" => "groq",
            "7" => "ollama",
            _ => "gemini",
        };

        model_id = match provider {
            "openai" => "gpt-5.4-mini".to_string(),
            "claude" => "claude-opus-4-8".to_string(),
            "deepseek" => "deepseek-v4-flash".to_string(),
            "mistral" => "mistral-large-latest".to_string(),
            "groq" => "llama-3.3-70b-versatile".to_string(),
            "ollama" => "llama3.2".to_string(),
            _ => "gemini-3.5-flash".to_string(),
        };
    } else {
        provider = "local";
        ui::blank();
        ui::section("Choose your local model");
        for (i, entry) in LOCAL_MODEL_CATALOG.iter().enumerate() {
            let recommended_tag = if entry.id == hardware.recommended_model_id {
                format!(" {}", ui::green("(Recommended)"))
            } else {
                "".to_string()
            };
            println!(
                "  {}  {}{}\n      RAM: {} | Storage: {}",
                ui::dim(&(i + 1).to_string()),
                ui::accent(entry.label),
                recommended_tag,
                ui::dim(entry.ram),
                ui::dim(entry.storage)
            );
        }
        ui::blank();

        print!(
            "{}",
            ui::accent(&format!(
                "  Selection [1-{}] (default recommended): ",
                LOCAL_MODEL_CATALOG.len()
            ))
        );
        io::stdout().flush()?;
        let mut model_choice = String::new();
        io::stdin().read_line(&mut model_choice)?;
        let model_choice = model_choice.trim();

        let index = model_choice.parse::<usize>().unwrap_or(0);
        let selected_entry = if index > 0 && index <= LOCAL_MODEL_CATALOG.len() {
            &LOCAL_MODEL_CATALOG[index - 1]
        } else {
            LOCAL_MODEL_CATALOG
                .iter()
                .find(|m| m.id == hardware.recommended_model_id)
                .or_else(|| {
                    SUPPORTED_LOCAL_MODELS
                        .iter()
                        .find(|m| m.id == hardware.recommended_model_id)
                })
                .unwrap_or(&LOCAL_MODEL_CATALOG[0])
        };

        model_id = selected_entry.id.to_string();
        ui::blank();
        ui::ok(&format!(
            "Model set to {}",
            ui::bright(selected_entry.label)
        ));
    }

    ui::blank();
    let provider_label = match provider {
        "local" => "Snatchy (Local Agent)",
        "ollama" => "Ollama (local server)",
        "claude" => "Claude — Anthropic",
        "openai" => "GPT — OpenAI",
        "gemini" => "Gemini — Google",
        "deepseek" => "DeepSeek",
        "mistral" => "Mistral AI",
        "groq" => "Groq (LPU)",
        _ => provider,
    };
    ui::ok(&format!("Brain set to {}", ui::bright(provider_label)));
    ui::ok(&format!("Model set to {}", ui::bright(&model_id)));

    // 4. Writes config to .uneven/config.json directly
    let root = loader::find_project_root(None);
    let mut config = loader::load_config(&root).unwrap_or_else(|_| {
        let default_dirs = vec!["./".to_string()];

        uneven_core::domain::UnevenConfig {
            project_root: root.to_string_lossy().to_string(),
            knowledge: Some(uneven_core::domain::KnowledgeConfig {
                dirs: Some(default_dirs.clone()),
                files: Some(vec![]),
                db: None,
                urls: Some(vec![]),
                external_project: None,
            }),
            watch: Some(uneven_core::domain::WatchConfig {
                terminal: Some("npm run dev".to_string()),
                terminals: None,
                ignore_patterns: None,
                auto_fix: Some(true),
                confirm_before_fix: Some(false),
                alert_only: Some(false),
                dirs: Some(default_dirs.clone()),
                only_use_strategies: Some(false),
                low_cpu_mode: Some(false),
            }),
            log: Some(uneven_core::domain::LogConfig {
                path: Some("./.uneven/log.md".to_string()),
                append: Some(true),
                include_timestamp: Some(true),
                include_diff: Some(true),
            }),
            proactive: Some(uneven_core::domain::ProactiveConfig {
                enabled: Some(true),
                auto_fix: Some(true),
                confirm_before_fix: Some(false),
                dirs: Some(default_dirs.clone()),
                ignore: Some(vec![]),
                vendor: Some(vec![]),
                only_use_strategies: Some(false),
                low_cpu_mode: Some(false),
            }),
            zeroday: Some(uneven_core::domain::ZeroDayConfig {
                enabled: Some(true),
                dirs: Some(default_dirs.clone()),
                low_cpu_mode: Some(false),
            }),
            ..Default::default()
        }
    });

    config.brain.provider = provider.to_string();
    config.brain.model = model_id.clone();

    if provider == "local" {
        config.brain.local = Some(uneven_core::domain::LocalBrainConfig {
            threads: Some(recommended_threads as u32),
            gpu_layers: Some(recommended_gpu_layers),
            unload_timeout: Some(10),
        });
    }

    let config_path =
        uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let config_json = serde_json::to_string_pretty(&config)?;
    fs::write(&config_path, config_json)?;
    ui::ok("Runtime config saved to cache");

    // 6. Update .gitignore
    let gitignore_path = Path::new(".gitignore");
    let mut gitignore_content = if gitignore_path.exists() {
        fs::read_to_string(gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut updated = false;
    if !gitignore_content.contains(".uneven") {
        if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
            gitignore_content.push('\n');
        }
        gitignore_content.push_str("\n# Uneven cache and logs\n.uneven/\n");
        updated = true;
    }

    if updated {
        fs::write(gitignore_path, gitignore_content)?;
        ui::ok(".gitignore updated");
    }

    // 7. Downloads embedding model files
    ui::section("Embedding model (all-MiniLM-L6-v2, 384-dim)");
    ui::info("Used for semantic search across your codebase — always required.");
    ui::blank();

    let global_models_dir = uneven_core::infrastructure::utils::paths::global_models_dir();
    for file_entry in EMBEDDING_FILES {
        let suffix = file_entry
            .dest
            .strip_prefix(".uneven/models/")
            .unwrap_or(file_entry.dest);
        let dest = global_models_dir.join(suffix);
        let label = dest.file_name().unwrap_or_default().to_string_lossy();
        if dest.exists() {
            ui::info(&format!("{} already present — skipping", label));
            continue;
        }
        download_file(file_entry.url, &dest, &label).await?;
    }

    // 8. Downloads local LLM model files if local provider selected
    if provider == "local" {
        let catalog_entry = SUPPORTED_LOCAL_MODELS
            .iter()
            .find(|m| m.id == model_id)
            .unwrap_or(&LOCAL_MODEL_CATALOG[0]);

        ui::section(&format!(
            "Snatchy Local Engine ({} — {})",
            catalog_entry.label, catalog_entry.storage
        ));
        ui::info("Runs 100% on your machine, totally offline.");
        ui::blank();

        for file_entry in catalog_entry.files {
            let suffix = file_entry
                .dest
                .strip_prefix(".uneven/models/")
                .unwrap_or(file_entry.dest);
            let dest = global_models_dir.join(suffix);
            let label = dest.file_name().unwrap_or_default().to_string_lossy();
            if dest.exists() {
                ui::info(&format!("{} already present — skipping", label));
                continue;
            }
            if let Err(err) = download_file(file_entry.url, &dest, &label).await {
                ui::blank();
                ui::warn(&format!(
                    "Could not download {} automatically: {}",
                    label, err
                ));
                ui::hint("If the model requires a HuggingFace token, set HF_TOKEN env var.");
                ui::blank();
            }
        }
    }

    // Webhook notifications sync/instruction
    if provider != "local" {
        ui::section("One more thing — provider setup");
        let instructions: &[(&str, &str)] = match provider {
            "ollama" => &[
                ("Install Ollama", "https://ollama.com"),
                ("Pull a model", "ollama pull llama3"),
                ("Start server", "ollama serve"),
            ],
            "claude" => &[("Set your API key", "export ANTHROPIC_API_KEY=sk-ant-...")],
            "openai" => &[("Set your API key", "export OPENAI_API_KEY=sk-...")],
            "gemini" => &[("Set your API key", "export GOOGLE_API_KEY=AI...")],
            "deepseek" => &[("Set your API key", "export DEEPSEEK_API_KEY=sk-...")],
            "mistral" => &[("Set your API key", "export MISTRAL_API_KEY=...")],
            "groq" => &[("Set your API key", "export GROQ_API_KEY=gsk_...")],
            _ => &[],
        };
        for (lbl, cmd) in instructions {
            println!(
                "  {}  {}",
                ui::dim(&format!("{:width$}", lbl, width = 20)),
                ui::accent(cmd)
            );
        }
        ui::blank();
        ui::info("Manage your Pro license:  uneven license activate <key>");
    }

    // Sync Agent Protocols
    let synced = uneven_core::infrastructure::security::agent_sync::sync_agent_protocols(
        &root.to_string_lossy(),
    );
    if !synced.is_empty() {
        ui::ok(&format!("AI Agent Protocols synced: {}", synced.join(", ")));
    }

    // Footer advice
    println!();
    println!("{}", ui::dim(&"─".repeat(52)));
    println!();
    println!(
        "{}",
        ui::bright("  ◈  You're all set! Here's what to do next:")
    );
    println!();
    ui::cmd_line("uneven index", "— index your codebase for semantic search");
    ui::cmd_line("uneven ask \"hello\"", "— test the brain");
    ui::cmd_line("uneven start", "— launch the full autonomous agent");
    ui::cmd_line("uneven pentest", "— run a security analysis");
    ui::blank();

    Ok(())
}

pub async fn download_file(url: &str, dest_path: &Path, label: &str) -> Result<()> {
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let client = reqwest::Client::new();
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to download {}: {}", label, e))?;

    if !res.status().is_success() {
        return Err(anyhow!(
            "Failed to download {}, status code: {}",
            label,
            res.status()
        ));
    }

    let total_size = res.content_length().unwrap_or(0);

    let pb = indicatif::ProgressBar::new(total_size);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template(&format!("  {} {{spinner}} [{{elapsed_precise}}] [{{wide_bar:.cyan/blue}}] {{bytes}}/{{total_bytes}} ({{eta}})", ui::accent("›")))?
            .progress_chars("##-")
    );
    pb.set_message(format!("Downloading {}", label));

    let mut file = fs::File::create(dest_path)?;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = match item {
            Ok(c) => c,
            Err(e) => {
                pb.finish_and_clear();
                return Err(anyhow!("Error while downloading chunk: {}", e));
            }
        };
        if let Err(e) = file.write_all(&chunk) {
            pb.finish_and_clear();
            return Err(e.into());
        }
        let new_pos = std::cmp::min(pb.position() + chunk.len() as u64, total_size);
        pb.set_position(new_pos);
    }

    pb.finish_and_clear();
    println!(
        "{} {} {}",
        ui::green("  ✓  "),
        ui::crystal("Downloaded"),
        ui::dim(label)
    );
    Ok(())
}

pub async fn ensure_workspace_initialized() -> Result<()> {
    let root = loader::find_project_root(None);
    let uneven_dir = root.join(".uneven");
    let global_models_dir = uneven_core::infrastructure::utils::paths::global_models_dir();

    // 1. Create global and local folders
    let _ = fs::create_dir_all(global_models_dir.join("embeddings"));
    let _ = fs::create_dir_all(uneven_dir.join("logs"));
    let _ = fs::create_dir_all(uneven_dir.join("obsolete"));
    let _ = fs::create_dir_all(uneven_dir.join("reports"));

    // 2. Write types.d.ts if missing
    let types_path = uneven_dir.join("types.d.ts");
    if !types_path.exists() {
        let _ = fs::write(&types_path, TYPES_D_TS);
    }

    // 3. Update .gitignore if needed
    let gitignore_path = root.join(".gitignore");
    if gitignore_path.exists() {
        if let Ok(content) = fs::read_to_string(&gitignore_path) {
            if !content.contains(".uneven/") {
                let mut new_content = content;
                if !new_content.ends_with('\n') {
                    new_content.push('\n');
                }
                new_content.push_str("\n# Uneven cache and logs\n.uneven/\n");
                let _ = fs::write(&gitignore_path, new_content);
            }
        }
    }

    // 4. Check and download embedding model files (BERT)
    let mut missing_embeddings = false;
    for file_entry in EMBEDDING_FILES {
        let suffix = file_entry
            .dest
            .strip_prefix(".uneven/models/")
            .unwrap_or(file_entry.dest);
        let dest = global_models_dir.join(suffix);
        if !dest.exists() {
            missing_embeddings = true;
            break;
        }
    }

    if missing_embeddings {
        println!(
            "\n  {} Initializing local embeddings engine (approx. 90MB)...",
            ui::accent("›")
        );
        for file_entry in EMBEDDING_FILES {
            let suffix = file_entry
                .dest
                .strip_prefix(".uneven/models/")
                .unwrap_or(file_entry.dest);
            let dest = global_models_dir.join(suffix);
            let label = dest.file_name().unwrap_or_default().to_string_lossy();
            if dest.exists() {
                continue;
            }
            download_file(file_entry.url, &dest, &label).await?;
        }
        println!();
    }

    // 5. Create default config.json if not present
    let config_path =
        uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
    if !config_path.exists() {
        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let default_config =
            loader::load_config(&root).unwrap_or_else(|_| uneven_core::UnevenConfig::default());
        if let Ok(config_json) = serde_json::to_string_pretty(&default_config) {
            let _ = fs::write(&config_path, config_json);
        }
    }

    Ok(())
}
