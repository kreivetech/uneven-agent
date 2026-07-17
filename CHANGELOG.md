# Changelog

All notable changes to Uneven Agent will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.2] - 2026-06-28

### Changed
- **Interactive UI Startup Optimization** — Reorganized the interactive prompt boot sequence to pre-load the welcome banner, the bottom status bar, and the initial prompt before initializing the LLM, making the CLI startup feel instantaneous and visual.
- **Conditional LLM Loader & Cursor Hiding** — Implemented conditional cursor blocking and status bar feedback during local model loading. For cloud providers, the initialization runs instantly without blocking or hiding the cursor.
- **Lazy Local Embedding Loading** — Deployed conditional BERT embeddings initialization inside the core `init_llm` loader to only trigger when using local models (`provider == "local"`), eliminating unnecessary RAM usage and tensor loading during cloud-only sessions.

### Fixed
- **Duplicate Prompt Rendering** — Configured the interactive shell loop to clear the prompt line using ANSI escape sequences (`\r\x1b[2K`) exactly before calling `readline`, preventing duplicated or overlapping prompt displays during redraws.

## [2.0.1] - 2026-06-27

### Added
- **Dynamic File Retrieval Prompting (`[NEED_FILE]`)** — Formally integrated the agentic file auto-recovery instructions into the system prompts (`ask.md` and `chat.md`), allowing the LLM to actively request missing files using the `[NEED_FILE: relative/path]` tag.
- **Custom Habilities & Skills Documentation** — Created a comprehensive markdown guide under `docs/CUSTOM_SKILLS.md` outlining the folder structure, rules parsing (`AGENTS.md` and `rules.md`), and Custom Persona setup.
- **Remote Shell Architecture Documentation** — Updated `docs/ARCHITECTURE.md` to document the HTTP/HTTPS custom server design, CORS whitelisting, Bearer secret brute-force locking, and route execution pipelines.
- **Slash Command `/stats` Integration** — Integrated `/stats` (and `/cost`) into the slash commands list inside the interactive prompt loop. Users can now type `/stats` or select it directly from the `/` menu to print real-time session, daily, and monthly token usage and costs.
- **Financial Budget Limit & Direct Limit Command** — Converted the monthly budget tracking from an arbitrary token count to a USD dollar amount (`monthly_limit_usd`, default `$10.00`). Users can now dynamically adjust their budget via `/stats limit <value>` directly from the prompt loop.
- **Keyless Web Search & Sitemap Caching** — Implemented an autonomous web search and scraping engine inside the interactive shell (`interactive.rs`) and shell engine (`shell_engine.rs`) utilizing DuckDuckGo and Wikipedia APIs. Added background domain sitemap discovery (`sitemap.xml`/`sitemap.txt`) caching sitemaps locally to the project's global cache directory. When executing searches, the agent automatically scans the sitemaps index to prioritize matching cached URLs.


### Changed
- **De-npmification & Pure Standalone Binary** — Cleaned up all documentation, including `README.md` and `llms.txt`, by purging references to npm global installation packages, node_modules dependencies, NAPI-RS node integration, and programmatic typescript configurations to reflect that Uneven Agent is strictly distributed as a standalone binary since v1.10.6.
- **Standardized Command Prefix** — Replaced all deprecated `uneven-ai` command lines in instruction files with the standard `uneven` command prefix.
- **Up-to-Date LLM Pricing Rates** — Calibrated the `COST_TABLE` to match current June 2026 pricing for OpenAI (GPT-5.5 / 5.4), Claude (4.8 / 4.6 / 3.7 / 3.5), Gemini (3.5 / 3.1 / 3 / 2.5), DeepSeek V4, and Groq (Llama 3.3 / 3.1 / Qwen3). Added smart local suffix filters to ensure local quantized models (-q4, -q8, phi) are always calculated as free ($0.00) instead of matching cloud provider pricing.
- **Unified Session ID for Caching Stability** — Standardized `session_id` management inside `interactive.rs` and `multi_agents.rs` by generating a single session ID once at startup and reusing it throughout the session. This prevents generating unique UUIDs on every sequential prompt, keeping the system prompt and conversation prefix byte-identical to ensure high prompt cache hit rates on Claude.


### Fixed
- **Terminal UI Corruption on Exit** — Deployed `restore_terminal_ui` across all interactive shell termination paths to reset terminal scroll bounds (`\x1b[r`) and wipe leftover bottom status bar artifacts, preventing CLI rendering leakage into the user's main shell.
- **Raw-mode SIGINT Interruption** — Configured `interactive.rs` to intercept rustyline `ReadlineError::Interrupted` (`Ctrl + C`) in raw mode and trigger immediate safe UI restoration followed by clean process exits.
- **Invisible Text Input Cursor** — Added explicit blinking and visibility escape sequences (`\x1b[?25h\x1b[?12h`) before rustyline reads to resolve invisible prompt cursor navigation issues in terminal interactive mode.
- **Failing Fallback Askf Test** — Fixed `test_parse_files_fallback` in `askf.rs` by updating the file block matching regexes to accept filename markers on the same line.

## [2.0.0] - 2026-06-21 [STABLE]


### Added
- **Configless Workspace Architecture** — Completely redesigned the boot flow to run out-of-the-box with zero workspace files. Generates configuration lazily in the global project cache directory.
- **Fast Path Path-Caching** — Implemented a real-time, two-phase path caching engine utilizing `file-list-cache.json` to enable instantaneous lookup updates and avoid redundant indexing traversals.
- **Uninstall Command** — Introduced `uneven uninstall` command to completely clean up global binaries, configuration cache folders, environment files, and helper path exports.
- **Custom Skills Directory Configuration** — Added the `/skills` slash command (and `"skills"` config JSON key) to allow configuring custom folders for workspace-level rules and skills dynamically appended to the LLM system instructions.
- **Custom Agent Persona Configuration** — Added the `/person` slash command (and `"persona"` config JSON key) to allow configuring custom agent descriptions and modular personas, dynamically prepended to the system context in place of the default software engineer identity.
- **Reasoning Effort Configuration** — Integrated support for reasoning effort / thinking parameters across OpenAI (o1/o3-mini), Google Gemini (Gemini 2.5/3), and Anthropic Claude (Claude 3.7 Sonnet). Added the `/effort` and `/reasoning` slash commands and the `"reasoningEffort"` configuration key.
- **Remote Shell API & Slash Commands** — Added `/config` GET and POST endpoints to the Remote Shell server, allowing external clients (like IDE extensions) to read and patch configuration dynamically. Integrated interactive slash command interception (`/model`, `/person`, `/effort`, etc.) in the conversational chat and message channels.
- **Multi-Agent Task Orchestration** — Integrated support for delegating complex, multi-step, or multi-file programming tasks to isolated subagents using the `/agents` toggle slash command. Subagents run in sandboxed contexts with unique session IDs, proportional token limits (minimum floor of 10k output tokens per subagent), and a strict safety limit of 3 modified files to prevent codebase pollution.
- **Sanitized Subagent Error Mapping** — Added 4 specific error codes (UE-06000012 to UE-06000015) in Category 06 (Automation) to map subagent-specific failures (initialization, inference, file write, and file limit exceeded) to sanitized user-facing messages, preventing technical leakage to the user.
- **Multi-Agent Task Resumption Cache** — Implemented task list serialization in `multi_agent_tasks.json` within the project cache directory. This preserves outstanding subtasks in case of token exhaustion or interruption, allowing the user to seamlessly resume the remaining steps upon restarting.
- **Session-Isolated Resumption Cache** — Updated multi-agent task caching to key file names using session ID suffixes (e.g. `multi_agent_tasks_<session-id>.json`), resolving race conditions in concurrent multi-user environments like Remote Shell.
- **Fast Startup & Directory-Ignore Scan Optimization** — Refactored the file tree indexer scan by configuring `WalkBuilder` to ignore compiler artifacts and hidden directories (like `target/`, `.git/`, `node_modules/`), reducing boot lookup traversal time from seconds/minutes to milliseconds.
- **Watcher Lock Contention & Event Filter** — Prevented recursive modify loops in file watching by filtering directory-level modify events under the `.uneven/` project folder, resolving CPU and IO resource exhaustion.
- **Interactive Initializer Spinner** — Integrated active loading visual feedback (a spinner) in the interactive shell initialization to replace terminal lag and flickering during model boot up.
- **Multi-Agent Task Interruption & Cancellation** — Deployed an asynchronous, non-blocking `abort_monitor` capturing keyboard actions (`stop`, `abort`, `exit`) to instantly cancel subagent runs and return control to the main interactive prompt.
- **EOF Handled Gracefully in Stdin Reader** — Updated input loops to exit cleanly on EOF (like closed stdin or redirection) instead of spinning CPU resources in an infinite loop.

### Changed
- **Unified Command Prefix** — Standardized all binary entrypoints and CLI command instructions to use the `uneven` command prefix instead of `uneven-ai`.
- **First-Run UX Polish** — Improved first-run welcome banner detection to show an introductory description rather than local model diagnostic info before model choices, and removed confusing "Zero cloud" references.
- **Global Path Relocation** — Moved all active state tracking, scraped assets, and database analyst caching from the local project root into the global project cache directory under `~/.uneven/bin/cache/<project-hash>/`.
- **High-Efficiency Markdown Laws Optimization** — Redesigned and streamlined all internal markdown system instructions (laws) in `uneven-contexts/` (such as `shell.md` and `askf.md`) to neutralize hardcoded personas and reduce input token overhead by over 50%.
- **Robust Modularity Rules** — Refined internal context rules in `askf.md` to prevent LLM code destruction and truncation hallucinations during file edits, clarifying that modularity line limits only apply to newly created files.
- **Robust API & Shell Sanitization** — Consolidated error handling across the interactive shell and the Remote Shell API endpoints to guarantee all technical database, compiler, and LLM inference error details are sanitized via the error code dictionary before presentation.
- **Sanitized Git Review Traces** — Applied `sanitize_error_msg` to Git diff load errors in the interactive `Review` action block to prevent low-level command outputs or repository paths from leaking to users.

## [1.12.2] - 2026-06-16 [STABLE]

### Changed
- **Engine Economics & Caching** — Consolidated prompt history caching rules to maximize provider-side caching benefits.
- **Agentic File Auto-Recovery** — Implemented a three-cache system (Conversation History, Loaded Files Registry, and Formatted Content Cache) inside the interactive CLI session. This allows the model to request and load extra codebase files dynamically using `[NEED_FILE: path]` tags while preventing infinite loops and redundant disk I/O.
- **Dynamic Conversation History** — Adjusted TTY conversation history limits dynamically based on active LLM provider specifications.
- **Strict Brand-Neutrality** — Refactored all internal system prompts and diagnostic logs to remove brand names and attribution.
- **Dedicated Benchmark Directory** — Moved time-series logs and outputs to a dedicated subfolder to keep the workspace root clean.
- **Regex Tag Escaping** — Fixed format character escaping in CLI regex configurations to resolve bracket parsing bugs.

## [1.12.1] - 2026-06-14

### Added
- **Local Fallback Backups** — Implemented a native fallback backup mechanism under `.uneven/obsolete` to copy files before any `askf` destructive action (overwrite or delete).
- **Flexible Recovery Paths** — Preserves the original file extension and relative directory structure with unique timestamp suffixes, enabling simple manual restoration.
- **Offline/Gitless Fail-Safe** — Direct instructions for users to recover overwritten files manually when Git is not configured or automatic reversion fails.

## [1.12.0] - 2026-06-11

### Added
- **Stateful Database Analyst** — Integrated conversational memory inside the `uneven analyze` session, allowing users to ask follow-up questions and refine SQL statements continuously.
- **Smart History for Data Introspection** — Applied context caching to DB schemas and security rules in data analyst tasks, reducing token consumption by up to 90% on subsequent queries.
- **Dedicated Remote IDE Chat Route** — Added a dedicated `POST /chat` route to the Remote Shell server, allowing IDE integrations (like VS Code chat panel) to communicate directly with the stateful conversational shell in-process.
- **Self-Documenting API Help** — Implemented a developer-friendly `GET /` route on the Remote Shell server that displays active endpoints, payload structures, and authorization guidance.

## [1.11.0] - 2026-06-02

### Added
- **Edge Telemetry with Cloudflare D1** — Implemented an edge-based SQLite telemetry storage using Cloudflare D1 to track installations, updates, active users, and active license tiers/status with zero upstream overhead.
- **Visual Edge Dashboard** — Created a secure, premium dark-themed HTML5 visual admin dashboard at `/api/admin/dashboard` utilizing Chart.js to render real-time telemetry metrics.
- **Client Telemetry Integration** — Updated the native Rust CLI to generate anonymous persistent device IDs (`~/.uneven/device_id`) and safely transmit pings on startup.

### Changed
- **Pure JavaScript Edge Worker** — Replaced the legacy TypeScript Edge Worker with a native JavaScript Cloudflare Worker (`cloudflare-worker.js`) to comply with direct Cloudflare deployment requirements.
- **Version Propagation** — Upgraded the workspace packages and all version references across the CLI, templates, and reporter engines to `1.11.0`.

---

## [1.10.6] - 2026-05-22

### Fixed
- **Proactive Autofix Binary Spawning** — Corrected the daemonization logic inside `autofix` command to properly resolve and spawn from virtual filesystems (`/$bunfs/`) inside Bun compiled standalone binaries, ensuring background process initialization works identically to Node execution.

### Changed
- **Version Propagation** — Bumped and synchronized CLI and core configurations, tests, fallbacks, and UI components to version `1.10.6`.

---

## [1.10.5] - 2026-05-21

### Added
- **Stateful Interactive History** — Restored sliding conversational memory capped at 6 turns (`MAX_HISTORY`) inside the shell interactive prompt loop to prevent LLM context repetition and hallucinations.
- **Index Pruning** — Added `pruneDeletedFiles` method to `IncrementalIndex` that automatically cleans up hashes and data of physically deleted files, ensuring the index and the folder map always stay in sync with the project's current state.

### Changed
- **Opaque File-Writer Streams** — Intercepted raw code token streams during file generation tasks (`askf`), displaying a clean, interactive spinner feedback instead of dumping token blocks in the conversational shell.

### Fixed
- **Integration Test Mocks** — Aligned `__tests__/shell-interaction.test.ts` mock parameters and assertions to pass under the stateful history parameter injection.

---

## [1.10.3] - 2026-05-18

### Added
- **Exact Streaming Token Metering** — Integrated physical token usage tracking directly into streaming provider adapter loops:
  - **Google Gemini**: Parsed and extracted official `usageMetadata.totalTokenCount` from the streaming JSON chunks.
  - **Ollama**: Captured and aggregated local inference `eval_count` and `prompt_eval_count` on completion payloads.
  - **Universal Fallback**: Added safety character-to-token fallback (`chars / 4`) in the core provider resolver to guarantee active metrics even if upstream APIs yield null values.

### Fixed
- **Readline Lifecycle in Shell Command** — Resolved `ERR_USE_AFTER_CLOSE` errors by moving `rl.close()` out of the synchronous `finally` block inside the interactive `shellCommand` loop. The readline interface is now closed gracefully at the end of each execution turn (or on explicit break/continue statements), enabling inner interactive prompts (such as `askf` file overwrite prompts) to fully leverage `stdin` without throwing socket closures.

---

## [1.10.2] - 2026-05-17

### Added
- **Solidity Smart Contracts Heuristic Strategy** — Dedicated diagnostic fixing rules for Solidity (`.sol`), covering SPDX license identifiers, explicit visibility, and view/pure function optimizations for Gas cost efficiency.
- **Unified Backend Framework Specialized Coverage** — Integrated high-fidelity, high-confidence strategy layers for leading 2025/2026 backend frameworks:
  - **Laravel (PHP)**: Automated Mass Assignment (`$fillable`) and CSRF Token validation and suggestions.
  - **Spring Boot (Java)**: Real-time discovery of missing stereotype annotations and mutual circular dependency resolving via `@Lazy`.
  - **Ruby on Rails**: Strong Parameters missing keys resolution (`ParameterMissing`) and ActiveRecord fallback options.
  - **Gin (Go)**: Smart detection of `ShouldBindJSON` non-pointer reference binding mismatches and invalid router signature parameter types.
  - **Axum (Rust)**: Compiler-level diagnostics for routing extractor ordering constraints and thread-safe Send/Sync constraints for async route handler futures.

### Changed
- **Strategy Precedence Mapping** — Reordered the central `StrategyRegistry` layout to prioritize all framework-specific heuristics ahead of raw language rules, optimizing offline success rates.
- **Explain and Ask Engine Decoupling** — Separated the file explanation pipeline from the conversational ask flow. Introduced a dedicated `uneven.explain()` method, bypassing conversational RAG (HNSW vector searches and Knowledge Map routing) to slash latency and token waste.

### Fixed
- **Unused Import Cleanups** — Resolved minor typescript lints by purging unused imports from the newly designed framework components.

---

## [1.10.0] - 2026-05-14

### Added
- **BYOK Cloud Pluggability** — Unified support for seamless cloud provider swapping (Google Gemini, OpenAI, Anthropic Claude) via a single configuration line, ensuring user-owned API keys are executed strictly within the local environment without proxying or credential storage.
- **Granular Database Access Policies** — Explicit whitelisting protocols (`allowedTables`) in `uneven.config.ts` enforcing zero-trust resource gating to prevent direct AI access to restricted schema objects.

---

## [1.9.2] - 2026-05-13

### Added
- **Opaque Standalone Binary Smoke Testing** — Introduced automated multi-platform End-to-End smoke testing suite targeting standalone release executables directly via isolated sandboxes (`smoke-sandbox/`).
- **Universal Error Opacity Protocol** — Established a strict policy ensuring zero exposure of raw native engine paths, low-level Rust stack traces, or internal DevSecOps exceptions to the user terminal.

### Changed
- **Insight Passthroughs** — Refined error priority mapping in `llm-error.ts` to ensure helpful database relationship instruction codes (`[UE-03000011]`) take precedence over generic masking.
- **Log Minimalism** — Silenced internal stream drop warnings and background stale-lock release logs to preserve terminal clarity.

## [1.9.0] - 2026-05-07

### Added

- **Cloud Intelligence Stabilization** — Complete alignment with the `gemini-2.5-flash` provider. The engine now handles multi-modal responses and unified usage metadata tracking across all 14 local and cloud models.
- **Token Economy Engine** — New local-first orchestration that reduces cloud token consumption by up to 98% by surgically selecting context before dispatching to external APIs.
- **Business Intelligence & Dashboards** — Pro/Team tier now supports natural-language data analysis with automated dashboard generation. All PII data is masked locally before schema reasoning.
- **Webhook Notifications** — Built-in support for Slack and Discord alerts on critical system errors or successful autonomous fixes.
- **Remote Shell (Beta)** — Secure HTTPS/Bearer authenticated endpoint to interact with Snatchy from external services.
- **Technical Blindness Policy** — Standardized error sanitization (UE-XXXX) to prevent internal system architecture leaks in any environment.

### Changed

- **CI/CD Guard** — Default `npm test` now ignores cloud-integration tests to prevent pipeline failures in environments without API credentials.
- **NAPI Bridge** — Standardized JSON normalization (snake_case to camelCase) for all native return values.

### Fixed

- **Gemini REST Compatibility** — Authentication migrated to query-string parameters to resolve 401/404 errors in legacy headers.
- **Context Assembler Resiliency** — Fixed a crash scenario when local models were unavailable during hybrid inference.

## [1.3.0] - 2026-05-02

### Added

- **Multi-architecture GGUF dispatcher** — The Rust inference loader now reads `general.architecture` from GGUF metadata and routes to the correct candle-transformers backend (`quantized_llama`, `quantized_qwen2`, `quantized_gemma3`, `quantized_phi3`). All seven catalog models are now supported out of the box.
- **Chat templates** — Each model family now receives prompts in its correct format: Llama 3 (`<|begin_of_text|>` header tokens), ChatML (`<|im_start|>`, used by Qwen and SmolLM2), DeepSeek-R1 (Unicode BOS/EOS tokens), Gemma 3 (`<start_of_turn>`), and Phi 3/4 (`<|system|>` role tokens). The `detectTemplate` helper selects the right template from the model ID automatically.
- **DeepSeek-R1 thinking-token stripping** — `<think>…</think>` blocks emitted by DeepSeek-R1 models are removed before the response reaches the user.
- **Adaptive inference timeout** — Local inference timeout is now `max(300 s, maxTokens × 1.2 s)` instead of a fixed 120 s, preventing spurious timeouts on slower hardware.
- **`llm-error` utility module** — `sanitizeLlmError` extracted to `src/infrastructure/utils/llm-error.ts` for isolated unit testing without the native binding mock.
- **Unit tests — chat templates (60 tests)** — Per-architecture token structure, content integrity, multi-turn history ordering, and per-model send/receive simulation for all seven catalog models.
- **Unit tests — error sanitisation (12 tests)** — Covers every error category mapped by `sanitizeLlmError`.

### Fixed

- **Double BOS token** — `encode(prompt, true)` was adding a BOS token on top of the one already embedded in the chat template, corrupting Llama 3 prompts. Changed to `encode(prompt, false)`.
- **Default model ID** — Fallback model in the NAPI layer updated from `llama-3.2-1b-q8` to `llama-3.2-1b-q4` to match the catalog.

---

## [1.2.2] - 2026-05-02

### Fixed

- **Local model path** — After `uneven-ai init`, commands such as `index` and `ask` failed with `Cannot open ./models/…: No such file or directory`. The Rust engine was resolving model files relative to `./models/` while the installer writes them to `.uneven/models/`. Both the inference loader (GGUF + tokenizer) and the embeddings module now read from `.uneven/models/` consistently.

---

## [1.2.1] - 2026-05-01

### Fixed

- **Auto-fix rollback** — In certain error scenarios the engine would skip the rollback step, leaving partially applied changes on disk. Rollback is now reliably triggered on every failure path.
- **Shell crash on fix confirmation timeout** — When the auto-fix confirmation prompt timed out, Node.js could throw an unhandled rejection and exit. The prompt now degrades gracefully.
- **Fix engine context accuracy** — The code context sent to the AI for generating fixes was occasionally pulled from the wrong region of the file. The engine now uses an accurate line-based extraction window.
- **AI confidence out of range** — The fix engine could accept confidence scores outside the valid 0–1 range, skewing fix prioritization. Scores are now clamped correctly.
- **Knowledge Map file routing** — Files were occasionally matched incorrectly when two files share a similar suffix (e.g. `user.ts` and `power-user.ts`). Path boundary matching is now enforced.
- **Knowledge Map multi-line response parsing** — When the AI returned a formatted (multi-line) file list, the parser silently dropped all entries. Both compact and formatted responses are now handled correctly.
- **Knowledge Map path corruption** — A stray trailing comma in the fallback parser produced unresolvable paths, causing context retrieval to fail silently.
- **Knowledge Map context overflow** — On large projects with many indexed files, the full file list could overflow the AI context window. The map is now capped at 800 entries with a clear truncation notice.
- **Watcher stability** — An uncaught error during a proactive scan could bring down the file watcher. Errors are now isolated and logged without interrupting the watch loop.
- **Index integrity** — A failed file read during indexing could record an empty hash, causing the file to be treated as unchanged on the next run. Affected files are now skipped until readable.
- **Remote shell hardening** — Improved robustness of the built-in remote shell endpoint.
- **Session lock reliability** — A lock-timeout error could be silently swallowed under concurrent load. Errors are now surfaced to the logger.
- **Local model loader stability** — Under concurrent usage, the local model could be loaded more than once simultaneously. Loader now serializes concurrent requests correctly.
- **Symlink loop protection** — Directory indexing could enter an infinite loop when the project contained circular symbolic links. Symlinked directories are now skipped during traversal.
- **Large file context cap** — Files without a size cap could saturate the AI context window when used as reference material. Content is now capped at 8 000 characters with a truncation notice.

---

## [1.2.0] - 2026-04-29

### Added
- **Global Strategy Architecture**: Introduced `src/utils` and `src/interfaces` to centralize shared constants and contracts, significantly reducing circular dependencies and improving modularity.
- **Data Analyst Strategy Pattern**: Refactored the monolithic `analyzer.ts` into a decoupled architecture using a Strategy pattern. Database-specific logic is now isolated in `KnexAdapter` and `MongoAdapter`.
- **Centralized Security Definitions**: Created `src/utils/security-definitions.ts` to host all security constants (`PORTS`, `REQUIRED_HEADERS`) used by the analysis engines.
- **Legal Compliance Flow (Frontend)**: Implemented `LegalGateModal` with dynamic localized links (BRL/PT vs USD/EN) and mandatory consent interceptor in the pricing flow.

### Changed
- **Pentester Modernization**: Deep refactoring of the `active` analysis module. Procedural logic in `helpers.ts`, `http.ts`, and `network.ts` replaced with modern asynchronous patterns and functional programming (Map/Filter).
- **Interface Segregation**: Moved all domain-specific interfaces to `src/interfaces/data-analyst.ts` and `src/interfaces/active-analysis.ts`.
- **Infrastructure Cleanup**: Streamlined core context files (`context.ts`) by removing heavy data definitions and focusing on runtime configuration.

### Fixed
- **Mobile UX (Frontend)**: Resolved horizontal overflow issues hiding the mobile menu; fixed logo font-size and spacing constraints for small screens.
- **CSS Syntax Error**: Corrected invalid `items-center` property in the Legal Modal to proper `align-items: center`.

## [1.1.9] - 2026-04-27

### Added
- **Attacker Identification (Uneven Trace)**: Active pentester now injects a silent origin-tracking header (`X-Uneven-Origin-Trace`) containing the attacker's machine IP.
- **Threat Detection Engine**: Terminal watcher now detects incoming Uneven pentest probes and logs a critical security alert with the source IP.
- **Portuguese Intent Support**: Shell now recognizes "crie", "faça", "escreva" for AI tasks even in keyword-fallback mode.

### Changed
- **License Gating**: Active Pentester mode is now restricted to the **Team** plan.
- **Shell Stability**: Improved intent classification to prevent substring collision (e.g., "mundo" no longer triggers "undo").
- **Success Hints**: Shell now only prints success hints if the underlying command actually returns a success exit code.

## [1.1.8] - 2026-04-27

### Added
- **3-Layer Privacy Protection**: Enhanced data safety suite for database analysis, featuring **Schema Filtering** (AI never sees blocked tables), **Semantic SQL Auditing** (blocks unauthorized access patterns), and **Live Result Redaction** (masks sensitive values in real-time).
- **Forensic Audit Logs**: New `log.path` configuration that persists a detailed history of all AI actions, terminal errors, and applied fixes in a structured Markdown format for accountability.
- **Autonomous Security Pentester**: Validated static analysis engine capable of detecting hardcoded secrets (Entropy-based), OS/SQL injections, and missing infrastructure security headers (HSTS, CSP).
- **Universal Data Indexing**: Full support for `.csv` and `.xlsx` files, enabling seamless context delivery for automated data scripts.

### Fixed
- **Configuration Loading**: Resolved a critical issue where the `analyze` command would ignore the project's `uneven.config.ts`.
- **Infrastructure Stability**: Fixed a build-breaking import error in the Pro feature gate (`gate.ts`).
- **Provider Reliability**: Improved Gemini API error handling for rate-limit and overload scenarios.

## [1.1.5] - 2026-04-24

### Fixed
- **Setup Reliability**: Restored safe workspace initialization logic to prevent file permission conflicts during setup.

## [1.1.4] - 2026-04-24

### Changed
- **System Stability**: Major update to core internal dependencies to ensure maximum compatibility and long-term stability.
- **Engine Optimization**: Significant performance enhancements across the primary analysis modules for faster processing.
- **Enhanced Installation Flow**: The initialization process has been refined for a more reliable out-of-the-box experience.
- **Improved Context Resolution**: Refined the intelligence layer to better handle complex project structures and file references.
- **Global Synchronization**: Optimized cross-layer communication for consistent and accurate project indexing.

## [1.1.3] - 2026-04-24

### Added
- **Proactive Code Auditing**: New background engine that scans for logic and security issues during active development sessions.
- **Extended Project Compatibility**: Massive expansion of the analysis engine, now supporting a vast array of modern file formats and ecosystems.
- **Autonomous Agent Intelligence**: Enhanced conversational capabilities allowing for deep project-wide audits via natural language.
- **System Orchestration Refinement**: Core architectural improvements for better reliability and faster response times.

## [1.1.2] - 2026-04-24 [DEPRECATED]

> [!CAUTION]
> **This version is deprecated.** Users are strongly encouraged to upgrade to **v1.1.3** to benefit from the new Proactive Analysis engine and universal language support.

### Fixed

- **Engine Initialization**: Resolved a naming convention mismatch where `maxTokens` (camelCase) from the TypeScript configuration was not being correctly mapped to `max_tokens` (snake_case) expected by the native Rust engine.
- **Native Bridge**: Updated the bridge to ensure all configuration fields are correctly transformed before being passed to the native core, preventing initialization failures.

## [1.1.1] - 2026-04-24

### Fixed

- **Naming Conventions**: Stabilized internal naming conventions between TypeScript and Rust layers.

## [1.1.0] - 2026-04-20

### Added

- **Privacy Hardening**: Enhanced protection for development environment privacy. Absolute paths from the host machine are now automatically stripped from distribution binaries.
- **Scalability**: New memory-optimized indexing architecture for massive codebases, allowing the engine to scale to millions of lines with a minimal RAM footprint.
- **Project Integrity**: Real-time configuration schema validation to prevent malformed settings from affecting system stability.

### Fixed

- **Stability**: Improved sub-process management to ensure clean termination of development servers and dependent tasks, preventing orphan processes and resource leaks.
- **Reliability**: Advanced loop protection and binary-header identification in the knowledge indexing engine for safer directory traversal.
- **Error Propagation**: Enriched internal error system for more precise diagnostics and user feedback during AI inference.

## [1.0.8] - 2026-04-19

### Fixed

- **watcher/terminal**: `start()` now returns `Ok(None)` for non-zero exit codes instead of silently returning `Ok(())`; `Send + 'static` bound removed from `stream_output` callback; `&mut self` corrected to `&self`; I/O errors on reader tasks now emit warning instead of silent EOF
- **watcher/error_parser**: `message` now extracts the line containing the regex match instead of always returning the first line of output (fixes preamble cases like Rust panics)
- **watcher/mod**: path traversal guard added to `watch_directory` — `../` paths are now rejected
- **retrieval**: cosine similarity clamped to `max(0.0)` before `to_bits()` ordering — negative scores were ranking semantically opposite documents above similar ones
- **retrieval**: `write_hnsw` index save now uses atomic tmp+rename — SIGKILL during save no longer corrupts the index permanently
- **retrieval**: `stats()` now returns distinct values for documents and embeddings — both fields were identical before, making diagnostics useless
- **retrieval**: `clear()` now uses `tokio::fs::remove_file` — was blocking the Tokio executor with synchronous `std::fs` calls
- **retrieval**: lock poisoning in `hnsw_backend` handled via `unwrap_or_else(|e| e.into_inner())` in 6 locations
- **pentester/static_analysis**: `scan_secrets` was a no-op — fully reimplemented with BFS file walk, static `Lazy<Regex>` and minimum-length filter on API key patterns
- **pentester/port_scanner**: empty host now rejected with `PentesterError`; scan concurrency capped at 50 via batching; non-ECONNREFUSED errors propagated instead of swallowed
- **pentester/mod**: `static_analysis()` and `active_testing()` were stubs returning empty results — now delegate to their submodules; `SecurityFinding.severity` promoted from unvalidated `String` to typed `enum Severity`; CIDR `/0` now rejected
- **llm/inference**: stale `INTERRUPT` flag from a previous call no longer silences the next generation; race condition in `unload_model` between `LLAMA` and `LOADED_MODEL` closed; `is_model_loaded` uses `AtomicBool` — no longer returns `false` while writer holds the lock
- **llm/inference**: `run_inference` now returns real token count and real model name instead of hardcoded values
- **llm/embeddings**: double-init race fixed with `AtomicBool::compare_exchange`; lock poisoning handled via `unwrap_or_else(|e| e.into_inner())`
- **knowledge/indexer**: `index_directory` was checking path type but never traversing the filesystem — BFS walk now implemented; symlink cycle protection via `entry.file_type()`; path traversal guard added
- **knowledge/file_loader**: TOCTOU `exists()` check removed; extension check now runs before filesystem access
- **agent/fix**: `generate_patch` now produces valid unified diff with headers; `apply_patch` uses atomic write-then-rename; path traversal guard and stale-snapshot guard added
- **agent/mod**: `apply_fix` always returned `Ok(true)` without touching files — now delegates to `fix::apply_patch`; `diagnose` now delegates to `diagnose::diagnose_error`
- **napi**: `store_documents_batch` validates that all five input slices have equal length — `zip` was silently truncating to the shortest; `load_file_content` path traversal guard added
- **application/engine_service**: `initialize` now validates `brain.provider` and `brain.model`; concurrent-init race closed with `AtomicBool`; `UnevenEngine` singleton wired to NAPI via `OnceLock`
- **utils/config-loader**: partial `brain` override no longer overwrites config defaults with `undefined`; `syncConfigFromTs` uses atomic write-then-rename; backslash-escaped quote detection corrected
- **utils/migration**: `ENOTEMPTY` on already-migrated destination no longer stalls migration on every startup; per-file rename errors now tracked individually
- **utils/process-lock**: `release()` removes SIGINT/SIGTERM/exit listeners — prevented `MaxListenersExceededWarning` accumulation in test suites
- **utils/error-parser**: `extractStackTrace` now captures the exception message line preceding the first stack frame
- **license/gate**: expired pro license (`active: false`) no longer granted access; `process.exit(0)` on blocked feature corrected to `process.exit(1)`
- **license/manager**: `status()` now reflects real expiry; HTTP error responses no longer silently extend grace period; `writeLicense` failures inside `.catch()` no longer produce unhandled rejections
- **license/storage**: `readLicense()` validates all required fields at runtime — partial/corrupt cache no longer propagates `undefined`; `writeLicense` uses atomic write-then-rename
- **io/git-manager**: shell injection in `checkout`, `deleteBranch`, `revertCommit`, `commitFix` fixed — migrated to `execFileSync`/`execFile` with argument arrays
- **io/web-scraper**: concurrent fetch capped at 10; `undici` Agent reused via singleton instead of instantiated per request
- **io/process-watcher**: SIGKILL fallback added after 5s if process ignores SIGTERM; `off()` method added for listener removal
- **io/file-watcher**: `start()` no longer hangs indefinitely — error handler and 30s timeout added
- **io/logger**: UTC fields used consistently in `getTimestamp()`; `debug()` now writes to log file
- **io/db-loader**: empty `tables: []` now warns instead of silently loading 0 documents; DB type detection migrated from substring match to `URL.protocol`; `sanitizeUrl` redacts username in addition to password
- **adapters/external-providers**: Gemini safety-filter responses no longer throw TypeError; streaming readers closed in `finally` across all four providers; Gemini NDJSON parser uses proper `JSON.parse` instead of string-match heuristic
- **domain/resource-guardian**: uncaught callback exceptions in `setInterval` no longer crash the process; CPU hot-plug length mismatch guarded
- **domain/snapshot**: `restore()` validates that target path is inside project root before writing
- **domain/session**: `lockTimeoutHandle` nullified before `forceReleaseLock` — prevented new lock from being cancelled by expired-timeout cleanup
- **cli/analyze**: double-disconnect race in `runBatchDashboard` fixed; spinner interval leak on thrown error fixed
- **cli/askf**: `isSafeFilename()` guard added — LLM-supplied filenames with path traversal or absolute paths now rejected
- **cli/ci**: `__UNEVEN_GH_LINES` env var replaced with module-level array; external `Promise.race` deadline added to prevent indefinite CI hang
- **cli/log**: `readFileSync` replaced with `createReadStream` — large log files no longer risk OOM
- **orchestration/engine**: `unloadTimer`, `saveProgress` and `activeFixes` race conditions fixed; `promptUserConfirmFix` and `llmInfer` now have timeouts; `docType` inconsistency between index phases corrected
- **orchestration/incremental-index**: `markIndexedWithHash` rejects empty hash, negative size, or non-array `chunkIds`
- **orchestration/knowledge-retriever**: ReDoS in `extractSource` fixed (`\S+` → `\S{1,500}`)
- **development/fix-engine**: snapshot-before-lock race fixed; prompt injection via `error.message` sanitized; path traversal guard added; `fixAttempts` memory leak fixed
- **development/test-runner**: SIGKILL flag logic corrected; output buffer capped at 2 MB
- **development/analyst-job-manager**: concurrent `saveJobState` race serialized per job-id; task ID collision on same millisecond fixed

## [1.0.7] - 2026-04-17

### Fixed

- **license**: `parseKey` now correctly validates `UNVN-PRO-*` / `UNVN-TEAM-*` keys — 1.0.6 was published with a stale build that still rejected these keys as malformed (`TMR` prefix check)
- **license**: gate message now shows correct command `uneven-ai license activate` — 1.0.6 stale build showed `uneven license activate`

## [1.0.6] - 2026-04-16

### Fixed

- **askf**: parser no longer creates spurious `output.*` files from unlabelled code blocks in LLM explanation sections (Pattern 3)
- **askf**: `parseFilesWithId` now tolerates optional whitespace/blank line between filename and opening code fence — silent parse failures eliminated
- **license**: `parseKey` no longer rejects all `UNVN-PRO-*` / `UNVN-TEAM-*` keys as malformed (wrong prefix `TMR` → `UNVN`)
- **license**: gate message now shows correct command `uneven-ai license activate` (was missing `-ai`)
- **analyze**: `--package-exe` flag now works in batch mode — packager was instantiated but never called
- **packager**: replaced `execFile` + `stdio: inherit` (unsupported) with `spawn` for real-time pkg output streaming
- **dashboard**: chart gridline colors corrected for light theme (`#1e293b` → `#e2e8f0`)
- **engine**: `readExcelFile` row cap guard moved before `rows.push` (was accumulating rows past the 500-row limit)
- **engine**: formula cells now resolved via `v.result` instead of rendering as `[object Object]`
- **engine**: `.xls` (legacy BIFF format) now detected early with a clear warning instead of silently returning empty string

### Added

- **test**: unit tests for `isValidTarget` — IPv4, CIDR, hostname, wildcard, localhost, edge cases (12 tests)
- **test**: unit tests for `parseFiles` and `parseFilesWithId` — all patterns including spurious-file regression (11 tests)

## [1.0.5] - 2026-04-16

### Fixed

- **config**: `uneven.config.ts` changes now applied automatically on every command — no longer requires manual edit of `.uneven/config.json` or re-running `uneven init`
- **init**: default Gemini model changed from `gemini-2.0-flash` to `gemini-2.5-flash`
- **index-planner**: added `gemini-2.5-flash` pricing entry to cost estimation table

## [1.0.4] - 2026-04-16

### Fixed

- **providers**: Gemini API endpoint corrected from `/v1/` to `/v1beta/` for full model compatibility
- **providers**: Ollama default model corrected from `llama-4` (non-existent) to `llama4`

### Changed

- **README**: update architecture section reference from `v0.12.6` to `v1.0.0`
- **package-lock.json**: regenerated| `feat` | `release` | Lançamento da v1.1.4 (Final Stable) |

---

## Sessão — 2026-04-24 · v1.1.5: Hotfix de Setup (Safe Init)
Reversão da lógica destrutiva de limpeza no comando `init` para evitar conflitos de permissão e falhas de instalação em ambientes restritos.
- **Safe Init**: O comando agora preserva configurações e diretórios existentes, agindo apenas onde necessário.
- **Permission Fix**: Eliminação de operações `fs.rm` que causavam bloqueios de I/O em alguns sistemas.

| Tipo | Atividade | Descrição |
| :--- | :--- | :--- |
| `fix` | `init` | Restauração de setup não-destrutivo e seguro |
| `feat` | `release` | Hotfix v1.1.5 |
at `0.12.6`)

## [1.0.3] - 2026-04-15

### Changed

- **README**: remove contributing section — repository is private

## [1.0.2] - 2026-04-15

### Fixed

- **CI (release)**: rename prebuild assets to unique filenames (`linux-x64.node`, etc.) before
  uploading to GitHub Release — previously all 5 platforms shared the name `uneven_core.node`,
  causing asset upload to fail with HTTP 404

## [1.0.1] - 2026-04-15

### Fixed

- **CI (linux-arm64)**: install `g++-aarch64-linux-gnu` and set `CXX_aarch64_unknown_linux_gnu`
  so crates with C++ build scripts (e.g. `esaxx-rs`) cross-compile correctly on the release pipeline

## [1.0.0] - 2026-04-15

### Added — Test suite

- **`__tests__/engine.test.ts`**: 26 unit tests covering the orchestration engine — `init()`,
  `stop()`, `handleErrorFix()` (anti-loop, deduplication, file queue), `_runFixForError()`
  (confidence gate, autoFix on/off paths, fix-applied/fix-reverted/fix-pending events)
- **`crates/.../error_parser.rs`**: 9 Rust unit tests — TypeScript, absolute path, relative,
  nested path formats; edge cases: empty string, missing column, plain URL rejection
- **`crates/.../retrieval/mod.rs`**: 11 Rust unit tests — `cosine_sim` (identical, orthogonal,
  opposite, mismatched lengths, empty vectors) and `assemble_context` (empty, single, multiple
  documents, score formatting, header presence)
- **`crates/.../providers.rs`**: 4 Rust unit tests — all valid providers, case-insensitive
  parsing, unknown provider error, error message content
- **`crates/.../embeddings.rs`**: 7 pure Rust unit tests — `normalize_l2` (unit vector, scaling,
  zero-vector error), `hash_embed` (dimension, normalization, determinism, distinctness), `fnv3`
  (determinism, collision); existing 4 model-dependent tests fixed (`#[ignore]` + multi-thread
  runtime to prevent `block_in_place` panic)
- **Total**: 135 TypeScript tests (7 suites) + 34 Rust tests (0 failures, 4 ignored)

### Added — CI workflow

- **`.github/workflows/ci.yml`**: `cargo test --lib` added to `build-rust` job — pure Rust
  unit tests run on every push to `main`; Jest job decoupled from full Rust build (bridge
  mocked, binary not required); `cargo clippy -D warnings` catches lint regressions before
  they reach a release tag

### Fixed — ESM test infrastructure

- **`jest.config.cjs`**: bridge `moduleNameMapper` patterns reordered before the `.js` stripper
  so patterns match `bridge.js` before the extension is stripped
- **`__tests__/engine.test.ts`**: uses `jest.unstable_mockModule()` + explicit
  `import { jest } from '@jest/globals'` — required for `--experimental-vm-modules` where
  `jest` is not injected as a bare global into ESM module scope
- **`__tests__/error-parser.test.ts`**: Python traceback fixture corrected — added the
  intermediate executing-code line that the parser requires to identify the last stack frame

## [0.16.0] - 2026-04-15

### Added — NAPI streaming inference

- **`inference.rs`**: `generate_streaming` and `run_inference_streaming` — token-by-token generation
  via callback, enabling progressive output instead of waiting for full completion
- **`llm/orchestrator.rs`**: `infer_with_params_streaming` wires the streaming path end-to-end
- **`api/napi_api.rs`**: `llm_infer_stream` exposed as a NAPI function with `ThreadsafeFunction`
  callback — each generated token is delivered to JS as it is produced
- **`bridge.ts`**: `llmInferStream` bound and wired into `engine.ts ask()` — `ask` and `askf`
  commands now stream tokens inline instead of printing the full response at once
- **`test/mocks`**: `llmInferStream` mock added to simulate token-by-token delivery in unit tests

### Added — pendingDiffs protocol (autoFix=false agent coexistence)

- **`engine.ts`**: P2 pendingDiffs protocol — when `autoFix: false`, detected errors are posted
  to `session.json` as `PendingDiff` records instead of being discarded; agents or the human
  user can inspect, apply, or reject them independently
- **`cli/commands/diff.ts`**: `uneven-ai diff` command — lists pending diffs, applies them with
  `--apply <id>` (checksum guard + snapshot + atomic write), rejects with `--reject <id>`,
  clears resolved history with `--clear`

### Fixed — autoFix pipeline: P0+P1 correctness

- **`engine.ts`**: P0 — concurrent writes to the same file during autoFix now serialized via a
  per-file `fileFixQueue` — without this, two rapid errors in the same file could produce an
  interleaved, corrupted result
- **`engine.ts`**: P0 — session lock (`acquireLock`/`releaseLock`) now wraps file writes;
  snapshot taken before every write to support `uneven-ai restore`
- **`engine.ts`**: P0 — checksum guard before `applyFix` aborts if the file was modified between
  error detection and LLM response, preventing the fix from being applied to the wrong version
- **`engine.ts`**: P0 — removed early return at the `autoFix=false` branch that silently discarded
  all errors before the session check at line 1183 — `autoFix: false` via `uneven.config.ts`
  was completely inoperative; `pendingDiff` was never posted
- **`engine.ts`**: P1 — 500 ms debounce applied to both autoFix paths; debounce timers cleared on
  `stop()` to prevent a queued fix from firing after the engine shuts down
- **`engine.ts`**: P1 — `promptUserConfirmFix` returns `false` in non-TTY environments instead
  of hanging indefinitely waiting for stdin
- **`engine.ts`**: P1 — `autoFix`/`confirmBeforeFix` shadow variables removed from error handler
  closure — outer config values now used directly, eliminating stale-capture bug

### Fixed — malware scanner

- **`supply-chain-auditor.ts`**: `levenshtein` call in typosquatting explanation replaced with
  `editDistance` — function was removed in v0.14.0 audit but one call site in the explanation
  string was missed; caused `ReferenceError` at runtime on any project with typosquat findings
- **`supply-chain-auditor.ts`**: unified `POPULAR_PACKAGES` list; minimum name length guard added
  to typosquatting — single-character package names produced false positives with edit distance 1
- **`malware-scanner.ts`**: `isTestFile` now detects `__tests__` directories via `filePath`
  instead of `basename` — test files in `__tests__/` were scanned and flagged as production code
- **`malware-scanner.ts`**: `computeRiskLevel` now applies confidence threshold symmetrically —
  low-confidence findings no longer elevated the risk level
- **`malware-scanner.ts`**: `dependenciesScanned` count fixed — was always `0` due to wrong
  variable reference
- **`malware-scanner.ts`**: LLM evaluation prompt now sanitizes the filename — special characters
  in filenames could break the prompt structure
- **`malware-scanner.ts`**: non-global regex in `STATIC_RULES` loop now uses `break` guard
  correctly; `peerDependencies` added to dependency scan scope; dead `dirs` variable removed
- **`scan.ts`**: `spinner.stop()` → `spinner.succeed('Scan complete')` — spinner disappeared
  silently with no completion feedback
- **`scan.ts`**: logger flushed before `process.exit` to prevent log truncation on findings
- **`cli/index.ts`**: `--report` help text corrected from `.uneven/` to `uneven-reports/`

### Fixed — pentest & security analyzer

- **`security-analyzer.ts`**: `COOKIE_CALL` regex rewritten — options object now captured via
  `\{[^}]*\}` group; `\b` word boundary added before `cookie` to eliminate false positives on
  `_cookie(` and `cookieStore(`
- **`security-analyzer.ts`**: CORS wildcard+credentials check scoped to each individual
  `cors({...})` block — file-wide flag produced false positives when the two options appeared in
  separate route configs that never co-existed
- **`security-analyzer.ts`**: `scanHeaders` now respects the `dirs` parameter — was ignoring it
  and always scanning the full project
- **`security-analyzer.ts`**: `execSync npm audit` replaced with async `execFile` — blocked the
  event loop and could not be interrupted
- **`security-analyzer.ts`**: `walkDir` skip list hoisted to module-level `Set` — was
  reallocated on every call
- **`security-analyzer.ts`**: `let mc` redeclaration in CORS_BLOCK loop renamed to `mc2` —
  TypeScript block-scoped variable redeclaration error in same function scope
- **`pentest-context.ts`**: `matchesTarget` CIDR — captures 4th octet correctly; supports
  `/25`–`/32` prefixes that were silently rejected
- **`pentest-context.ts`**: credential spray blocked at `>= 50` threads — the `100–499` range
  was missing from the guard condition
- **`pentest.ts`**: IP octet range and CIDR prefix validated on input — malformed targets
  previously reached the scanner unchecked

### Fixed — terminal watch engine

- **`process-lock.ts`**: lock file created with `O_EXCL` (atomic) — race between two processes
  reading "no lock exists" then both writing was possible with the old check-then-create pattern;
  signal handler accumulation on repeated `watch` invocations fixed with `once`
- **`watcher.ts`**: zombie process prevention — `ProcessWatcher` now guards against
  restart-after-stop; output buffers capped at 100 KB to prevent OOM on runaway processes;
  `restartCount` reset after stability window; spawn errors surfaced; `watch()` resolves on max
  restarts
- **`file-watcher.ts`**: `running = true` set before the `ready` event so that `stop()` called
  immediately after `start()` actually closes the watcher

### Fixed — data analyst & document intel

- **`db-loader.ts`**: `LOAD_LIMIT` applied to `loadDocuments` queries — unbounded `SELECT *` on
  large tables caused OOM on the analyst pipeline
- **`data-analyst.ts`**: MongoDB schema inference now samples multiple documents — single-document
  sample missed fields that only appeared in a subset of records
- **`data-analyst.ts`**: `generateSQL` prompt truncated when schema exceeds context budget —
  oversized schema was silently overflowing LLM context
- **`data-analyst.ts`**: MongoDB `executeQuery` now parses and applies `WHERE` clause filters —
  all MongoDB queries previously returned unfiltered collections
- **`data-analyst.ts`**: SQLite `PRAGMA table_info` now quotes the table name — table names with
  spaces or reserved words caused a parse error
- **`data-security-context.ts`**: wildcard column regex fixed to match single-segment names;
  arbitrary `slice(0, 30)` cap on `buildPromptRules` removed — truncated security rules were
  silently omitted from the LLM prompt
- **`analyze.ts`**: requires explicit `A` or `APPROVE` to execute destructive queries — any
  truthy input previously approved execution
- **`analyze.ts`**: `dbUrl` (not `dbType`) now passed to `jobManager.createJob` — job was
  created without a connection string, failing on first query
- **`report-packager.ts`**: `execSync` replaced with async `execFile`; shell injection in
  filename argument fixed
- **`dashboard-generator.ts`**: `</script>` escaped in JSON chart data to prevent XSS when
  the dashboard HTML is served from a web origin
- **`excel-exporter.ts`**: `require()` replaced with ESM-compatible dynamic `import()`
- **`llm-security-reviewer.ts`**: operator precedence bug in `sanitizeContext` fixed — `&&`
  was binding tighter than intended, causing some sensitive patterns to pass through unsanitized

### Fixed — logger & error parser

- **`logger/index.ts`**: `getTimestamp` now includes date (`YYYY-MM-DD HH:MM:SS`) — log entries
  across midnight were indistinguishable
- **`logger/index.ts`**: triple backticks in `fixApplied` before/after code blocks escaped —
  unescaped backticks broke the Markdown fence in the log file
- **`logger/index.ts`**: `securityFinding` uses `line != null` — `line === 0` (first line of
  file) was treated as falsy and silently omitted from the log
- **`error-parser.ts`**: `parsePythonErrors` reads exception message at the frame boundary, not
  at `i+1` — off-by-one caused the wrong line to be stored as the error message
- **`error-parser.ts`**: `getContext` now receives the terminal output index, not the source line
  number — produced context slices from the wrong position
- **`error-parser.ts`**: `parseRustErrors` searches forward for the `-->` location line —
  fixed cases where the location appeared after the error header, not on the same line
- **`error-parser.ts`**: `FATAL` mapped to `'error'` and `WARN` mapped to `'warning'` in
  `parseGenericErrors` — unrecognized severity strings were dropped

### Fixed — CLI commands

- **`init.ts`**: downloads are now atomic (`.tmp` + rename) — SIGINT during download no longer
  leaves a partial file that `fileExists()` accepts as valid on the next run
- **`init.ts`**: `readline` wrapped in `try/finally` — `rl.close()` now called on Ctrl+C or
  EOF during the provider prompt, preventing stdin from being held open
- **`init.ts`**: `fetch()` for model downloads now carries `AbortSignal.timeout(20 * 60 * 1000)`
  — stalled CDN connections previously blocked `init` indefinitely
- **`init.ts`**: `hardware.gpu.recommendation.gpuLayers` now propagated to both `saveConfig` and
  `makeConfig` — was detected and displayed but always overridden by the hardcoded `gpuLayers: 32`
  default; users with 1.5 GB VRAM received 32 layers and crashed at runtime
- **`init.ts`**: invalid provider choice now emits a `warn()` instead of silently defaulting —
  a typo during setup produced no diagnostic
- **`init.ts`**: `uneven-reports/` added to `.gitignore` — HTML pentest and Excel data reports
  were committed accidentally
- **`init.ts`**: `res.body` null check moved inside the try block so the partial file descriptor
  is cleaned up before throwing
- **`init.ts`**: `threads` value now derived from `Math.max(2, Math.min(hardware.cpuCores, 8))`
  in both `saveConfig` and `makeConfig` — the two had diverged (2 vs 4)
- **`init.ts`**: `defaultModelFor(provider)` pure function extracted — duplicate model-resolution
  logic in `makeConfig` and `initCommand` had already diverged
- **`init.ts`**: `User-Agent` updated to use the runtime `version` string instead of hardcoded
  `'uneven-ai-init/0.7.1'`
- **`ask.ts`**: `guardian` hoisted before the `try` block so `catch` can call `guardian?.stop()`
  — was a block-scope error; `ResourceGuardian` was never stopped on exception
- **`ask.ts`**: `config.brain.temperature` forwarded to local inference — `ask` was always using
  the default temperature regardless of user config
- **`ask.ts`**: tokens now streamed inline — each token was printed on a new line instead of
  appended to the current line
- **`askf.ts`**: `readline` interface closed in `finally` to prevent stdin leak on any exit path
- **`askf.ts`**: `path.sep` added to `startsWith` check — sibling directories sharing a prefix
  (e.g. `src/foo-extra`) could bypass the allowed-paths guard
- **`askf.ts`**: files and summary parsed from the sanitized answer, not from raw tokens —
  streaming tokens arrived in arbitrary chunks, making per-token parsing unreliable
- **`index.ts` (CLI)**: lock released on all exit paths including early-return guards — a failed
  file read during indexing left the lock held indefinitely
- **`engine.ts` (index)**: Phase 2B file guards aligned with Phase 2A — binary, ignored-dir, and
  extension checks were missing from the Phase 2B worker path
- **`diff.ts`**: missing `/**` JSDoc opening delimiter added — absence generated dozens of tsc
  parse errors that saturated compiler output and masked all other real type errors in the project
- **`engine.ts`**: `'uneven-ai diff reject'` hint corrected to `'uneven-ai diff --reject'`

### Fixed — CI/CD pipeline

- **`.github/workflows`**: `timeout-minutes` added to all jobs — runaway builds previously
  consumed the full Actions quota before timing out
- **`.github/workflows`**: `uneven-ci` job dependency chain fixed; `lint-and-typecheck` added as
  a required dependency of the test job
- **`local-release.sh`**: `wait_actions` fixed for `workflow_dispatch` trigger — was polling the
  wrong event type and timing out immediately
- **`build/release`**: tag-push trigger restored for darwin and win32 Actions builds
- **`postinstall.ts`**: banner box alignment corrected

### Changed

- Version bumped to `0.16.0` across `package.json`, `Cargo.toml`, `napi_api.rs`, `session.ts`,
  and `sbom-generator.ts`
- Local release pipeline added (`build/local-release.sh`) — Linux builds locally, darwin and
  win32 via Actions; win32 cross-compile via MinGW auto-detected if `gcc-mingw` is installed

## [0.15.0] - 2026-04-14

### Performance — local LLM inference

- **`llm/inference.rs`**: replaced manual `argmax` with `candle_transformers::generation::LogitsProcessor` —
  supports greedy decode (temperature=0, default) and temperature/top-p sampling
- **`llm/inference.rs`**: added repetition penalty via `apply_repeat_penalty` on a 64-token sliding
  window of generated output (default penalty=1.1) — prevents degenerate loops that previously
  exhausted the full `max_tokens` budget without hitting EOS
- **`llm/inference.rs`**: context-window guard — prompts exceeding `max_context - max_tokens` tokens
  are truncated from the left (most-recent context kept); default `max_context=2048`
- **`llm/inference.rs`**: `tokens.reserve(max_tokens)` — eliminates O(log n) Vec reallocations
  during generation
- **`api/napi_api.rs`**: `llm_infer` now accepts `temperature: f64` and `repeat_penalty: f64`;
  TypeScript callers default to `temperature=0.0, repeatPenalty=1.1`

## [0.14.1] - 2026-04-14

### Fixed — CI

- **`retrieval/mod.rs`**: gated `STORE_USEARCH` and `STORE_META_BIN` constants behind
  `#[cfg(feature = "vector-store")]` — dead-code lint under `-D warnings` blocked CI on v0.14.0
- **`retrieval/mod.rs`**: replaced `&*store` with `&store` in two `write_binary` call sites —
  `clippy::explicit_auto_deref` lint also blocked CI
- **`error-parser.ts`**: added `startsWith('PHP ')` fallback to `hasPHP` detection —
  single-line PHP output was never parsed, causing 3 test failures in CI on v0.13.0

## [0.14.0] - 2026-04-14

### Performance — Rust core: vector store dual-backend

- **`retrieval/mod.rs`**: replaced JSON persistence with bincode in the default HashMap backend —
  10–50× smaller files, ~100× faster parse; atomic writes (tmp + rename) prevent corruption
- **`retrieval/mod.rs`**: added optional HNSW backend (`--features vector-store`) via usearch —
  O(log n) ANN search replaces O(n) brute-force; `StoreMeta` serialized with bincode; automatic
  migration chain from legacy JSON and old bincode formats
- **`retrieval/mod.rs`**: `VECTOR_STORE` changed from `Mutex` to `RwLock` — concurrent readers
  no longer serialize during search
- **`embeddings.rs`**: `BERT` changed from `Mutex` to `RwLock` — rayon threads in `batch_embed`
  can now hold concurrent read locks, restoring true parallelism
- **`api/napi_api.rs`**: `retrieval_search` returns `NapiRetrievalResult` struct instead of JSON
  string — eliminates per-query `serde_json::to_string` + JS `JSON.parse` round-trip

### Performance — TypeScript: indexing pipeline

- **`engine.ts`**: `processFile` reads each file once (raw `Buffer`), reuses it for binary
  detection, SHA-256 hash, and content extraction — eliminates two extra syscalls per file
- **`engine.ts`**: `getFilesRecursive` parallelized with `Promise.all` — subdirectory traversal
  no longer sequential
- **`engine.ts`**: Phase 2B and Phase 2D use the same worker-pool pattern as Phase 2A —
  `INDEX_CONCURRENCY` scales to `os.cpus().length`
- **`engine.ts`**: `watch()` busy-wait loop replaced with event-driven `stopSignal` promise —
  eliminates 1 s polling
- **`engine.ts`**: `IGNORED_DIRS`, `SUPPORTED_EXTENSIONS`, `IGNORED_EXTENSIONS` converted to
  module-level `Set` — O(1) lookup instead of O(n) array scan per file
- **`incremental-index.ts`**: all sync I/O eliminated; `status`, `diff`, `needsIndexing` are
  fully async; `diff` uses `Promise.all` for concurrent hash checks; `markIndexedWithHash`
  avoids re-reading a file whose buffer was already read by the caller
- **`supply-chain-auditor.ts`**: `editDistance` rewritten with two rolling rows (O(n) memory)
  and early-exit when row minimum exceeds `maxDist` — O(m×n) → sublinear for typosquatting
  threshold of 2

## [0.13.0] - 2026-04-13

### Fixed — Rust core: critical bug fixes and anti-pattern removal

- **`embeddings.rs`**: extracted `embed_sync()` synchronous path for `batch_embed` — previous
  implementation called `futures::executor::block_on(generate_embedding())` inside rayon threads,
  which panicked at runtime because `block_in_place` is only valid on tokio worker threads
- **`retrieval/mod.rs`**: removed redundant `Arc` wrapping from `VECTOR_STORE` static —
  `Lazy<RwLock<T>>` is already thread-safe; the extra `Arc` added indirection with no benefit
- **`inference.rs`**: removed dead `prompt_len` variable and its `let _ = prompt_len` suppressor
- **`retrieval/mod.rs`**: removed unused `use std::sync::Arc` import

### Fixed — Rust core: performance and correctness (previous session)

- **`napi_api.rs`**: replaced hardcoded `"0.12.3"` with `UNEVEN_VERSION` constant in `get_engine_info()`
- **`napi_api.rs`**: removed `spawn_blocking(|| block_on(...))` anti-pattern — CPU-bound LLM
  inference now offloaded correctly via `block_in_place` inside `run_inference`
- **`inference.rs`**: removed `Arc` from `LLAMA`, `LOADED_MODEL`, `INTERRUPT` statics
- **`embeddings.rs`**: wrapped BERT forward pass in `block_in_place` to prevent tokio starvation
- **`retrieval/mod.rs`**: replaced O(n log n) full sort with O(n log k) min-heap in `semantic_search`;
  replaced blocking `std::fs` calls with async `tokio::fs` in `init_store` and `persist_store`
- **`error_parser.rs`**: regex compiled once via `Lazy<Regex>` instead of on every parse call
- **`file_loader.rs`**: `is_supported()` changed from O(n) `Vec::contains` to O(1) `HashSet` lookup
  via `OnceLock`; `FileLoader` struct is now zero-sized
- **`port_scanner.rs`**: `scan_common_ports` changed from sequential to concurrent via `join_all`,
  reducing wall time from ~9 s to ~1 s

### Fixed — TypeScript layer: bugs and anti-patterns

- **`git-manager.ts`**: `undoLastFix` and `listFixCommits` were grepping for `"uneven-ai"` but
  `commitFix` generates messages with `"fix(uneven):"` — both commands were completely broken
- **`bridge.ts`**: duplicate `process.env.UNEVEN_DEBUG` condition `|| process.env.UNEVEN_DEBUG`
  removed (always evaluated the same branch)
- **`logger/index.ts`**: duplicate `=== '1' || === '1'` condition collapsed to single check
- **`process-watcher.ts`**: `command.split(' ')` replaced with quoted-arg-aware regex tokenizer —
  commands with quoted arguments (e.g. `npm run "my script"`) were split incorrectly
- **`process-lock.ts`**: `process.on()` replaced with `process.once()` for exit/signal handlers —
  repeated `watch` invocations were stacking handlers
- **`web-scraper.ts`**: `fetchMultiple` changed from sequential loop to `Promise.allSettled`;
  `urlToId` hash upgraded from charCode-sum to FNV-1a 32-bit for better distribution
- **`chunker.ts`**: removed O(n) `.includes()` guard before `.push()` in line-tracking loop —
  was causing chunks to silently drop line references on large files
- **`error-parser.ts`**: added O(1) pre-scan dispatch — only relevant parsers run per error,
  skipping all 9 parsers for every line
- **`data-analyst.ts`**: fixed `raw.length + (raw.length - raw.length)` expression (always
  equalled `raw.length`) — total-before-filter count now captured correctly before filtering
- **`sbom-generator.ts`**: stale fallback version updated
- **`incremental-index.ts`**: `save()` now wraps atomic write in try/catch and unlinks temp file
  on failure to prevent corrupted state replacing valid state on next run
- **`cli/index.ts`**: dead third condition in help display logic removed

### Fixed — TypeScript layer: second-pass subtle bugs

- **`supply-chain-auditor.ts`**: phantom-package inner condition `const inLock` was always false
  (outer `if` already guaranteed `!lockedPkgs.has(pkgName)`) — dead code removed
- **`supply-chain-auditor.ts`**: typosquatting loop used `break` on exact name match, exiting
  the entire popular-packages loop instead of skipping only that entry — changed to `continue`
- **`engine.ts`**: shadowed `threads` re-declaration inside `if (isNativeAvailable())` removed;
  fixed indentation that made `initLlmEngine` appear outside the conditional block

### Changed

- Version bumped to `0.13.0` across `package.json`, `Cargo.toml`, `napi_api.rs`,
  `session.ts`, and `sbom-generator.ts`

## [0.12.0] - 2026-04-12

### Changed — Pure Clean Architecture Refactor (Stages 1 & 2)

- **TypeScript Layer**: Complete migration of `src/core` to a modular 3-layer architecture:
    - `src/domain/`: Core business entities, index planners, and safety guards.
    - `src/application/`: High-level orchestration, security analyzers, and fix engines.
    - `src/infrastructure/`: Low-level adapters including IO, DB loaders, terminal watchers, and the native bridge.
- **Rust Core**: Refactored `crates/uneven-core` into modular domain, application, and infrastructure modules for enhanced maintainability and feature isolation.
- **Barrels**: Introduced central entry points (`index.ts`) for each layer to standardize internal imports and discourage deep module coupling.
- **Licensing**: Refined the license gate with refined error reporting and direct exports for core verification functions.
- **Stability**: Fixed over 100 broken relative imports across the CLI and core engine caused by the directory structure changes.
- **Build System**: Standardized on absolute paths for internal tooling stability across various Node.js environments.

## [0.7.3] - 2026-04-09

### Fixed — complete audit of unreviewed modules

- `external-providers.ts`: all four provider fetch calls (OpenAI, Claude, Gemini, Ollama)
  now carry `AbortSignal.timeout(60_000)` — previously a slow or unresponsive API would
  hang indefinitely and hold the process lock
- `git-manager.ts`: `execAsync` now passes `{ timeout: 30_000 }` — local git operations
  should never take more than 30 s; previously a credential prompt or locked index could
  freeze the auto-fix pipeline
- `pentest-security-context.ts`: `durationSecs` default was `8` (eight seconds) instead
  of `8 * 3600` (eight hours) — a direct programmatic call to `declareScope()` without
  an explicit duration would expire the scope almost instantly
- `test-runner.ts`: SIGTERM is now followed by a 5 s SIGKILL fallback — Node.js or JVM
  processes that install signal handlers can ignore SIGTERM; without SIGKILL the spawned
  test process would outlive Uneven AI and block the terminal

## [0.7.2] - 2026-04-09

### Added — timeout kill switch for all blocking operations

- New `src/core/timeout.ts`: `withTimeout<T>(promise, ms, label)` + `TimeoutError`
  — races any promise against a deadline; clears the timer in both success and failure
  paths to prevent event-loop retention
- `bridge.ts`: `llmEmbed` (2 min), `llmInfer` (3 min), `externalInfer` (1 min) — every
  LLM consumer inherits timeouts automatically without any per-call change
- `data-analyst.ts`: DB connect (10 s), schema introspection (20 s), query execution
  (30 s) — `TimeoutError` on query surfaces a user-facing hint to add a `LIMIT` clause
- `web-scraper.ts`: added `headersTimeout: 15 s` + `bodyTimeout: 30 s` to the undici
  `Agent` — `connectTimeout` already existed but body timeout was missing, allowing
  slow-loris-style hangs

## [0.7.1] - 2026-04-09

### Added — process lock, atomic writes, throttled scanners

- New `src/core/process-lock.ts`: exclusive `.uneven/uneven-ai.lock` with stale PID
  detection via `process.kill(pid, 0)` — prevents two Uneven AI instances from corrupting
  the vector store or index state concurrently; all five heavy commands acquire the lock
  (`watch`, `index`, `pentest`, `analyze`, `ci`) and release it in `finally`
- `incremental-index.ts`: `save()` now writes to `.uneven/index-state.tmp` then
  renames atomically — a crash during save no longer corrupts the previous state
- `logger/index.ts`: replaced unused `buffer: string[]` with a `writeQueue: Promise<void>`
  — all `appendFile` calls are serialized through a promise chain, eliminating concurrent
  write races within the same process
- `engine.ts`: replaced two `Promise.all([8 scan tasks])` in `pentest()` with a throttled
  worker queue (`SCAN_CONCURRENCY = 3`) — previously 8 filesystem walkers ran simultaneously
  and could saturate I/O on slower machines

## [0.7.0] - 2026-04-08

### Added — license system and Pro feature gates

- `src/core/license/` module: machine fingerprinting (SHA256 of platform+arch+hostname+homedir), encrypted local cache (`~/.config/uneven-ai/license.json`), offline grace period (7-day extension, max 30 days cumulative)
- License key format: `TMR-{TIER}-{payload_b64}-{hmac}` — payload parsed locally, HMAC verified server-side
- `uneven-ai license activate <key>` — activates key against `api.uneven.dev`, caches result locally
- `uneven-ai license status` — shows tier, seats, expiry, machine ID
- `uneven-ai license deactivate` — removes machine from seat count
- Pro feature gate (`src/core/license/gate.ts`): commands blocked on free tier show upgrade message and exit cleanly

### Changed — Pro-only commands now require license

- `uneven-ai analyze` — requires Pro
- `uneven-ai ci` — requires Pro
- `uneven-ai askf` — requires Pro
- `uneven-ai pentest -m active` — requires Pro (static mode remains free)

## [0.6.0] - 2026-04-08

### Fixed — local model stays loaded after switching to API provider

- `llm/mod.rs`: `init()` now accepts the `provider` string — local LLaMA model is only loaded when `provider == "local"`; API providers (claude, openai, gemini, ollama) skip model loading entirely
- `llm/inference.rs`: added `unload_model()` to drop the LLaMA state from the `LLAMA` mutex and free RAM immediately
- `lib.rs`: `init_llm_engine(provider)` passes provider to the Rust init chain; new `unload_local_model()` NAPI binding exposed for TypeScript
- `bridge.ts`: `initLlmEngine(provider)` and `unloadLocalModel()` updated/added
- `engine.ts`: provider resolved before `initLlmEngine()` call so it is never passed as `undefined`

### Added — runtime config loader and provider-aware CLI

- `src/core/config-loader.ts`: new module reads `.uneven/config.json` (written by `uneven-ai init`) so all commands use the provider actually configured by the user — no more hardcoded `provider: 'local'` across commands
- `init.ts`: writes `.uneven/config.json` at the end of init so the selected provider is persisted for future commands
- All commands (`start`, `watch`, `ask`, `askf`, `index`, `pentest`, `ci`) now call `loadConfig()` instead of hardcoding the brain
- CLI displays the active brain after startup: `brain: claude (claude-sonnet-4-6) — API` or `brain: local (llama-3.2-1b-q8) — offline`
- `EventType` includes new `brain-ready` event emitted after engine init

### Fixed — clarified RAM requirements in README

- Requirements section now split into three tables: API providers (~256 MB), Ollama (~512 MB), local brain (4-8 GB)
- New section "Running Uneven AI via API with minimal memory" with optimized config example and tips (`chunkSize`, `maxChunks`, smaller models)

## [0.5.9] - 2026-04-08

### Added — `uneven-ai askf` command

- New `askf <question>` command: same as `ask` but instructs the LLM to declare files using `FILE:/END_FILE` markers and writes them to the current directory
- Parser handles 3 formats: explicit `FILE:/END_FILE` blocks, plain markers, and markdown code blocks with filename hint in first-line comment
- Creates parent directories automatically (`mkdir -p`)
- Prints each created file with size in KB
- Falls back to plain answer with a hint if LLM produces no file blocks

## [0.5.8] - 2026-04-08

### Fixed — LLaMA inference argmax crash

- `inference.rs`: candle's `quantized_llama::forward` returns `[1, vocab]` (not `[1, seq, vocab]`) — code now handles both shapes before calling argmax, preventing "dimension index -1 out of range for shape []" panic

## [0.5.7] - 2026-04-08

### Fixed — LLaMA tokenizer download

- Changed tokenizer.json source from `meta-llama` (gated, requires HuggingFace login) to `unsloth/Llama-3.2-1B-Instruct` (public, no auth required)
- `inference.rs`: EOS token now read from GGUF metadata (`tokenizer.ggml.eos_token_id`) before falling back to vocab lookup — correct default for LLaMA 3 is `128009` (`<|eot_id|>`)

## [0.5.6] - 2026-04-08

### Fixed — Release pipeline

- `continue-on-error: true` on GitHub Release step — asset deletion error on re-run was blocking `npm publish`
- `fail_on_unmatched_files: false` added to prevent false failures on asset upload

## [0.5.5] - 2026-04-08

### Fixed — linux-arm64 cross-compilation

- Install `g++-aarch64-linux-gnu` in CI alongside `gcc-aarch64-linux-gnu` — `esaxx-rs` (C++ dependency of tokenizers) requires the C++ cross-compiler

## [0.5.4] - 2026-04-08

### Fixed — Windows CI

- `npm test` script changed from `node_modules/.bin/jest` (bash shebang, breaks PowerShell) to `node_modules/jest/bin/jest.js` (cross-platform Node.js entry point)

## [0.5.3] - 2026-04-08

### Fixed — Clippy

- `embeddings.rs`: removed unnecessary `m as u32` cast (already `u32`)
- `error.rs`: removed `to_string()` method that shadowed `Display` implementation (provided by `thiserror`)

## [0.5.2] - 2026-04-08

### Fixed — CI/CD Pipeline

- `package-lock.json` removed from `.gitignore` and committed — `npm ci` was failing on macOS/Windows runners with "lock file not found"
- GitHub Actions updated to Node.js 24 (avoids Node.js 20 deprecation warnings)
- `npm ci` → `npm install` in release workflow for compatibility

## [0.5.1] - 2026-04-08

### Added — Real LLM Engine

- **Candle BERT embeddings**: real 384-dim sentence vectors via `all-MiniLM-L6-v2` (safetensors + tokenizer.json). Replaces hash-based fallback for semantic search.
- **Candle GGUF inference**: real LLaMA 3.2 1B Q8 local inference via `candle-transformers`. Greedy decoding, configurable max-tokens.
- **Real terminal watcher**: `tokio::process::Command` spawns dev server, `tokio::sync::mpsc` channel streams stdout + stderr concurrently to TypeScript layer.
- **Real external providers**: `OpenAI`, `Anthropic Claude`, `Google Gemini`, `Ollama` — all use native Node.js `fetch` (no extra SDK packages required).
- **Interactive `uneven-ai init`**: prompts provider choice (1–5), downloads embedding model (~90 MB) and optionally LLaMA 3.2 1B Q8 (~1.4 GB) from HuggingFace with progress bars.
- **`uneven-ai watch <cmd>`**: command now fully wired to Uneven AI engine (was a no-op stub).
- `FileWatcher` class exported from public API for programmatic use.

### Fixed

- `retrieval/mod.rs`: removed hardcoded 1024-dim assertion — now compatible with 384-dim BERT vectors.
- `port_scanner.rs`: removed unused `ToSocketAddrs` import.
- `error_parser.rs`: removed unused `Uneven AIError` import.
- Brain providers table in docs: removed incorrect `openai`/`@anthropic-ai/sdk`/`@google/generative-ai` package requirements.

### Changed

- Embedding dimension: 1024 → **384** (all-MiniLM-L6-v2 output size).
- Local model name: `llama3.2` → `llama-3.2-1b-q8` (matches GGUF filename).
- Rust `llm` feature enabled by default (`default = ["llm"]`).
- `crate-type = ["cdylib", "rlib"]` — enables both Node.js addon and Rust library builds.

## [0.5.0] - 2026-04-08

### Breaking — Full ESM Migration

- Package is now native ESM (`"type": "module"` in `package.json`)
- Module resolution upgraded to `NodeNext` (TypeScript `module + moduleResolution: NodeNext`)
- All relative imports now carry explicit `.js` extensions as required by Node.js ESM
- `dist/` output is now ES module format — drop-in CommonJS compatibility removed
- Consumers must use `import` syntax (or dynamic `import()` for CJS interop)

### Changed — Module Resolution

- `tsconfig.json`: `module` and `moduleResolution` upgraded from `"node"` (deprecated) to `"NodeNext"` — the most modern TypeScript setting available as of 2026
- `src/core/bridge.ts`: uses `createRequire(import.meta.url)` to load `.node` native addon (native addons are inherently CommonJS and cannot be dynamically imported)
- `src/cli/index.ts`: replaced `__dirname` with `dirname(fileURLToPath(import.meta.url))` for ESM compatibility

### Changed — Scripts and Config

- `scripts/postinstall.js` → `scripts/postinstall.cjs` (stays CJS; runs before `"type": "module"` kicks in)
- `scripts/copy-prebuild.js` → `scripts/copy-prebuild.cjs` (same reason)
- `jest.config.js` → `jest.config.cjs` with `ts-jest/presets/default-esm` preset
- Test runner: `node --experimental-vm-modules node_modules/.bin/jest`

### Changed — File Watcher

- Replaced `inotify` (Linux-only, unmaintained, broken on Node 25+) with `chokidar` v5
- `src/core/file-watcher.ts`: new `FileWatcher` class — cross-platform, pure TypeScript, no native addons
- New event types added to `EventType`: `file-changed`, `file-added`, `file-removed`

### Changed — Rust Dependencies (all latest as of 2026)

- `napi` 2 → 3, `napi-derive` 2 → 3 (breaking: `Object` lifetime, `create_object()` removed, use `#[napi(object)]` struct)
- `thiserror` 1 → 2
- `similar` 2 → 3
- `candle-core`, `candle-transformers` → 0.10
- `tokenizers` → 0.22
- `reqwest` 0.12 → 0.13
- Rust watcher: cross-platform `shell_command()` helper (`sh -c` on Unix, `cmd /C` on Windows)

### Added — GitHub Actions Release Pipeline

- `.github/workflows/release.yml`: 3-gate pipeline
  - **Gate 1 — test**: runs on ubuntu, macos, windows in parallel
  - **Gate 2 — build-native**: compiles `.node` binaries for 5 targets (linux-x64, linux-arm64, darwin-x64, darwin-arm64, win32-x64) via cross-compilation
  - **Gate 3 — publish**: creates GitHub Release + uploads all binaries + publishes to npm
- `.github/workflows/build.yml`: simplified to `workflow_dispatch` only with artifact verification

### Fixed — Tests

- `data-security-context.test.ts`: bcrypt regex `.{53}` → `.{20,}` (test hash was 45 chars, not exactly 53)
- `pentest-security-context.test.ts`: scope duration unit changed from hours to seconds (`durationSecs`), so `0.0001` = 0.1ms (expires in test window) and `8` = 8s (safe during test run)
- `data-analyst-safe-query.test.ts`: `PRAGMA` now allowed in safe-query regex; lookahead fixed (was lowercase, normalized string is uppercase)

### Documentation

- `llms.txt` + `llms-full.txt`: added dependency policy — always use latest library versions, no legacy packages
- Version tables for both npm and Rust crates pinned to 2026 latest

## [0.4.0] - 2026-04-11

### Added — Data Analyst

- `uneven-ai analyze --db <url>` — interactive REPL for AI-powered data analysis
- Natural language → SQL generation via local LLM (grounded in real schema)
- Human approval gate: `[A]pprove / [E]dit / [S]kip` before every query executes
- Schema introspection for PostgreSQL, MySQL, SQLite, MongoDB
- Excel export (`.xlsx`) with dark blue headers, alternating rows, auto-filter, auto-fit columns
- HTML dashboard generation with Chart.js — auto-detects chart type (line, bar, doughnut, scatter, table)
- Combined workbook: all session queries merged into a single Excel file at session end
- `--package-exe` flag: packages HTML dashboard as a self-contained Windows `.exe` via `pkg`
  - Fully offline — Chart.js inlined from local cache
  - HTTP server on `127.0.0.1` only (not network-accessible)
  - Security headers: `Content-Security-Policy: connect-src 'none'`, `X-Frame-Options: DENY`
  - Targets: win-x64, win-arm64, macos-x64, macos-arm64, linux-x64

### Added — Data Security Context (3-layer defense)

- **Layer 1 — Schema Filter**: removes blocked tables and columns before LLM generates SQL
- **Layer 2 — SQL Audit**: blocks DML/DDL (`INSERT`, `UPDATE`, `DELETE`, `DROP`, `GRANT`, `EXEC`) and references to sensitive tables/columns
- **Layer 3 — Result Masking**: redacts sensitive values by column name patterns and value regex (bcrypt, argon2, JWT, AWS keys, CPF, private keys)
- Default policy blocks 42 column patterns (passwords, tokens, API keys, CPF, CVV, etc.) and 11 table patterns (sessions, oauth_tokens, audit_logs, etc.)
- `SECURITY_BLOCKED` escape hatch — LLM signals when requested data falls outside security policy
- `buildPromptRules()` — injects security constraints into every LLM prompt
- Custom policy extension: `new DataSecurityContext({ blockedColumns: [...], blockedTables: [...] })`
- Exported `isSafeQuery(sql)` — pure function, safe to use anywhere

### Added — Malware Scanner

- `uneven-ai scan` — scans project files and dependencies for malicious code
- Static rule engine: 8 categories with confidence scores
  - `remote-shell` (critical) — `/dev/tcp`, `nc -e`, `bash -i >&`
  - `data-exfiltration` (high) — credential harvesting + network upload patterns
  - `obfuscation` (high) — `eval(atob())`, `eval(Buffer.from(..., 'base64'))`, large `String.fromCharCode` arrays
  - `supply-chain` (critical) — network download in postinstall scripts
  - `credential-theft` (high) — reading SSH keys, `/etc/passwd`, `~/.aws/credentials`
  - `persistence` (high) — writing crontab, systemd units, shell profile modification
  - `crypto-mining` (high) — stratum+tcp URLs, xmrig/cryptonight references
  - `typosquatting` (medium) — Levenshtein distance ≤2 against 50 popular packages
- Dependency audit: scans `package.json` dependencies and `node_modules` postinstall hooks
- LLM evaluation for ambiguous patterns (local model, no cloud)
- Risk level: `none | low | medium | high | critical`
- `--report` flag: HTML + Markdown report via SecurityReporter
- `--json` flag: raw JSON output for CI integration
- Exit code 1 on critical/high findings

### Added — Pentest Security Context

- `PentestSecurityContext` — authorization scope enforcement for active testing
- `declareScope(authorizedBy, targets, allowedModes, durationHours)` — creates signed scope file
- SHA-256 integrity check on scope file — tampering detected on next load
- Scope expiry — automatically invalidated after declared duration
- `checkTarget(ip | hostname | cidr)` — CIDR matching, blocks public internet IPs without authorization
- `checkCommand(cmd)` — blocks DoS tools (`hping3 --flood`, `slowloris`, `loic`, `hoic`, `slowhttptest`), mass scans (`/16`+), destructive flags (`--dump-all`), exfiltration
- `buildPromptConstraints(mode)` — injects scope + FORBIDDEN rules into LLM system prompt
- Interactive `--declare-scope` in CLI with `I AUTHORIZE` confirmation prompt
- Scope status shown before any active test run

### Added — CI/CD Command

- `uneven-ai ci` — headless pipeline (TypeScript typecheck → security scan → test suite)
- Exit code 0 = pass, 1 = fail (CI-compatible)
- Flags: `--skip-security`, `--skip-tests`, `--strict-security`, `--github`, `--output <path>`
- Writes `ci-summary.json` to `.uneven/` with step results and timestamps
- GitHub Actions integration: writes to `$GITHUB_STEP_SUMMARY` when `--github` flag is set

### Added — Retrieval-Augmented Fix (RAF)

- Error fixes are now grounded in the indexed knowledge base before pattern-match fallback
- `KnowledgeRetriever` — semantic vector search with similarity threshold 0.60
- `buildErrorQuery()` — constructs optimized search query from error code, message, language, and context
- `suggestRAFFix()` — structured LLM prompt with KB context, returns `null` on `INSUFFICIENT_CONTEXT`
- `buildContextOnlySuggestion()` — KB-grounded suggestion without LLM inference (confidence ≤ 0.70)
- Fix suggestions extended with `groundedInKB` and `kbSources` fields
- Anti-hallucination: LLM only sees schema/chunks that were actually retrieved; out-of-scope requests return `INSUFFICIENT_CONTEXT`

### Added — Multi-Language Error Parsing

- Go error parser: `file.go:line:col: message` — codes GO_UNDEF, GO_SYNTAX, GO_TYPE
- Java error parser: javac compile errors + `Exception in thread` runtime stack traces
- PHP error parser: Fatal/Parse errors, Warnings, Notices from PHP CLI output
- Ruby error parser: `file.rb:line:in 'method': message (ExceptionClass)` + syntax errors
- Language-specific fix suggestions: `suggestGoFix`, `suggestJavaFix`, `suggestPHPFix`, `suggestRubyFix`
- All parsers return `ParsedError[]` with consistent schema

### Added — Security Report Generator

- `SecurityReporter` — generates HTML and Markdown reports from `SecurityFinding[]`
- HTML: severity-colored badges (red/orange/yellow/blue), summary cards, remediation section
- `uneven-ai pentest --report [md|html|both]`
- Findings collection via `pentest-finding` events during test run

### Added — Test Suite

- Jest + ts-jest configuration with native bridge mocked (bridge unavailable → graceful degradation)
- `__tests__/__mocks__/bridge.ts` — all napi exports stubbed
- `__tests__/error-parser.test.ts` — 20 tests (TS, Go, Java, PHP, Ruby, Python)
- `__tests__/data-security-context.test.ts` — 25 tests (schema filter, SQL audit, result masking, custom policy)
- `__tests__/pentest-security-context.test.ts` — 22 tests (scope lifecycle, target validation, command safety, prompt constraints)
- `__tests__/data-analyst-safe-query.test.ts` — 20 tests (`isSafeQuery` allowed/blocked SQL patterns)
- `__tests__/knowledge-retriever.test.ts` — 12 tests (query builder, graceful degradation)
- `__tests__/malware-scanner.test.ts` — 15 tests (clean files, obfuscation, reverse shell, mining, persistence, typosquatting, supply chain, risk levels)

---

## [0.3.0] - 2026-03-15

### Added — Knowledge Indexing

- File/directory recursive indexing with text extraction
- Document chunking (500 tokens per chunk, 100-token overlap)
- Database connectors: PostgreSQL, MySQL, SQLite, MongoDB
- URL fetching and HTML scraping via undici + cheerio
- PDF and DOCX parsing via pdf-parse, mammoth
- Incremental indexing — only re-indexes changed files
- `FileLoader`, `DatabaseLoader`, `WebScraper` classes
- `uneven-ai index --incremental` flag

### Added — Snapshot & Git Manager

- `.uneven/snapshots/` — file snapshots before each auto-fix
- Git integration: creates commit after each applied fix (optional)
- Rollback: `uneven-ai reset --snapshot <id>`

### Added — Test Runner Integration

- `uneven-ai watch --run-tests` — runs test suite after each auto-fix
- Fix reverted automatically if tests fail after application

---

## [0.2.0] - 2026-02-15

### Added — LLM Inference + Vector Store

- Local LLM inference via Candle (LLaMA 3.2 1B Q8)
- Embedding generation (1024-dimensional, L2 normalized)
- Vector store via usearch HNSW index — persists to `.uneven/vectors.usearch`
- napi-rs exports: `llmEmbed`, `llmInfer`, `retrievalSearch`, `initLlmEngine`
- TypeScript bridge: `llmEmbed()`, `llmInfer()`, `retrievalSearch()` functions
- `uneven-ai ask` — semantic knowledge base query via CLI

### Added — Terminal Watcher + Auto-Fix

- Process spawning via tokio with stdout/stderr capture
- Error parser: TypeScript/JavaScript, Python, Rust compiler errors
- Auto-fix engine with surgical diff application
- `FixSuggestion` with confidence score, explanation, before/after code
- Structured logging to `.uneven/log.md`

---

## [0.1.0] - 2026-01-15

### Added — Project Scaffold

- Rust workspace with napi-rs bridge structure
- TypeScript public API (`Uneven AI` class)
- CLI framework with `init`, `start`, `watch`, `index`, `ask`, `pentest`, `log`, `reset` commands
- Configuration system (`uneven-ai.config.ts`) with cosmiconfig loader
- Logger with Markdown output
- Type definitions for all interfaces (`BrainConfig`, `PentesterConfig`, etc.)
- Error handling foundation
- Module structure for all features
- Documentation: README, CONTRIBUTING, LICENSE (BSL 1.1), llms.txt, llms-full.txt
- Postinstall script for model downloading

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md)

## License

Business Source License 1.1 — See [LICENSE](./LICENSE)
