# uneven agent

![license](https://img.shields.io/badge/license-BSL%201.1-blue)

>
> Runs on your machine. An autonomous senior AI software engineering agent that indexes your codebase, plans and audits code changes with deep self-critique, watches running terminals in real time, **autonomously fixes errors**, scans for **malicious code**, performs **security testing**, and manages your project via a **conversational shell** or native developer interface.
>
> Use your own cloud API keys or run 100% offline.

---

## Features

- **⬡ Uneven Pods (Dedicated Architectural Clusters)** — Unlike ephemeral multi-agents that resolve small ad-hoc subtasks, Pods are persistent, dedicated clusters assigned to an entire software layer (e.g. *Database & Schema*, *Backend API & Logic*, *Frontend UI & Client*, *DevOps*). Pods execute large macro-missions powered by the reflective 3-turn `think` loop under strict domain-isolation laws (`get_pod_context`), preventing layer drift. Completed missions emit a structured `PodHandoffReport` with exported public contracts and sister-layer requirements, allowing the Master Agent to orchestrate and chain orders across clusters automatically without requiring manual pod switching.
- **🧠 Reflection & Self-Critique Engine with Rubber Duck Verification (`think`)** — Deep reasoning loop enhanced with intermediate Rubber Duck reflections (draft assumption unpacking and pre-flight execution dry-run simulation) coupled with an 18-point architectural audit, guaranteeing logic fidelity (anti-drift), cyclomatic complexity limits, documentation integrity, and host resource safety.
- **🎯 Adaptive Cognitive Sizing & Reactive Plan Execution** — Automatically scales output strategy based on task complexity: conceptual queries output clean markdown explanations (Case A); atomic single-file edits generate immediate patches with confirmation requests (Case B); complex multi-file architectural tasks present a blueprint first to save token budget without premature diff truncation (Case C); approving a proposal with natural confirmations (*"sim"*, *"aplique"*, *"proceed"*) or `/go` dedicates 100% token budget to generate and apply complete file patches to disk (Case D).
- **🛠️ Dynamic Operational Skills (`[USE_SKILL: name]`)** — Skills such as `explain` and `docs` are operational engineering standards invoked autonomously by the model without rigid personas, clashing biases, or dead weight in system prompts.
- **🛡️ Modernized Resource Guardian & Hardware Calibration** — Continuous monitoring across per-core CPU utilization, dynamic RAM thresholds, and integrated GPU (iGPU) UMA shared memory pressure in `uneven-core`, coupled with zero-copy `mmap` and layer-by-layer weight streaming for local models.
- **🛡️ Single-Run Command Guard & Anti-Freeze Protection** — Global asynchronous synchronization lock (`acquire_heavy_command_guard()`) ensuring that heavy compilation, testing, or build steps (`cargo check`, `cargo build`, `npm run build`, `pytest`) never execute concurrently across Pods, preventing host CPU freeze.
- **⚡ Cognitive Harness Over Model Size** — By wrapping the model with strict domain laws, self-critique audits, real-time `PROJECT_FILES_MAP` context, and inter-cluster handoffs, Uneven makes *any* model (from local 1B–3B offline GGUFs running on CPU to frontier cloud models) execute with the discipline and accuracy of a senior software engineer.
- **🌐 Keyless Token Economy & Prompt Guard** — Converts natural language prompts to concise English via DuckDuckGo before LLM submission, slashing 30–50% input tokens and boosting code generation fidelity. Imposes a 2,000-character safety guard with real-time UI counters to prevent overflow and encourage workspace file attachments (`📎`).
- **🔄 Autonomous Context Compaction & Quota Telemetry** — Integrates autonomous compaction in `uneven-core` that condenses multi-turn conversation logs into high-fidelity context summaries when token saturation is reached, preventing context truncation while maximizing cognitive budget. Provides real-time sliding-window quota tracking (5-hour turn, rolling 7 days, and monthly calendar quota) with zero filesystem leakage.
- **🗣️ Native-Language Response Synthesis** — Human-facing explanations, proposals, and summaries are generated natively in the developer's primary language (e.g. Portuguese), while diff hunks, functions, and code syntax strictly remain in clean English. Zero external translation calls on output avoids character limits and diff corruption.
- **🤖 Hierarchical Multi-Agent Supervision** — Prime Supervisors break down complex engineering tasks into sequential, atomic subtasks executed by specialized worker agents with live step-by-step auditing.
- **🖥️ Dual Interface Architecture & Integrated Terminal** — Engineered with a modular workspace design: work seamlessly from a high-speed command-line interface (`uneven`) with full autonomous agent capabilities, or launch the native visual Desktop IDE Cockpit (`uneven-ui` v0.2.0) built with Dioxus 0.7. Launch directly from terminal using `une .` or `uneven .`, manage multiple integrated terminal tabs concurrently, expand terminal to full height (`⤢`), edit code in-place, and track real-time sliding-window quota telemetry.
- **📝 Declarative Markdown Protocols** — Fully transparent action blocks (```` ```shell-action ````, ``` ```` sql ````) and standard unified diff patches without fragile raw JSON envelopes, making all proposed changes human-auditable and reliable across AI models.
- **🛡️ DevSecOps & Anti-False Positive Pentesting (`pentest`)** — Automated security audit pipeline with test/fixture heuristic awareness, multi-level semantic LLM review to eliminate false alarms, cryptographic scope signing, and CycloneDX 1.4 SBOM generation.
- **📊 Blind Autonomous Business Intelligence (`analyze`)** — Generate interactive queries and dashboards in a single prompt with strict column redaction, Catfive sample data masking, and mandatory user permission prior to any SQL execution.
- **⚡ High-Efficiency Intelligence & Postcard Persistence** — Reduces token consumption by executing heavy tasks locally, using AI only for final reasoning and code synthesis, backed by ultra-compact binary persistence powered by `postcard`.
- **📶 Remote Intelligence Hub** — Securely integrate with external tools (such as IDE extensions and remote frontends) via a Remote Shell API, supporting stateful in-process conversational chat sessions and stateless command execution.
- **🛡️ Enterprise Brand-Neutrality** — All internal system instructions are brand-neutral and customizable, allowing easy integration into white-labeled developer copilots.
- **🔍 Keyless Web Search & Sitemap Caching** — Enables the agent to query DuckDuckGo and Wikipedia on-demand for coding documentation or API help, while automatically crawling and caching domain sitemaps in the background.
- **⚡ Native Performance** — Built in 100% Rust and optimized for your hardware during installation, ensuring ultra-fast and private execution.

---

## Free vs Pro

| Feature | Free | Pro |
|---|:---:|:---:|
| Conversational shell | ✅ | ✅ |
| `ask` — query your codebase | ✅ | ✅ |
| `think` — deep reasoning & 18-point audit | ✅ | ✅ |
| `pods` — Connected Workstreams Pipeline | ✅ | ✅ |
| `multi-agent` — Hierarchical Prime & Subagents | ✅ | ✅ |
| Native Desktop Cockpit (`uneven-ui`) | ✅ (v0.2.0) | ✅ (v0.2.0) |
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

> Free users can use their own API keys, run Pods, and interact via Shell and Desktop IDE without message limits. Security Auditing (`pentest`) and Data Analysis (`analyze`) are exclusive to Pro/Team tiers.

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

### Local brain (100% Offline — no API, no cloud)

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

# 5. Deep reasoning & 18-point self-critique before coding
uneven think "Refactor authentication layer to JWT"

# 6. Apply file changes or create code directly
uneven askf "Create user model with password hashing"

# 7. Start the autonomous agent (watch + auto-fix) [Pro]
uneven start

# 8. Scan for malware and supply chain attacks
uneven scan

# 9. Run a code review [Pro]
uneven review

# 10. Analyze data & Dashboards [Pro]
uneven analyze --db postgresql://localhost/mydb
# Generate a complete sales dashboard for the last month in a single prompt.
# Uneven processes the data locally and delivers the ready-to-use insight.

# 11. Run security analysis [Pro]
uneven pentest

# 12. Run CI pipeline [Pro]
uneven ci

# 13. Launch the visual Desktop IDE Cockpit
cargo run -p uneven-ui
```

---

## Setup & Configuration

`uneven init` walks you through selecting a provider and configuring your environment:

- **Local Brain (Offline)** — Runs local models natively on your machine. No internet required after setup.
- **Cloud providers** — Use your existing accounts (OpenAI, Claude, Gemini, DeepSeek).

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
    "provider": "local",          // "local" (offline embedded) | "ollama" | "gemini" | "openai" | "claude" | "deepseek"
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

### Reasoning, Code Generation & Editing

```bash
uneven think "refactor auth layer"       # Multi-turn 18-point self-critique & declarative patch plan
uneven askf "create a login endpoint"    # AI creates/edits files (with automatic rollback & backup)
```

> **Hierarchy of Mutation & Adaptive Sizing:** Uneven enforces cognitive sizing before mutation:
> - **Case A (Inquiry / Q&A):** Clean markdown technical explanations with no file mutations.
> - **Case B (Micro / Atomic):** Immediate patch generation with concise search/replace blocks for single-file fixes.
> - **Case C (Medium / Complex Multi-File):** Comprehensive architectural blueprint and component map first, preserving output tokens without premature truncated diffs.
> - **Case D (Approved Execution / Autonomous):** Responding naturally with *"sim"*, *"aplique"*, or running with `/go` allocates 100% of the token budget to concrete diff patches and automatically applies them to disk.

### Uneven Pods (Dedicated Architectural Clusters & Connected Workstreams)

```bash
# Launch the Native Desktop Cockpit to access the visual Pods cluster
cargo run -p uneven-ui

# Or dispatch macro-missions directly from the interactive shell
uneven
> dispatch backend "Implement JWT authentication middleware and login route"
```

Uneven introduces **Pods**: persistent, dedicated engineering clusters assigned to an entire architectural layer of your application (`Database ──► Backend ──► Frontend ──► DevOps`):

- **Dedicated Clusters vs. Ephemeral Multi-Agents**:
  - **Multi-Agents (`multi_agents`)**: Temporary, ephemeral micro-workers spawned on-the-fly to execute small, ad-hoc tasks (e.g. searching a file, generating an isolated test case) and discarded immediately after.
  - **Uneven Pods (`pods`)**: Persistent, domain-bound engineering clusters that own an entire vertical slice of the stack. A Pod maintains persistent disk-backed conversational history (`.uneven/history/{session_id}_{pod_id}.json`), scoped project context, and specialized Prime leadership for executing large-scale, multi-file macro-missions.

- **Cognitive Harness Over Model Size**:
  - Pods do not require massive 70B+ cloud models to deliver senior engineering output. Every Pod runs inside the **Cognitive Harness**:
    1. **Strict Layer Isolation Laws (`laws::get_pod_context`)**: Primes have full read visibility over `PROJECT_FILES_MAP` to inspect cross-module imports and interfaces, but are system-forbidden from modifying files outside their assigned layer.
    2. **3-Turn Reflective Reasoning Loop (`think`)**: Every mission undergoes a mandatory `Draft Plan` ➔ `18-Point Architectural Self-Audit` (Anti-Drift, Cyclomatic complexity ≤ 10, Interface integrity, Backward compatibility) ➔ `Executive Synthesis`.
    3. **Small-Model Empowerment**: By combining boundary enforcement, topologic codebase maps, and iterative self-critique, even lightweight 1B–3B offline GGUFs running on CPU operate with the discipline and precision of a senior software engineer.

- **Inter-Cluster Handoff Protocol (`PodHandoffReport`)**:
  - When a Pod completes a mission, it automatically produces a structured handoff report containing:
    - **Technical Mission Summary**: High-level overview of accomplishments.
    - **Exported Public Contracts**: Schemas, API endpoints, types, DTOs, and event signatures created for consumption by other layers.
    - **Sister-Layer Advisory Notes**: Concrete integration guidelines and requirements for sibling clusters (e.g. *Database Pod* exports migration schemas and pool requirements for the *Backend Pod*; *Backend Pod* exports REST/gRPC endpoints for the *Frontend Pod*).
    - **Modified Artifacts**: Explicit list of altered files within domain boundaries.

- **Master Agent Orchestration & Autonomous Dispatch**:
  - The **Master Agent** serves as Chief Architect, dispatching missions to specific clusters via `dispatch_pod_mission(pod_id, mission)` or natural language delegation.
  - Upon mission completion, the Master automatically consumes the Pod's `PodHandoffReport::to_executive_summary()`, reviewing contracts and optionally chaining instructions to the next Pod in the pipeline without requiring manual developer intervention.

- **Desktop Cockpit Focus Flow & Minimalist UI (`uneven-ui`)**:
  - **Horizontal Pod Cards**: Monochromatic cards displaying live status (`idle`, `running`, `completed`), task counts, and assigned Prime titles.
  - **Auto-Hide on Focus**: Clicking a Pod card opens its dedicated conversation stream and automatically hides the Pod cards row, maximizing screen real estate and preventing UI distraction.
  - **Instant Exit (`✕`)**: A sleek `✕` button inside the cockpit input agent pill returns the developer to the global Master Agent chat at any time.
  - **Pure OLED Aesthetic**: Designed for true black `#0a0a0a` panels, `#121212` matte surfaces, and subtle `#262626` borders for zero eye strain during long engineering sprints.

- **Single-Run Command Guard & Anti-Freeze Protection**:
  - An asynchronous concurrency lock (`acquire_heavy_command_guard()`) guarantees that heavy compilation or testing commands (e.g. `cargo check`, `cargo build`, `npm run build`, `pytest`) are strictly serialized across all Pods and the Master, ensuring the host machine never freezes.


### Keyless Token Economy & Translation Engine

Uneven integrates an automated, keyless input translation pipeline powered by DuckDuckGo:

- **Pre-Flight Input Translation**: Natural language prompts (e.g. in Portuguese) are translated into concise technical English before submission to the LLM, reducing input token consumption by 30–50% and enhancing code generation accuracy.
- **Smart Paragraph & Code Chunking**: Prompts up to 2,000 characters are automatically split into safe sub-chunks (≤ 900 characters) preserving markdown code fences (```` ```...``` ````) untouched.
- **2,000-Character Prompt Guard**: A live character counter in the UI (`char_count / 2000`) and an overflow banner prevent oversized payloads, encouraging developers to attach large workspace files directly via the `📎` button instead of pasting code dumps.
- **Bilingual Output Synthesis**: The model formulates human-facing explanations and summaries in the developer's native tongue (e.g. Portuguese), while all diffs, functions, and variable names remain strictly in English. No external translation API is used on output, eliminating character truncation and diff syntax corruption.
- **Quick Toggle**: Enable or disable automatic translation instantly using the `[ 🌐 EN Tokens: On/Off ]` toolbar button.

### Native Desktop IDE (`uneven-ui`)

Uneven includes a lightning-fast native desktop IDE built with Dioxus:

- **CLI Launcher (`une .` / `uneven .`)**: Launch the desktop IDE straight from your terminal positioned in the current directory (`une .`), a target path (`une /path/to/project`), or standalone (`uneven ide`). Running from your home directory (`~`) opens cleanly without auto-binding a project.
- **Multi-Tab Integrated Terminal**: Native project terminal supporting multiple concurrent shell tabs. Create tabs with `+`, switch between tabs, terminate sessions with per-tab `✕`, and expand the terminal to full cockpit height (`⤢` / `⤡`).
- **Freedesktop & App Store Integration**: Properly packaged as **Uneven IDE** across Linux desktop launchers, Ubuntu App Center, GNOME Software, macOS, and Windows.
- **Engineering Cockpit**: Live layout featuring the interactive `FileTree`, the `PodFlow` workstream pipeline, real-time `ThinkStepper` indicator, and dedicated chat streams.
- **Surgical Patch Viewer**: Side-by-side or unified diff viewer powered by `similar` with multi-file tabs, live hunk inspection, and single-click `[ Apply All Changes ]`.
- **Dynamic Model Selector**: Switch on-the-fly between local offline models (Gemma 3, Qwen 2.5, DeepSeek-R1, Llama 3.2, Phi-4 Mini with auto-download) and cloud providers (Gemini 2.5, Claude 3.7/Sonnet, GPT-4o, DeepSeek V3/R1).
- **Secure Key Manager**: Dedicated Settings page to securely configure and persist API keys by provider into `.env`.
- **Prompt Guard & Token Toolbar**: Real-time character counter (`120 / 2000`), overflow alerts, and instant toggles for `[ 🤖 Multi-Agent ]` and `[ 🌐 EN Tokens ]`.
- **Session History**: Dedicated timeline viewer for past engineering sessions, audits, and applied patches.

### Watch & Review

```bash
uneven start                   # Start full agent (watch + auto-fix) [Pro]
uneven watch "npm run dev"     # Watch terminal — log errors only (Free) / auto-fix (Pro)
uneven watch "cmd1" "cmd2"     # Watch multiple processes simultaneously
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


## Production & Monitoring Mode

```json
{
  "watch": {
    "terminal": "node dist/server.js",
    "autoFix": false,
    "confirmBeforeFix": true
  },
  "pentester": {
    "enabled": true,
    "mode": "static"
  }
}
```

When `autoFix` is `false`, Uneven Agent operates strictly in monitoring mode — it captures and correlates errors but never modifies code without explicit approval.

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
