use anyhow::Result;
use crate::ui;

pub async fn run() -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let a = |s: &str| ui::accent(s);
    let b = |s: &str| ui::bright(s);
    let d = |s: &str| ui::dim(s);
    let g = |s: &str| ui::green(s);
    let o = |s: &str| ui::rust(s);
    let t = |s: &str| ui::truecolor(s, 181, 140, 255); // #b58cff - lavender/purple

    let w = |cmd: &str, desc: &str| {
        format!("  {}{}", a(&ui::pad(cmd, 34)), d(desc))
    };

    let version_tag = format!("v{}", version);
    println!("  {} {}", b("◈  U N E V E N"), d(&version_tag));
    println!("  {}", d("Autonomous local development agent"));
    println!();

    println!("{}", b("  What is Uneven?"));
    println!();
    println!("{}", d("  Uneven sits inside your project and works as an invisible co-developer."));
    println!("{}", d("  It reads your code, watches your terminal, catches errors as they happen,"));
    println!("{}", d("  suggests (and optionally applies) fixes, answers questions about your"));
    println!("{}", d("  project in plain English, scans for security issues, and helps you explore"));
    println!("{}", d("  your database — all running locally on your machine, no data sent out."));
    println!();

    println!("{}", b("  Quick start — three commands to get going:"));
    println!();
    println!("  {} {}   {}", g("1."), a("uneven info"), d("→ check system environment and AI brain status"));
    println!("  {} {}  {}", g("2."), a("uneven index"), d("→ Uneven reads and learns your codebase"));
    println!("  {} {}  {}", g("3."), a("uneven watch"), d("→ Uneven runs your dev server and fixes errors live"));
    println!();

    print_section("Setup & Utilities (Free)");
    println!("{}", w("init", "Optionally configure AI brain and workspace"));
    println!("{}", w("info", "Show system diagnostics and GPU status"));
    println!("{}", w("stats", "Show API usage costs and optimization stats"));
    println!("{}", w("update", "Update Uneven AI to the latest stable release"));
    println!("{}", w("license activate <key>", "Activate a Pro/Team license key"));
    println!("{}", w("license status", "Show current license details and active tier"));
    println!("{}", w("license deactivate", "Remove active license from this machine"));

    print_section("Free Plan Commands");
    println!("{}", w("watch [commands...]", "Watch dev server processes & auto-fix errors (15-min limit on Free)"));
    println!("{}", d("    --ignore <pattern>             Suppress errors matching glob pattern"));
    println!("{}", d("    --provider, --model <name>     Override AI configuration for this session"));
    println!("{}", w("index", "Build/learn project codebase, database, and docs"));
    println!("{}", d("    -d, --dir <path>               Target directory to index (default: project root)"));
    println!("{}", d("    --dry-run                      Preview files to be indexed without committing"));
    println!("{}", d("    --full                         Force full re-index bypassing cached snapshots"));
    println!("{}", d("    --skip-large                   Automatically skip files exceeding limit"));
    println!("{}", d("    --max-file-size <mb>           Specify maximum indexable file size (default: 2MB)"));
    println!("{}", w("chat", "Start a direct, raw chat session with the AI brain"));
    println!("{}", w("ask \"<task or question>\"", "Deep reasoning, code investigation, and pre-flight implementation planning"));
    println!("{}", w("docs <file> [symbol]", "Generate Markdown or JSDoc documentation for code symbols"));
    println!("{}", d("    --format md|jsdoc              Choose output format (default: md)"));
    println!("{}", d("    --out <path>                   Save generated documentation to specific file"));
    println!("{}", w("scan", "Detect secrets, malicious patterns & compromised npm packages (2/6h limit on Free)"));
    println!("{}", d("    -d, --dir <path>               Target directory to scan (default: ./src)"));
    println!("{}", d("    -r, --report                   Generate interactive HTML and Markdown report"));
    println!("{}", d("    --json                         Output scan findings in raw JSON format"));
    println!("{}", w("test", "Detect test runner & run tests (Jest, Vitest, Pytest, etc.)"));
    println!("{}", d("    -f, --file <path>              Run only test files associated with this file"));
    println!("{}", w("diff", "Review and apply/reject pending AI fix suggestions"));
    println!("{}", d("    --apply, --reject <id>         Process a specific suggestion by its ID prefix"));
    println!("{}", d("    --clear                        Clear all currently resolved suggestions"));
    println!("{}", w("undo", "Revert the last AI-applied code modification in git history"));
    println!("{}", d("    -c, --commit <hash>            Revert a specific AI fix commit by its hash"));
    println!("{}", d("    -l, --list                     Display a history of recent Uneven fix commits"));
    println!("{}", w("restore", "Restore a single file to a previous point in time"));
    println!("{}", d("    -f, --file <path>              Path to the file to restore"));
    println!("{}", d("    -i, --id <id>                  Restore file to a specific snapshot ID"));
    println!("{}", d("    -l, --list                     List all available snapshot states for this project"));
    println!("{}", w("log", "Show the global activity and intervention log"));
    println!("{}", w("reset", "Clear Uneven local HNSW memory index and start fresh"));

    print_section(&format!("Pro Plan Commands  {}", o("◈ Pro")));
    println!("{}", w("autofix", "Proactive background file-save analyzer & auto-fixer"));
    println!("{}", d("    --off                          Gracefully stop the proactive background service"));
    println!("{}", d("    --provider, --model <name>     Override brain model for background processes"));
    println!("{}", w("explain <file> [focus]", "Explain a source file or answer file-specific questions"));
    println!("{}", d("    --no-guardian                  Disable file safety/token limits for massive files"));
    println!("{}", w("analyze", "Natural-language BI: query database and build live dashboards"));
    println!("{}", d("    [Mode 1: Interactive UI Shell] (Runs when no --prompt is specified)"));
    println!("{}", d("    --db <url>                     Target connection URL for DB introspection"));
    println!("{}", d("    [Mode 2: Automated Analysis]   (Runs when --prompt is specified)"));
    println!("{}", d("    --prompt \"<query>\"             Provide analysis directives, chart preferences & colors"));
    println!("{}", d("    --report excel|html|both       Export report/dashboard formats (default: html)"));
    println!("{}", d("    --theme light|dark             Set dashboard interface theme (default: dark)"));
    println!("{}", d("    --package-exe                  Bundle generated HTML dashboard as portable Windows exe"));
    println!("{}", d("    --clear-cache                  Purge introspection and analysis schema cache"));
    println!("{}", w("ci", "Execute full CI pipeline (typecheck → security scan → tests)"));
    println!("{}", d("    --skip-security, --skip-tests  Exclude security scan or unit test steps"));
    println!("{}", d("    --strict-security              Fail CI if any medium severity leak is detected"));
    println!("{}", d("    -o, --output <path>            Write structured JSON build summary to file"));
    println!("{}", d("    --github                       Enable GitHub Actions inline annotation format"));

    print_section(&format!("Team Plan Commands  {}", t("◈ Team")));
    println!("{}", w("pentest", "Perform automated static/active pentesting over code & db (2 static scans/6h limit on Free, active requires Team)"));
    println!("{}", d("    -m static|active               Static vulnerability scanning or active testing (default: static)"));
    println!("{}", d("    -r md|html|both                Choose penetration report formats"));
    println!("{}", d("    --declare-scope                Safely restrict/declare target APIs before testing"));
    println!("{}", w("zeroday", "Scan project files for custom zero-day vulnerability flows"));
    println!("{}", d("    -d, --dir <path>               Target directory to scan"));
    println!("{}", d("    -r, --report <format>          Generate report (md, html, both)"));
    println!("{}", w("remote-shell", "Expose secure local HTTP/HTTPS endpoint for bot integrations"));
    println!("{}", d("    (Requires remoteShell settings in your configuration)"));
    println!("{}", w("webhook notifications", "Integrate real-time Slack, Discord or generic webhooks"));
    println!("{}", d("    (Triggered on error detections. Requires config.notifications in watch mode)"));

    print_section("Global Options");
    println!("{}", w("-v, --version", "Show version"));
    println!("{}", w("-h, --help", "Show this help"));
    println!();

    println!("{} {} {}", d("  Run"), a("uneven <command> --help"), d("for full options of any command."));
    println!("{} {}", d("  Docs: "), a("https://uneven.dev/docs"));
    println!();
    println!("{} {} {}", d("  ◈  Tip: run "), a("uneven"), d("without arguments to enter the unified conversational shell"));
    println!("{}", d("     (ask questions, surgically write code, or execute commands in the same place)"));
    println!();

    Ok(())
}

fn print_section(title: &str) {
    let d = |s: &str| ui::dim(s);
    let a = |s: &str| ui::accent(s);
    let clean_title = console::strip_ansi_codes(title);
    let len = clean_title.chars().count();
    let pad_len = 44_usize.saturating_sub(len);
    println!();
    println!("{} {}{}", d("  ──"), a(title), d(&"─".repeat(pad_len)));
}
