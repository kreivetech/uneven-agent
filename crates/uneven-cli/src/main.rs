use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod errors;
mod ui;

#[derive(Parser)]
#[command(name = "uneven")]
#[command(about = "Autonomous local development agent.", long_about = None)]
#[command(version)]
struct Cli {
    /// Enable or disable auto-fix for the current session
    #[arg(long, value_name = "boolean")]
    autofix: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Set up Uneven in the current project
    Init,

    /// Show system diagnostics (version, GPU status, hardware acceleration)
    Info,

    /// Start the full Uneven agent (terminal watcher + security)
    Start,

    /// Watch processes for errors and fix them automatically (15-minute limit on Free tier)
    Watch {
        /// Commands to run and watch
        commands: Vec<String>,

        /// Suppress errors matching pattern
        #[arg(long, value_name = "pattern")]
        ignore: Vec<String>,

        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,
    },

    /// Index project files into Uneven's memory
    Index {
        /// Directory to index
        #[arg(short, long, value_name = "path")]
        dir: Option<String>,

        /// Force full re-index
        #[arg(long)]
        full: bool,

        /// Preview without indexing
        #[arg(long)]
        dry_run: bool,

        /// Skip large files
        #[arg(long)]
        skip_large: bool,

        /// Size limit in MB
        #[arg(long, value_name = "mb")]
        max_file_size: Option<f64>,

        /// Bypass confirmation prompt (non-interactive mode)
        #[arg(short, long)]
        yes: bool,
    },

    /// Interactive conversation with Uneven
    Chat {
        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,
    },

    /// Ask anything about your project or request a code task
    Ask {
        /// Question or task to investigate and plan
        question: String,

        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,
    },

    /// Generate API documentation
    Docs {
        /// Target file
        file: String,

        /// Specific symbol to document
        symbol: Option<String>,

        /// Format: md or jsdoc
        #[arg(long, default_value = "md")]
        format: String,

        /// Save output to this file
        #[arg(long, value_name = "path")]
        out: Option<String>,

        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,
    },

    /// Explain a source file
    Explain {
        /// Target file
        file: String,

        /// Specific section or focus area
        focus: Option<String>,

        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,

        /// Disable resource limits for large files
        #[arg(long)]
        no_guardian: bool,
    },

    /// Start the proactive file-watching service (independent of terminal)
    Autofix {
        /// Stop the proactive service
        #[arg(long)]
        off: bool,

        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,
    },

    /// Create or edit files based on a task (alias to ask)
    #[command(hide = true)]
    Askf {
        /// Task description
        task: String,

        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,
    },

    /// Deep reasoning and pre-flight architectural critique before proposing changes
    Think {
        /// Task or modification goal
        task: String,

        /// Target file paths to inspect
        #[arg(short, long, value_name = "path")]
        files: Vec<String>,

        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,
    },

    /// Database analysis and BI dashboard generation
    Analyze {
        /// Database URL
        #[arg(long, value_name = "url")]
        db: Option<String>,

        /// Non-interactive analysis request
        #[arg(long, value_name = "query")]
        prompt: Option<String>,

        /// Generate report (excel, html, both)
        #[arg(long, default_value = "html")]
        report: String,

        /// Dashboard theme (dark, light)
        #[arg(long, default_value = "dark")]
        theme: String,

        /// Bundle dashboards as portable .exe
        #[arg(long)]
        package_exe: bool,

        /// Clear analysis cache
        #[arg(long)]
        clear_cache: bool,

        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,
    },

    /// Scan source code for security vulnerabilities (2 static scans per 6h limit on Free tier, active requires Team)
    Pentest {
        /// Mode: static or active
        #[arg(short, long, default_value = "static")]
        mode: String,

        /// Generate report (md, html, both)
        #[arg(short, long, value_name = "format")]
        report: Option<String>,

        /// Declare targets before active testing
        #[arg(long)]
        declare_scope: bool,

        /// AI provider
        #[arg(long, value_name = "name")]
        provider: Option<String>,

        /// AI model name
        #[arg(long, value_name = "name")]
        model: Option<String>,
    },

    /// Scan project files for custom zero-day vulnerability flows (Team feature)
    Zeroday {
        /// Directory to scan
        #[arg(short, long)]
        dir: Option<String>,

        /// Generate report (md, html, both)
        #[arg(short, long, value_name = "format")]
        report: Option<String>,

        /// Run deep multi-turn autonomous taint tracing with Think reasoning
        #[arg(long)]
        deep: bool,
    },

    /// Detect malicious patterns and compromised npm packages (2 executions per 6h limit on Free tier)
    Scan {
        /// Directory to scan
        #[arg(short, long, default_value = "./")]
        dir: String,

        /// Generate HTML + Markdown report
        #[arg(short, long)]
        report: bool,

        /// Output findings as JSON
        #[arg(long)]
        json: bool,
    },

    /// Run project's tests
    Test {
        /// Run only tests related to this file
        #[arg(short, long, value_name = "path")]
        file: Option<String>,
    },

    /// Undo the last fix Uneven applied
    Undo {
        /// Undo a specific fix by hash
        #[arg(short, long, value_name = "hash")]
        commit: Option<String>,

        /// List recent Uneven fix commits
        #[arg(short, long)]
        list: bool,
    },

    /// Restore a file to its previous state
    Restore {
        /// File to restore
        #[arg(short, long, value_name = "path")]
        file: Option<String>,

        /// Restore a specific snapshot by ID
        #[arg(short, long, value_name = "id")]
        id: Option<String>,

        /// List available snapshots
        #[arg(short, long)]
        list: bool,
    },

    /// Review and apply pending fix suggestions
    Diff {
        /// Apply pending diff by ID prefix
        #[arg(long, value_name = "id")]
        apply: Option<String>,

        /// Reject pending diff by ID prefix
        #[arg(long, value_name = "id")]
        reject: Option<String>,

        /// Clear all resolved diffs
        #[arg(long)]
        clear: bool,
    },

    /// Run the full CI pipeline
    Ci {
        /// Directory to scan
        #[arg(short, long, default_value = "./")]
        dir: String,

        /// Skip security scan step
        #[arg(long)]
        skip_security: bool,

        /// Skip test suite step
        #[arg(long)]
        skip_tests: bool,

        /// Fail on medium severity findings
        #[arg(long)]
        strict_security: bool,

        /// Write JSON summary to this path
        #[arg(short, long, value_name = "path")]
        output: Option<String>,

        /// Emit GitHub Actions step summary
        #[arg(long)]
        github: bool,
    },

    /// Show activity log
    Log,

    /// Clear memory and start fresh
    Reset,

    /// Manage your Uneven Pro license
    License {
        #[command(subcommand)]
        subcmd: LicenseCommands,
    },

    /// Update Uneven AI executable to the latest stable release
    Update,

    /// Launch the Uneven visual desktop IDE cockpit
    #[command(alias = "gui", alias = "ui")]
    Ide {
        /// Optional directory or project path to open
        #[arg(value_name = "path")]
        path: Option<String>,
    },

    /// Show API usage costs and optimization stats
    Stats,

    /// Uninstall Uneven AI and clean up all global files/configurations
    Uninstall {
        /// Bypass confirmation prompt (non-interactive mode)
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum LicenseCommands {
    /// Activate a license key
    Activate {
        /// License key
        key: String,
    },
    /// Show current license status
    Status,
    /// Remove the license
    Deactivate,
}

fn is_known_subcommand(cmd: &str) -> bool {
    matches!(
        cmd,
        "init"
            | "info"
            | "start"
            | "watch"
            | "index"
            | "chat"
            | "ask"
            | "askf"
            | "think"
            | "docs"
            | "explain"
            | "autofix"
            | "analyze"
            | "pentest"
            | "zeroday"
            | "scan"
            | "test"
            | "undo"
            | "restore"
            | "diff"
            | "ci"
            | "log"
            | "reset"
            | "license"
            | "update"
            | "stats"
            | "uninstall"
            | "ide"
            | "ui"
            | "gui"
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    // Load global environment variables from ~/.uneven/bin/env
    let global_env_path = uneven_core::infrastructure::utils::paths::global_bin_dir().join("env");
    if global_env_path.exists()
        && let Ok(content) = std::fs::read_to_string(&global_env_path) {
            for line in content.lines() {
                let trimmed_line = line.trim();
                if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
                    continue;
                }
                if let Some((key, val)) = trimmed_line.split_once('=') {
                    let val_clean = val.trim().trim_matches(|c| c == '\'' || c == '"');
                    unsafe { std::env::set_var(key.trim(), val_clean) };
                }
            }
        }

    let args: Vec<String> = std::env::args().collect();
    let bin_name = args.first().map(|s| s.as_str()).unwrap_or("");
    let is_une_alias = bin_name.ends_with("/une")
        || bin_name == "une"
        || bin_name.ends_with("\\une.exe")
        || bin_name == "une.exe";

    // If invoked as `une` or `une <path>` (e.g. `une .` or `une ~`)
    if is_une_alias {
        let target_path = args.get(1).map(|s| s.as_str());
        return commands::ide::run(target_path).await;
    }

    if args.len() == 2 {
        if args[1] == "-h" || args[1] == "--help" {
            commands::help::run().await?;
            {
                use std::io::Write;
                let _ = std::io::stdout().flush();
                let _ = std::io::stderr().flush();
            }
            std::process::exit(0);
        }
        if args[1] == "-v" || args[1] == "--version" || args[1] == "-V" {
            println!("uneven {}", env!("CARGO_PKG_VERSION"));
            {
                use std::io::Write;
                let _ = std::io::stdout().flush();
                let _ = std::io::stderr().flush();
            }
            std::process::exit(0);
        }

        // If invoked as `uneven .` or `uneven <path>` where the argument is a directory/path
        let candidate = &args[1];
        let is_path_candidate = candidate == "."
            || candidate == ".."
            || candidate.starts_with("./")
            || candidate.starts_with("../")
            || candidate.starts_with('/')
            || candidate.starts_with('~')
            || (std::path::Path::new(candidate).exists() && std::path::Path::new(candidate).is_dir());

        if is_path_candidate && !candidate.starts_with('-') && !is_known_subcommand(candidate) {
            return commands::ide::run(Some(candidate.as_str())).await;
        }
    }

    // Automatically ensure workspace setup and embedding models are initialized
    let first_run_root = uneven_core::infrastructure::config::loader::find_project_root(None);
    let first_run_config_path =
        uneven_core::infrastructure::utils::paths::project_cache_dir(&first_run_root)
            .join("config.json");
    if !first_run_config_path.exists() {
        unsafe { std::env::set_var("UNEVEN_FIRST_RUN", "true") };
    }

    let _ = commands::init::ensure_workspace_initialized().await;

    let cli = Cli::parse();

    tokio::spawn(async {
        uneven_core::infrastructure::utils::telemetry::send_telemetry_ping().await;
    });

    if let Some(autofix_val) = cli.autofix {
        ui::print_info(&format!("Session auto-fix flag set to: {}", autofix_val));
    }

    let result = match cli.command {
        Some(Commands::Init) => commands::init::run().await,
        Some(Commands::Info) => commands::info::run().await,
        Some(Commands::Start) => commands::start::run().await,
        Some(Commands::Watch {
            commands,
            ignore,
            provider,
            model,
        }) => commands::watch::run(commands, ignore, provider, model).await,
        Some(Commands::Index {
            dir,
            full,
            dry_run,
            skip_large,
            max_file_size,
            yes,
        }) => commands::index::run(dir, full, dry_run, skip_large, max_file_size, yes).await,
        Some(Commands::Chat { provider, model }) => commands::chat::run(provider, model).await,
        Some(Commands::Ask {
            question,
            provider,
            model,
        }) => commands::ask::run(question, provider, model).await,
        Some(Commands::Docs {
            file,
            symbol,
            format,
            out,
            provider,
            model,
        }) => commands::docs::run(file, symbol, format, out, provider, model).await,
        Some(Commands::Explain {
            file,
            focus,
            provider,
            model,
            no_guardian,
        }) => commands::explain::run(file, focus, provider, model, no_guardian).await,
        Some(Commands::Autofix {
            off,
            provider,
            model,
        }) => commands::autofix::run(off, provider, model).await,
        Some(Commands::Askf {
            task,
            provider,
            model,
        }) => commands::ask::run(task, provider, model).await,
        Some(Commands::Think {
            task,
            files,
            provider,
            model,
        }) => {
            commands::think::run(task, files, provider, model).await.map(|_| ())
        }
        Some(Commands::Analyze {
            db,
            prompt,
            report,
            theme,
            package_exe,
            clear_cache,
            provider,
            model,
        }) => {
            commands::analyze::run(
                db,
                prompt,
                report,
                theme,
                package_exe,
                clear_cache,
                provider,
                model,
            )
            .await
        }
        Some(Commands::Pentest {
            mode,
            report,
            declare_scope,
            provider,
            model,
        }) => commands::pentest::run(mode, report, declare_scope, provider, model).await,
        Some(Commands::Zeroday { dir, report, deep }) => commands::zeroday::run(dir, report, deep).await,
        Some(Commands::Scan { dir, report, json }) => commands::scan::run(dir, report, json).await,
        Some(Commands::Test { file }) => commands::test::run(file).await,
        Some(Commands::Undo { commit, list }) => commands::undo::run(commit, list).await,
        Some(Commands::Restore { file, id, list }) => commands::restore::run(file, id, list).await,
        Some(Commands::Diff {
            apply,
            reject,
            clear,
        }) => commands::diff::run(apply, reject, clear).await,
        Some(Commands::Ci {
            dir,
            skip_security,
            skip_tests,
            strict_security,
            output,
            github,
        }) => {
            commands::ci::run(
                dir,
                skip_security,
                skip_tests,
                strict_security,
                output,
                github,
            )
            .await
        }
        Some(Commands::Log) => commands::log::run().await,
        Some(Commands::Reset) => commands::reset::run().await,
        Some(Commands::License { subcmd }) => match subcmd {
            LicenseCommands::Activate { key } => commands::license::run_activate(key).await,
            LicenseCommands::Status => commands::license::run_status().await,
            LicenseCommands::Deactivate => commands::license::run_deactivate().await,
        },
        Some(Commands::Update) => commands::update::run().await,
        Some(Commands::Ide { path }) => commands::ide::run(path.as_deref()).await,
        Some(Commands::Stats) => commands::stats::run().await,
        Some(Commands::Uninstall { yes }) => commands::uninstall::run(yes).await,
        None => commands::interactive::run().await,
    };

    if let Err(err) = result {
        errors::handle_error(err);
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let _ = std::io::stderr().flush();
        }
        std::process::exit(1);
    }

    {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
    }

    Ok(())
}
