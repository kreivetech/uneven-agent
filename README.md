# uneven agent

![license](https://img.shields.io/badge/license-BSL%201.1-blue)

>
> Runs on your machine. Meets **Snatchy**, your autonomous senior engineer. She indexes your codebase, watches running terminals in real time, **autonomously fixes errors**, scans for **malicious code**, performs **security testing**, and manages your project via a **conversational shell**.
>
> Use your own API key or run 100% offline.

---

## Features

- **⚡ High-Efficiency Intelligence** — Reduces token consumption by executing heavy tasks locally, using AI only for final reasoning.
- **🔄 Three-Cache Auto-Recovery** — Automatically retrieves missing files requested by the LLM during conversation while avoiding infinite loops and redundant I/O via a three-layered caching architecture.
- **📊 Autonomous Business Intelligence** — Generate interactive queries and dashboards in a single prompt with stateful conversational refinement and dynamic 3-layer data protection.
- **📶 Remote Intelligence Hub** — Securely integrate with external tools (such as IDE extensions) via a Remote Shell API, supporting stateful in-process conversational chat sessions and stateless command execution.
- **🛡️ Enterprise Brand-Neutrality** — All internal system instructions are brand-neutral, allowing easy integration into custom white-labeled developer copilots.
- **🔍 Keyless Web Search & Sitemap Caching** — Enables the agent to query DuckDuckGo and Wikipedia on-demand for coding documentation or API help, while automatically crawling and caching domain sitemaps in the background to build a local map of documentation URLs.
- **⚡ Native Performance** — Optimized for your hardware during installation, ensuring fast and private execution.

---

## Free vs Pro

| Feature | Free | Pro |
|---|:---:|:---:|
| Conversational shell | ✅ | ✅ |
| `ask` — query your codebase | ✅ | ✅ |
| `index` — index knowledge base | ✅ | ✅ |
| `explain` — explain any file | ✅ | ✅ |
| `docs` — generate documentation | ✅ | ✅ |
| `scan` — malware & supply chain | ✅ | ✅ |
| `log` / `undo` / `info` | ✅ | ✅ |
| `watch` — terminal error watcher | ✅ | ✅ |
| `watch` — auto-fix errors | — | ✅ |
| `askf` — AI writes/edits files | ✅ | ✅ |
| `review` — AI code review | — | ✅ |
| `pentest` — security testing | — | ✅ |
| `analyze` — data analyst | — | ✅ |
| `ci` — CI/CD pipeline | — | ✅ |

> Free users can use their own API keys and interact via Shell without message limits. Security Auditing (`pentest`) and Data Analysis (`analyze`) are exclusive to Pro/Team tiers.

---

## Requirements

Requirements vary depending on the **brain provider** you choose. The standalone binary runs natively with **zero runtime dependencies** (no Node.js required).

### API providers (OpenAI, Claude, Gemini)

| Item | Minimum | Notes |
|---|---|---|
| RAM | 256 MB | Only embeddings model loaded (~300 MB peak) |
| Disk | 200 MB | Embeddings model only |
| OS | Linux, macOS, Windows 11 | |

### Ollama (local via Ollama daemon)

| Item | Minimum | Notes |
|---|---|---|
| RAM | 512 MB (Uneven Agent) | Ollama manages model memory separately |
| Disk | 200 MB + model size | Ollama stores models in its own directory |
| OS | Linux, macOS, Windows 11 | |

### Local brain (Snatchy — no API, no cloud)

| Item | Minimum | Recommended |
|---|---|---|
| RAM | 4 GB | 8 GB+ |
| Disk | 200 MB (embeddings) | 2 GB+ (local model) |
| OS | Linux, macOS, Windows 11 | Linux / macOS |

## Installation

```bash
curl -fsSL https://uneven.dev/install.sh | bash
```

---

## Quick Start

```bash
# 1. Initialize — choose brain, download models, auto-detect GPU
uneven init

# 2. Index your knowledge base
uneven index

# 3. Open the conversational shell (natural language interface)
uneven

# 4. Ask questions about your code
uneven ask "What does the authenticateUser function do?"

# 5. Start the autonomous agent (watch + auto-fix) [Pro]
uneven start

# 6. Scan for malware and supply chain attacks
uneven scan

# 7. Run a code review [Pro]
uneven review

# 8. Analyze data & Dashboards [Pro]
uneven analyze --db postgresql://localhost/mydb
# Generate a complete sales dashboard for the last month in a single prompt.
# Uneven processes the data locally and delivers the ready-to-use insight.

# 9. Run security analysis [Pro]
uneven pentest

# 10. Run CI pipeline [Pro]
uneven ci
```

---

## Setup & Configuration

`uneven init` walks you through selecting a provider and configuring your environment:

- **Snatchy (Local)** — Runs everything on your machine. No internet required after setup.
- **Cloud providers** — Use your existing accounts (OpenAI, Claude, Gemini).

---

## Configuration

Uneven Agent runs **out-of-the-box with zero configuration files** required in your workspace root. 

When you first launch `uneven`, a default configuration is generated and stored globally in the project's cache directory: `~/.uneven/bin/cache/<project-hash>/config.json`.

You can configure Uneven in three ways:

1. **Interactive Slash Commands** — Modify settings directly inside the conversational shell:
   - `/models` — Choose or configure AI providers and models.
   - `/analyze` — Set up database connection URLs, allowed tables, and data masking filters.
   - `/fix` — Configure watch terminal commands, auto-fix preferences, and confirmations.
   - `/urls` — Add documentation URLs for web loading/scraping.
   - `/skills` — Set path to a custom directory for workspace-level skills/rules.
   - `/person` — Configure a custom agent description or modular persona (e.g. administrative assistant).
   - `/max_tokens` — Set the maximum number of output tokens (default: 2048).
   - `/effort` — Configure reasoning effort (e.g. low, medium, high, none, or custom tokens limit).
   - `/agents` — Toggle multi-agent task execution mode (runs complex tasks across isolated subagents).
   - `/export` — Save persistent API keys globally.

2. **Direct Cache Editing** — Modify the JSON configuration file directly at:
   `~/.uneven/bin/cache/<project-hash>/config.json`

3. **Shell Environment Variables** — Export API keys in your terminal session or a local `.env` file:
   - `export GOOGLE_API_KEY=AI...` (Gemini)
   - `export OPENAI_API_KEY=sk-...` (OpenAI)
   - `export ANTHROPIC_API_KEY=sk-ant-...` (Claude)
   - `export DEEPSEEK_API_KEY=sk-...` (DeepSeek)

### Configuration Schema Reference

If you choose to edit the `config.json` file directly, here is the supported schema:

```json
{
  "brain": {
    "provider": "local",          // "local" (Snatchy) | "ollama" | "gemini" | "openai" | "claude" | "deepseek"
    "model": "llama-3.2-1b-q4",   // set automatically or via /models
    "apiKey": null,
    "temperature": 0.3,
    "maxTokens": 2048,
    "reasoningEffort": "medium",  // "low" | "medium" | "high" | "none" or custom tokens limit
    "local": {
      "threads": 4,
      "gpuLayers": 35             // configured automatically based on your VRAM
    }
  },
  "knowledge": {
    "dirs": ["./src"],
    "files": ["./README.md"],
    "db": {
      "url": "postgresql://...", // PostgreSQL, MySQL, SQLite, MongoDB
      "allowedTables": ["products", "faq"]
    },
    "urls": ["https://docs.example.com"]
  },
  "watch": {
    "terminal": "npm run dev",
    "autoFix": true,
    "confirmBeforeFix": false
  },
  "pentester": {
    "enabled": false,
    "mode": "static",             // "static" | "active"
    "target": "http://localhost:3000"
  },
  "notifications": {
    "webhookUrl": "https://hooks.slack.com/...",
    "minSeverity": "error"
  },
  "remoteShell": {
    "port": 4242,
    "secret": "your-master-key"
  },
  "log": {
    "path": "./.uneven/log.md"
  },
  "skills": "./custom-skills",
  "persona": "You are a professional administrative assistant...",
  "multiAgents": false            // Toggle multi-agent task execution mode (runs complex tasks across isolated subagents)
}
```

In API mode Uneven Agent stays well under 512 MB — only the local embeddings model is loaded for semantic search.

---

## CLI Commands

### Shell & Knowledge

```bash
uneven                         # Open conversational shell (natural language interface)
uneven init                    # Initialize project (interactive setup + model download)
uneven index                   # Index knowledge base (codebase, docs, DBs, URLs)
uneven ask "query"             # Single-turn question about your codebase
uneven explain <file>          # Explain a file in plain English
uneven docs <file>             # Generate Markdown documentation for a file
uneven log                     # View Uneven activity history
uneven undo                    # Undo last applied fix
uneven restore                 # Restore file from backup
uneven reset                   # Clear cache and re-index
uneven info                    # Show system info (GPU, version, model)
uneven uninstall               # Uninstall Uneven Agent and clean up all global files
```

### Watch & Fix

```bash
uneven start                   # Start full agent (watch + auto-fix) [Pro]
uneven watch "npm run dev"     # Watch terminal — log errors only (Free) / auto-fix (Pro)
uneven watch "cmd1" "cmd2"     # Watch multiple processes simultaneously
uneven askf "create a login endpoint"   # AI writes/edits files for the task (Free)
uneven review                  # AI code review of latest commit [Pro]
uneven review --staged         # Review staged changes
uneven review --from <ref>     # Review from a git ref
```

### Malware Scanner

```bash
uneven scan                  # Scan project files and dependencies
uneven scan --report         # Generate HTML + Markdown report
uneven scan --json           # JSON output for CI integration
```

Detects:
- **Reverse shells** — `/dev/tcp`, `nc -e`, `bash -i >&`
- **Obfuscation** — `eval(atob())`, `Buffer.from(..., 'base64')`, `String.fromCharCode` arrays
- **Crypto mining** — stratum+tcp URLs, xmrig/cryptonight references
- **Supply chain attacks** — network downloads in `postinstall` scripts
- **Credential theft** — SSH keys, `/etc/passwd`, `~/.aws/credentials` access
- **Persistence** — crontab writes, systemd units, shell profile modification
- **Typosquatting** — package names within edit-distance 2 of popular packages
- **Data exfiltration** — credential harvesting + external network upload patterns

### Data Analyst [Pro]

```bash
uneven analyze --db postgresql://localhost/mydb
uneven analyze --db sqlite://./data.db
uneven analyze --db mongodb://localhost/mydb
uneven analyze --db mysql://user:pass@localhost/mydb --package-exe
```

Interactive workflow:
1. Connects and introspects real schema (read-only)
2. Shows security policy (blocked columns + tables)
3. Describe what you want in plain English
4. Uneven Agent generates SQL — you `[A]pprove`, `[E]dit`, or `[S]kip`
5. Query executes — sensitive data auto-redacted in results
6. Export to Excel and/or HTML dashboard

**3-layer data protection:**

| Layer | What it does |
|---|---|
| **1. Schema Filter** | Automatically removes sensitive tables and columns from the schema before the AI analyzes it. |
| **2. Semantic SQL Audit** | Actively blocks suspicious queries or attempts to access unauthorized data patterns. |
| **3. Live Result Redaction** | Dynamically masks sensitive values (Passwords, JWTs, AWS keys) in real-time before they reach your screen. |

### Pentester [Pro]

```bash
uneven pentest                         # Static analysis
uneven pentest --mode active           # Active testing (requires scope declaration)
uneven pentest --declare-scope         # Declare authorization scope
uneven pentest --report html           # Generate HTML report
uneven pentest --report both           # HTML + Markdown
```

Active mode requires explicit authorization:

```
? Who authorized this test? John Doe
? Target(s): 192.168.1.0/24, dev.myapp.com
? Duration (hours): 8
? Type I AUTHORIZE to confirm: I AUTHORIZE
✓ Scope declared. Expires in 8h. Integrity-signed.
```

- Only in-scope IPs/hostnames can be tested
- Public internet IPs blocked unless explicitly in scope
- DoS tools, mass scans, and destructive flags blocked at the engine level

### CI/CD [Pro]

```bash
uneven ci                        # Full pipeline
uneven ci --skip-security        # Skip malware scan step
uneven ci --skip-tests           # Skip test suite step
uneven ci --strict-security      # Fail on medium findings (default: high+)
uneven ci --github               # Write GitHub Actions step summary
uneven ci --output ./ci-out.json # Custom output path
```

Pipeline steps: type check → malware scan → test suite. Exit code 0 = pass.

---

## Supported Languages

| Language | Error Parsing | Auto-Fix |
|---|:---:|:---:|
| TypeScript / JavaScript | ✅ | ✅ |
| Python | ✅ | ✅ |
| Rust | ✅ | ✅ |
| Go | ✅ | ✅ |
| Java | ✅ | ✅ |
| PHP | ✅ | ✅ |
| Ruby | ✅ | ✅ |
| COBOL | ✅ | ✅ |
| Assembly (x86/ARM) | ✅ | ✅ |

## Knowledge Sources

| Source | Details |
|---|---|
| **Local files** | `.ts .js .py .rs .go .java .php .rb .md .txt .json .yaml .toml` |
| **Spreadsheets** | Excel (.xlsx, .xls) and CSV |
| **Documents** | PDF (.pdf) and Word (.docx) |
| **Databases** | PostgreSQL, MySQL, SQLite, MongoDB |
| **URLs** | Any web page or documentation site |

## Brain Providers

| Provider | Config | Requires |
|---|---|---|
| **Local (default)** | `provider: 'local'` | Nothing — model downloads via `init` |
| **OpenAI** | `provider: 'openai'` | `OPENAI_API_KEY` env var |
| **Anthropic Claude** | `provider: 'claude'` | `ANTHROPIC_API_KEY` env var |
| **Google Gemini** | `provider: 'gemini'` | `GOOGLE_API_KEY` env var |
| **Ollama** | `provider: 'ollama'` | Ollama running locally |

---

## Security & Privacy

Privacy is not a feature — it's the foundation.

✅ **Absolute sovereignty** — 100% local execution available  
✅ **No telemetry** — zero usage tracking or analytics  
✅ **Forensic Audit Logs** — Detailed activity history with secure timestamps and auditable code diffs  
✅ **Fallback Local Backups** — Native copies of files are saved under `.uneven/obsolete` (preserving relative paths and appending timestamp suffixes) before any destructive overwrite or delete by `askf` occurs, ensuring manual recovery is always possible.  
✅ **3-layer data protection** — Sensitive schema and results are dynamically filtered and redacted  
✅ **Scope-enforced pentester** — Active mode requires explicit authorization  
✅ **Ethical safety guard** — Built-in protection against generating malicious payloads  

---


## Production Mode

```typescript
const config: UnevenConfig = {
  watch: {
    terminal: 'node dist/server.js',
    autoFix: false,     // never auto-fix in production
    alertOnly: true,
  },
  pentester: {
    enabled: true,
    severity: 'low',
  },
}
```

In `alertOnly` mode, Uneven Agent is a monitoring agent — it detects issues but never modifies code.

---

## Log Format

All findings are written to `.uneven/log.md`:

```markdown
## [10:32:14] Error Detected

**File:** src/services/auth.service.ts
**Line:** 42 | **Column:** 18
**Message:** TypeError: Cannot read properties of undefined

## [10:32:15] Fix Applied

**File:** src/services/auth.service.ts

**Before:**
return user.id

**After:**
return user?.id ?? null

## [10:45:00] Security Finding

**Severity:** HIGH
**Category:** obfuscation
**File:** scripts/deploy.js
**Line:** 12

eval(atob('aGVsbG8gd29ybGQ='))
```

---

## Disclaimer

Uneven Agent uses artificial intelligence models that **may produce incorrect, incomplete, or misleading results**. AI-generated outputs — including code fixes, security findings, SQL queries, and diagnoses — must be reviewed by a qualified professional before being applied to any production system.

Kreive Tech is **not liable** for:

- Damages caused by incorrect or incomplete AI-generated suggestions
- Data loss, system failures, or security incidents resulting from automated fixes
- Misuse of security testing features against systems without explicit authorization
- Any direct, indirect, incidental, or consequential damages arising from use of this software

**You are solely responsible** for validating AI outputs and ensuring compliance with applicable laws and regulations.

---

## License & Ownership

Uneven Agent is proprietary software. All intellectual property rights — including source code, design, and documentation — are owned exclusively by **Kreive Tech**.

**Business Source License 1.1 (BSL)**

✅ Free for: personal projects, learning, non-commercial use  
❌ Requires license for: commercial production use  
✅ Converts to Apache 2.0 on 2056-04-19  

Read the full [LICENSE](./LICENSE) file.

## Learning & Documentation

* Detailed architecture, internal subsystem workflows, and CLI command execution logic are documented in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
* Want to master Uneven Agent? Visit our official learning center for tutorials, guides, and best practices:

👉 **[uneven.dev/docs/learn](https://uneven.dev/docs/learn)**

---

## Support

- 📧 [Email support](mailto:contact@kreive.com.br)

---

**© 2026 Kreive Tech. All rights reserved.**

Uneven Agent is developed and owned by **Kreive Tech** — a software engineering consultancy in Brazil.

- 🌐 [uneven.dev](https://uneven.dev)
- 📧 [contact@kreive.com.br](mailto:contact@kreive.com.br)
- 🐙 [GitHub](https://github.com/kreivesler)

---

**Made with ❤️ for developers who want AI that respects their privacy.**
