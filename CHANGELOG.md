# Changelog

All notable changes to Uneven Agent are documented in this file.

## [v2.4.0] — November 2026

- **Non-Blocking Security Scans & Batch LLM Review (`scan` / `pentest` / `zeroday`)** — Offloaded static analysis, supply-chain audits, `npm audit`, file counting, and report/SBOM generation off the Dioxus UI thread (`tokio::spawn` + `tokio::task::spawn_blocking`), resolved target paths against `project_root`, and batched `LLMSecurityReviewer` into a single Markdown review request with a ±5 line context window and dynamic model token/context limits.
- **Minimalist Think & Subagent HUD (`uneven-ui` v0.2.3)** — Redesigned `ThinkStepper` and `StatusBar` into compact, left-aligned 1-word phase pills (`Context`, `Drafting`, `Reasoning`, `Auditing`, `Verifying`, `Simulating`, `Synthesizing`) with a separate inline subagent badge (`Prime`, `Worker X/Y`, `Review X`).
- **Unified `AgentDirective` & `contextcraft` Reasoning Pipeline** — Added `domain::directives` (`AgentDirective`, `DIRECTIVES_PROTOCOL`, `parse_directives`, `strip_directives`, `resolve_builtin_skill`) with `catfive` validation; converted `think` to an internal `Vec<BrainTurn>` loop using `ContextOptimizer` (`BlockStrategy::UpdateInPlace` for `LAW` and `MAP`); enabled `optimize_context` in `shell`; and added `laws::get_context_in_root` decoupling workspace skills from the shell intent classifier.
- **2-Level Grouped `FilesMapIndex` & `[NEED_DIR]` Expansion** — Replaced flat full-repo file lists with a cached 2-level directory group index (`[id] prefix (N files)`) preserving `.gitignore` and `.env*` visibility, paginated `[NEED_DIR: id | page]` expansion, and surgical per-group refresh after `patchcraft` mutations.
- **Core Cleanup** — Removed the unused `ragcraft` dependency and legacy `chunker` / `retrieval` modules.

## [v2.3.1] — November 2026

- **Desktop App Store & Launcher Branding** — Standardized native desktop packaging for Ubuntu App Center, GNOME Software, and desktop launchers: the app now displays cleanly as **Uneven IDE** (previously defaulted to `UnevenUi`).
- **Multi-Tab Integrated Terminal in Uneven IDE** — Added full multi-tab terminal management to `uneven-ui`:
  - Open new concurrent terminal sessions with `+`.
  - Active tab tracking and effortless tab switching.
  - Per-tab kill/close (`✕`) buttons to terminate background tasks independently.
- **Full-Screen Terminal Expansion (`⤢` / `⤡`)** — Integrated expand/collapse toggle allowing the terminal to occupy 100% of the cockpit height for in-depth trace inspection, build logs, and multi-process monitoring.
- **CLI IDE Launchers (`une .` & `uneven .`)** — Introduced instant terminal invocation to launch Uneven IDE directly into the current directory (`une .` / `uneven .`) or specified target directory (`une <path>`), with home directory (`~`) protection preventing accidental root workspace bindings.
- **Desktop IDE Version Bump (`v0.2.0`)** — Bumped `uneven-ui` to `0.2.0` across manifests, desktop status bars, terminal banners, and documentation.

## [v2.3.0] — November 2026

- **Integrated Multiplatform Terminal in Desktop IDE** — Native project terminal with active process execution, clean ANSI color handling, quick tab termination (`✕`), and keyboard shortcuts: `Ctrl+C` interrupt for long-running processes, `Ctrl+Shift+C`/`Ctrl+Shift+V` clipboard management, and click-to-focus output.
- **Rubber Duck Reasoning Stages** — Integrated dual intermediate reflection stages into the `think` pipeline:
  - *Draft Decomposition* (`think_duck_draft`): Unpacks hidden assumptions, mechanics, and fragility prior to the 18-point Critic Audit.
  - *Pre-Flight Execution Simulation* (`think_duck_simulation`): Performs an end-to-end dry-run mental execution trace verifying imports, types, and reconciliation before final synthesis.
- **Modernized Resource Guardian** — Comprehensive hardware monitoring in `uneven-core` with per-core CPU load calculation, dynamic memory thresholds, and integrated GPU (iGPU) UMA shared memory pressure tracking.
- **Hardware-Calibrated Local Inference** — Zero-copy memory mapping (`mmap`) and layer-by-layer weight streaming enabled by default for local GGUF models, optimizing execution across integrated GPUs and constrained RAM environments.
- **Autonomous Context Compaction & Sliding-Window Quota Telemetry** — Autonomous multi-turn conversation condensation and real-time rolling quota tracking (5-hour, 7-day, and 30-day limits) without local filesystem leakage.
- **Visual Desktop Cockpit (`uneven-ui`)** — Modernized Dioxus 0.7 desktop IDE with multi-tab file editor, lazy-loaded tree explorer, in-place syntax highlighting, crystal branding icons, and blind SQL dashboard analytics.

## [v2.2.0] — November 2026

- **Adaptive Cognitive Sizing (Cases A–D)** — Automatic output scaling: conceptual Q&A generates pure markdown (Case A); atomic fixes generate immediate patches (Case B); multi-file tasks present architectural blueprints first without token burning (Case C); approved plans dedicate 100% token budget to full diff generation (Case D).
- **Reactive Plan Execution in Interactive Shell** — Approving a proposal with natural language confirmations (*"sim"*, *"aplique"*, *"proceed"*) or `/go` automatically executes the plan, generating concrete patches and applying them to disk without errors.
- **Dynamic Operational Skills** — Reusable engineering standards (`explain`, `docs`, etc.) requested autonomously by the model via `[USE_SKILL: <name>]` without rigid personas or conflicting system prompts.
- **Interactive Monolith Decoupling** — Decoupled the 4,500-line `interactive.rs` into `uneven-core` domain services (`application::model_routing`, `infrastructure::knowledge::context_builder`) and specialized CLI components (`terminal_runner`, `slash_commands`, `tests`).
- **Centralized Resource Guardian** — Centralized hardware checks and memory pressure detection in `resource_guardian.rs` across all command delegators.

## [v2.1.1] — October 2026

- **Uneven Pods Architecture** — Persistent, dedicated clusters for application layers (`Database`, `Backend`, `Frontend`, `DevOps`) executing macro-missions with domain isolation.
- **Reflection & Self-Critique Engine (`think`)** — Deep 3-turn actor-critic audit checking proposals against 18 architectural criteria before modifying files.
- **Single-Run Heavy Command Guard** — Global asynchronous synchronization preventing concurrent heavy compilation/build steps.
- **Catfive Blind Masking & Security Hardening** — Advanced column and data masking for safe database introspection and analysis.

## [v2.0.1] — June 2026

- Keyless Web Search Integration — Added capabilities to query online documentation and encylopedic resources on-demand to answer queries with external context.
- Dynamic Site Structure Caching — Discovers and caches domain resources locally inside the global workspace directory to accelerate subsequent code lookups and references.
- Seamless TTY Stream Rendering — Buffered prompt streaming in the interactive interface to swallow control sequences and improve visual layout during code generation.
- Enhanced Intent Routing Heuristics — Optimized natural language intent routing to classify and forward web-search queries into the workspace assistant loop.

## [v2.0.0] — June 2026

- Configless Workspace Architecture — Redesigned boot flow to run out-of-the-box with zero setup files, automatically optimizing configurations lazily.
- Multi-Agent Task Orchestration — Support for delegating complex tasks to isolated subagents with safe session boundaries and recovery limits.
- Flexible Reasoning Effort — Active support for o1/o3-mini reasoning parameter configurations across OpenAI, Google Gemini, and Anthropic Claude.
- Custom Personas & Skills — Dynamic configurations to configure workspace-level custom rules and personas dynamically appended to AI context instructions.
- Fast Path Cache — Real-time path caching database utilizing file-list-cache to enable instantaneous lookup updates and avoid redundant indexing scans.
- Unified Command Prefix — Standardized all binary entrypoints and commands under the unified uneven executable instead of uneven-ai.
- Remote Shell & Configuration APIs — Interception of slash commands in chat channels and config GET/POST endpoints for external IDE extensions.
- Fast Startup Scanning — walkbuilder scan optimization ignoring compiler artifacts and caching directories to reduce boot LOOKUP times to milliseconds.

## [v1.12.2] — June 2026

- Prompt Optimization & Caching — Optimized context construction to leverage cloud-provider caching, significantly reducing token consumption and accelerating response times during interactive sessions.
- Smart Context Retrieval — Enabled the assistant to automatically locate and load additional files from the project as needed, ensuring it has all necessary context to solve complex tasks.
- Adaptive Session Memory — Automatically scales memory retention based on the capabilities of the selected model, preventing performance bottlenecks in local execution while maximizing depth on large cloud models.
- Streamlined Diagnostic Logs — Cleaned runtime logs and CLI diagnostic messages to ensure a clean, brand-neutral, and distraction-free workspace.
- Workspace Cleanliness — Automatically stores performance logs and reports in a dedicated directory, preventing clutter in the project root.
- Format & Parsing Fixes — Resolved stability issues where special characters in code files could cause parsing errors or layout breaks in the terminal.

## [v1.12.1] — June 2026

- Local Fallback Backups — Implemented a native fallback backup mechanism under .uneven/obsolete to copy files before any askf destructive action (overwrite or delete).
- Flexible Recovery Paths — Preserves the original file extension and relative directory structure with unique timestamp suffixes, enabling simple manual restoration.
- Offline/Gitless Fail-Safe — Direct instructions for users to recover overwritten files manually when Git is not configured or automatic reversion fails.

## [v1.12.0] — June 2026

- Engine Performance Optimization — Redesigned local workspace mapping and retrieval routines, slashing context preparation latency from ~200ms to less than 5ms (making CLI context preparation practically instantaneous).
- Workspace Mapping Speed — Optimized from ~120ms to ~4ms (30x faster).
- Vector Store Search (HNSW/Cosine) — Optimized from ~45ms to &lt; 1ms (45x faster).
- Physical File Reading (RAG) — Optimized from ~15ms to &lt; 0.2ms (75x faster).
- Lock-free Concurrent I/O — Thread-safe index access reducing concurrent I/O blocking from average/variable to zero.
- Stateful Data Analyst — Integrated sliding conversational memory into the Database Analyst, enabling continuous follow-up questions and query refinement in the same session.
- Token Optimization in Analysis — Applied smart context caching to data analysis schemas and rules, cutting token consumption by up to 90% on subsequent queries.
- Dedicated Remote Chat Route — Added a dedicated POST /chat endpoint to the Remote Shell, allowing IDE plugins to interact directly with the conversational assistant in-process.
- Self-Documenting API — Enabled a developer-friendly GET / route on the Remote Shell server that displays active routes, payload specifications, and authorization guidelines.

## [v1.11.0] — June 2026

- Zero-Day Vulnerability Scanning — Implemented advanced file-level scanning to detect custom security vulnerabilities and coding threats natively.
- Context-Aware Efficiency — Consolidated intent classification and file-path routing into a single reasoning step, reducing API queries by 33% and cutting response times in half.
- Immediate Conversational Responses — Enabled instant, single-turn answers for general software questions, bypassing secondary context calls to save time and token usage.
- Smart History Hardening — Implemented in-place system rules and files map injection in local session memory, maximizing cloud prompt caching to save up to 76% on API billing.
- Optimized Local BI Analyst — Fully integrated local database configurations with automatic schema introspection, custom table gating, and read-only query approval constraints.
- System Resource Optimization — Redesigned background worker processes to run with optimized CPU utilization and safety thresholds.
- Visual Interface Redesign — Redesigned CLI welcome headers and diagnostic logs into a clean, modern, and borderless left-aligned terminal layout.
- CLI Command Standardization — Standardized all command parameters, activation utilities, and offline logs under the unified uneven executable.

## [v1.10.6] — May 2026

- Background Process Daemonization — Resolved an initialization issue where the background proactive autofix service failed to start or detach properly under specific packaged distributions.
- Release Metadata Synchronization — Fully aligned internal version references, configuration fallbacks, and edge proxy distribution endpoints to ensure version compliance across all deployment targets.

## [v1.10.5] — May 2026

- Global Execution Pathing — Resolved pathing and subcommand resolution errors (such as background indexing and remote shell sessions) when running global command installations in specific system environments.
- Background Process Resilience — Corrected a silent initialization failure that prevented proactive auto-fixing and directory watchdogs from starting correctly under background execution contexts.
- System Version Standardization — Synchronized internal release tags and build metadata to ensure reliable updater verification and environment checks.

## [v1.10.4] — May 2026

- Context-Aware Dialogue — Restored deep conversational history in the interactive shell, enabling seamless follow-up questions and preventing memory drift.
- Dynamic Workspace Synchronization — Automatic cleanup of deleted files in the directory mapping to keep the agent's context strictly aligned with your local files.
- Clean File Generation Feedback — Replaced raw code stream printouts with elegant interactive loaders during file-writing tasks to preserve terminal readability.

## [v1.10.3] — May 2026

- Exact Streaming Token Metrics — Accurate real-time token tracking and local consumption billing metrics across all AI execution layers.
- Specialized Framework Diagnostics — High-fidelity error parsing and automated coding strategies tailored for modern web application architectures and secure routing constraints.
- Enhanced Smart Contract Heuristics — Specialized diagnostics targeting gas-efficiency optimizations and standard safety rules on decentralized code files.
- Improved Prompt Lifecycle Management — Graceful input loop protection inside the Conversational Shell, guaranteeing stable TTY confirmations during automated file-creation sessions.
- Decoupled Explainer Engine — Optimized performance path for file-level reviews and summaries, slashing query latency and token overhead.

## [v1.10.0] — May 2026

- Bring Your Own Key (BYOK) Pluggability: Seamlessly switch cloud providers (Google Gemini, OpenAI, Anthropic Claude) via standard configurations while retaining complete local ownership and control of API credentials
- Granular Resource Gating: Advanced schema access controls to restrict automated database actions to explicitly whitelisted tables only
- Native Telemetry Isolation: Absolute operational guarantee of zero source code tracking, zero telemetry transmission, and complete offline capability

## [v1.9.1] — May 2026

- Native Delivery Architecture — Optimized distribution pathways with flat resource usage for direct execution.
- Global Edge Intelligence — Real-time licensing and high-speed distribution powered by dedicated edge networks for instantaneous deployment.
- Enterprise Memory Protection: Active memory pruning and strict resource isolation guaranteeing flat RAM consumption under continuous operation
- Cloud Intelligence Stabilization: Optimized multi-modal alignment with Gemini and premium external providers
- Token Economy Engine: Vendor-adaptable orchestration reducing external cloud inference costs by up to 98%
- Business Intelligence & Dashboards: Natural-language data analysis with automated visual dashboard generation

## [v1.2.1] — May 2026

- Context Intelligence: Agent file selection now resolves with significantly higher accuracy
- Reliable response handling: AI suggestions are extracted correctly across all scenarios
- Large project efficiency: Knowledge base now handles codebases with many files more efficiently
- Accurate path resolution: File references in agent responses resolve correctly
- Expanded platform support: Pre-built binaries now cover additional operating systems

## [v1.1.9] — April 2026

- Attacker Attribution: Active pentest probes now silently transmit the origin IP for trace purposes
- Threat Detection: Terminal watcher identifies incoming Uneven probes and alerts the administrator
- License Hardening: Active Pentester mode now requires a Team tier license
- Shell Polish: Improved Portuguese intent classification and fixed false success log notifications

## [v1.1.8] — April 2026

- 3-Layer Privacy Protection: Enhanced data safety suite with Schema Filtering, Semantic SQL Auditing, and Live Result Redaction
- Forensic Audit Logs: Detailed local history of AI actions and system fixes stored in structured Markdown format
- Autonomous Security Pentester: Validated engine for detecting hardcoded secrets, injections, and infrastructure header flaws
- Universal Data Context: Seamlessly index and query .csv and .xlsx files for automated data processing tasks

## [v1.0.8] — April 2026

- Conversational Interface — launch an integrated natural language environment in any language directly on the free tier
- Hardware Optimization — automatic detection of accelerated hardware offloading capabilities with streamlined setup
- Expanded Capabilities — added commands for file explanation, automated documentation generation, and proactive task processing
- Uncapped Custom Inference — direct custom key compatibility providing complete usage flexibility
- Task Isolation — safe background job handling guaranteeing uninterrupted host system operation
- Stable Parallel Inference — robust parallel execution paths ensuring clean response delivery under load
- Configuration Resilience — absolute safeguard mechanisms preventing file modifications during system interruption
- State Preservation — continuous serial coordination protecting local knowledge base consistency

## [v1.0] — April 2026

- Stable public release — production-ready across all core features
- Streaming AI responses — output appears token by token instead of waiting for the full answer
- Human-centric fix approval — alertOnly mode suggests fixes for your review before applying anything
- Dynamic autofix toggle — switch between autonomous and assistant mode mid-session with --autofix
- Faster knowledge base — significantly smaller index files and faster search queries
- SBOM generation (uneven sbom) — export a software bill of materials for your project
- Improved provider support — updated model defaults and correct API endpoints for all external providers

## [v0.11.1] — April 2026

- Hardware Acceleration: Automatic GPU/CUDA offloading (32 layers max) for lightning-fast local inference
- Ethical Guardrails: Built-in defensive system that prevents malware, exploit, and cracking generation
- Advanced Document Intelligence: Native support for PDF, Word (.docx), Excel (.xlsx), and CSV Pro (industrial parsing)
- Strategic AI Recommendations: Preferred path for Local AI and Google Gemini Flash for privacy and cost-efficiency
- System Diagnostics: New `uneven info` command for real-time hardware, VRAM, and version auditing
- Memory Hardening: Silent model auto-unload (10min) and anti-loop safety (5-try ceiling)
- Local Data Analyst: Direct natural language queries over local CSV and Excel files without DB requirements

## [v0.7] — April 2026

- Process Execution Safety — absolute coordination protocols preventing instance interference
- State Resilience — safe snapshot processing safeguarding project indexing during power interruptions
- Ordered Audit Trails — linear event sequencing ensuring flawless history consistency
- Resource-Aware Scanning — optimized hardware utilization preventing system overload during security audits
- Network Deadlines — strict execution timers guarding against hanging requests
- Index Cost Preview — real-time intelligence estimating resource allocation before committing
- Custom Table Gating — granular user control over analyzed database segments
- Spreadsheet Analytics — automated reading of direct Excel document folders
- Humanized CLI Assistance — plain-language instruction strings for all commands

## [v0.4] — April 2026

- Pentester static mode (OWASP Top 10, secrets, CVEs, injections, headers)
- Pentester active mode with SHA-256 signed authorization scope
- Malware scanner: 8 categories, CI-compatible exit code (uneven scan)
- AI data analyst: natural language → SQL → Excel/dashboard (uneven analyze)
- CI/CD headless pipeline with GitHub Actions integration (uneven ci)
- Session state protocol (.uneven/session.json)
- Security report generator: HTML + Markdown output
- License system: free / Pro tiers with machine fingerprinting and offline grace period

## [v0.3] — Early 2026

- Proactive terminal watching & real-time monitoring
- Multi-language stack trace parser
- Retrieval-Augmented Fix (RAF) system
- Surgical code correction engine
- Autonomous file watcher with live re-indexing

## [v0.2] — Early 2026

- Advanced semantic retrieval engine
- Vector store integration with optimized HNSW search
- Deep indexing for projects and databases
- Native connectors for PostgreSQL, MySQL, SQLite, MongoDB
- Automated web documentation scraping

## [v0.1] — Early 2026

- Native core infrastructure stabilization
- High-performance bridge for local inference
- Initial CLI toolkit: uneven init, uneven ask
- Proprietary local brain support
- Multi-provider orchestration: OpenAI, Claude, Gemini, Ollama
