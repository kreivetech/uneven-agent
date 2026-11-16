# How Uneven Agent Works

This document describes the high-level design of Uneven Agent — what it does, how its subsystems relate, and the privacy guarantees it provides. Internal implementation details are not disclosed.

---

## Overview

Uneven Agent is a local-first development agent. Every component that handles your code, your data, or your credentials operates on your machine. Nothing leaves your environment unless you explicitly configure a cloud brain.

---

## Subsystems

### Knowledge Base
Uneven indexes your codebase, documentation, and connected databases into a local vector store. Semantic search over this index powers every feature — from answering questions about your project to generating context-aware fixes.

The index is stored on disk and updated incrementally. Only changed files are re-processed on subsequent runs.

### Inference Engine
Uneven supports two inference modes:

- **Local model** — runs fully offline; no API key required. The engine uses your CPU (or GPU when available) to run a bundled quantized model.
- **Cloud brain** — delegates reasoning to an external provider (OpenAI, Claude, Gemini, or Ollama). Your API key stays local; only the prompt and response travel over the network.

Both modes use the same interface. You can switch at any time via `uneven configure`.

### GPU Acceleration
During `uneven init`, Uneven detects your GPU and automatically installs the optimized native binary for your hardware. No manual steps or recompilation required.

Supported acceleration: **CUDA** (NVIDIA), **Metal** (Apple Silicon). Falls back to CPU on unsupported hardware.

### Terminal Watcher
A background process monitors your terminal for runtime errors. When an error is detected, Uneven retrieves relevant context from the knowledge base and applies a targeted fix — or surfaces the analysis for your review, depending on your configuration.

### Security Shield
Static analysis across your dependency tree and source files detects malicious patterns, supply chain indicators, and compromised packages. Results are scoped strictly to your project.

### Conversational Shell
Running `uneven` with no arguments opens an interactive shell. You can issue any command through natural language in any language. The shell routes your input to the correct subsystem and responds conversationally.

The shell is available on the **free tier**. Special commands like `review`, `ci`, and `pentest` require a Pro/Team license.

### Remote Shell Server
A lightweight, high-performance in-process HTTP/HTTPS server that exposes Uneven's capabilities to external integrations (like IDE extensions). It handles stateful chat sessions, configuration updates, and message routing.

### Web Search & Scraping
An autonomous, keyless web search and crawling engine utilizing DuckDuckGo Lite HTML scraping and Wikipedia search APIs. Web pages can be scraped on-demand using a regex-based HTML cleaner that transforms pages into markdown. Domain sitemaps are crawled in the background and cached locally to speed up subsequent resource lookups.

---

## Command Logic and Subsystem Execution

This section details the internal runtime behavior, flows, and optimization strategies for the primary CLI commands.

### 1. Conversational Shell (`uneven` / No Arguments)
The interactive shell manages natural language dialogues using a multi-mode optimization loop to minimize cloud latency and token consumption:
* **Heuristic Routing:** Inputs are first run through a 100% local, quick scanner. Unambiguous actions (like `exit`, `help`, `watch`, `scan`, `index`, `undo`) are executed locally without calling the brain (0 ms latency).
* **Brain Classifier (`shell.md`):** If the input is conversational, the shell runs a classification step sending the query, files map, and recent conversation history.
  * **Single-Turn Concept Flow:** For theoretical queries or greetings that do not require project files, the classifier sets `"command": "none"` and embeds the final answer directly in `"message"`. The shell displays this message immediately. **Total: 1 LLM turn.**
  * **Double-Turn Context Flow (RAG):** For queries referring to the codebase, the classifier identifies the intent and populates the `"paths"` array with relevant files. The local Rust engine reads these files directly from disk and forwards them along with the history to the chat generator. **Total: 2 LLM turns** (the separate intermediate LLM routing call has been completely eliminated).
* **Context Compaction (`optimize_context`):** To avoid resending massive instructions and file maps on every stateless HTTP request, system rules (`law`) and file maps (`map`) are injected into the history on the first turn using structured tag blocks (`SYSTEM_LAWS_ACTIVE` / `PROJECT_FILES_MAP_ACTIVE`). On subsequent turns, they are dynamically stripped from the payload and only updated in-place if changes are made to the workspace. This leverages the LLM provider's **Prompt Caching**, saving up to 90% of tokens.
* **Agentic File Auto-Recovery (Three-Cache Architecture):** If the LLM determines it needs to read additional files during execution, the shell runs an agentic recovery loop using three cache layers:
  * *Cache 1 (Dialogue History):* Tracks conversation turns.
  * *Cache 2 (Loaded Files Registry):* A set of relative file paths loaded in the turn, used to prevent infinite loops (using set difference operations).
  * *Cache 3 (Formatted Content Cache):* Caches files already read and formatted with line numbers, avoiding redundant I/O and processing on subsequent loop iterations.
* **Dynamic History Limits:** The dialogue history (Cache 1) size automatically adjusts based on the brain provider:
  * *Local brain (`provider: "local"`):* Retains the last **8 turns** (maximizing generation speed and preventing OOM errors on consumer hardware).
  * *Cloud brain (e.g., `provider: "gemini"`):* Retains the last **14 turns** (taking advantage of larger context windows and prefix caching).
* **White-Label Compatibility (Brand-Neutrality):** All internal prompts and instruction files are brand-neutral, removing product-specific names and corporate references to allow seamless integration into white-labeled enterprise environments.
* **Active Loading Spinner & EOF Graceful Exit:** When the shell initializes, it shows an active loader spinner during model boot up to prevent visual lag. Stdin reads check for zero-byte returns (EOF) to exit cleanly when inputs are piped or closed.

### 2. `index` (Knowledge Base Indexer)
Scans directories listed in `knowledge.dirs` and independent configuration files in `knowledge.files` (such as `package.json`). It breaks them down into semantic text chunks, calculates vector embeddings, and populates the incremental local database. Only modified files are processed during subsequent index operations.
* **Fast Startup Ignore Filter:** Utilizes `WalkBuilder` filter entries to explicitly skip traversing compiler build artifacts, dependency directories, and repository state folders (such as `target/`, `node_modules/`, `.git/`, `dist/`). This optimizes indexing startup times from minutes down to milliseconds.

### 3. `ask` (CLI Q&A Interface)
Executes a single-turn question over the codebase. It triggers the `ContextAssembler` to identify relevant files via vector lookup or semantic routing, loads their contents, formats the context instructions, and calls the brain model to provide a precise, offline-validated response.

### 4. `askf` (Autonomous File Generator)
Translates complex coding tasks into ready-to-compile file updates:
* It reads the request context and prompts the brain.
* The output is parsed for structured markers: `FILE: <relative_path> \n ```code\n ``` \n END_FILE`.
* The Rust engine parses these markers in real time, securely writes or modifies the files on the local disk, and automatically tags them for incremental indexing.
* **Obsolete Fallback Backups**: Before any file is deleted or completely overwritten by `askf`, the Rust engine automatically copies the original file to `.uneven/obsolete` inside the project root (mirroring the relative directory structure and appending a `_YYYYMMDD_HHMMSS` timestamp suffix). This ensures that if the project lacks Git or the automatic restore system fails, the user can always manually recover their files.


### 5. `watch` / `start` (Autonomous Terminal Watcher)
Spawns target commands (like `npm run dev`) in a monitored subprocess. It intercepts standard error streams (stderr) to detect compilation failures, exceptions, and runtime crashes:
* It parses the traceback, finds the failing code file and line, and queries the local knowledge database for matching context.
* It feeds the error and context into the `fix` brain.
* Propose surgical fixes that resolve the error with minimal modifications, applying them automatically (Pro) or upon confirmation (Free).
* **Watcher Event Filtering:** Verifies change events against path components, ignoring nested events inside the `.uneven/` cache folders. This prevents write-loop triggers and database lock contention.

### 6. `scan` (Malware & Supply Chain Auditor)
Runs fully offline static analysis over source code and dependencies. It checks for signatures of supply chain injections, reverse shells, obfuscated payload loaders, rogue network calls, credential exfiltration, and typosquatting.

### 7. `review` (Code Reviewer)
Loads git diff patches (staged, commit hashes, or branch references) and feeds them into the `review` brain alongside the system rules. It validates code styles, checks for security bottlenecks, recommends guard clauses, and formats a technical Markdown report.

### 8. `analyze` (Data Analyst Engine)
Runs interactive natural language business intelligence queries:
* Connects locally to database drivers in read-only mode.
* Pre-filters schema metadata to omit tables and columns defined as sensitive.
* Converts natural language requests to SQL, allowing manual validation before execution.
* Mask sensitive data (passwords, JWTs, secret keys) in results dynamically.

### 9. `pentest` (Security Pentester)
Conducts automated vulnerability scanning and active target exploitation tests. It strictly enforces security boundaries by requiring explicit authorization declaration and integrity signature before executing active modes.

### 10. Multi-Agent Task Orchestrator (Multi-Agent Mode)
When multi-agent mode is enabled (via `/agents`), complex programming directives are analyzed and decomposed into a sequence of logical subtasks. The orchestrator delegates each subtask to an isolated subagent:
* **Sandboxed Subagents:** Each subagent is initialized with its own context session ID, token constraints (with a safe minimum floor of 10,000 output tokens), and a strict system law boundary restricting file modifications to a maximum of 3 files to prevent accidental codebase pollution.
* **Resumption Cache:** Subtask queues are serialized to disk (`multi_agent_tasks_<session-id>.json`) within the project's global cache directory. If a run is interrupted or fails due to token exhaustion, the remaining tasks are preserved, enabling seamless user-confirmed resumption on the next run.
* **Non-Blocking Asynchronous Cancellation:** A background keyboard monitor listens asynchronously on standard input. If `stop`, `abort`, or `exit` is detected, the orchestrator breaks the task loops and terminates ongoing subagent inferences instantly, returning the terminal control back to the interactive prompt cleanly.

### 11. Remote Shell Server (`uneven remote-shell`)
Uneven spawns a native TCP server that manually parses HTTP/HTTPS requests to offer low-latency integration endpoints for IDE extensions:
* **Custom HTTP Parser:** Implements a zero-dependency HTTP parsing loop directly on top of `tokio::net::TcpListener` supporting optional TLS wrappers (HTTPS).
* **Endpoints:**
  * `GET /health` — Returns status and version metadata.
  * `GET /config` — Retrieves the current project configuration.
  * `POST /config` — Updates specific parts of the configuration in-place.
  * `POST /chat` — Manages stateful conversational chat turns tied to an `x-session-id`.
  * `POST /message` — Processes stateless query classification and execution turns.
  * `GET /` — Serves self-documenting guidance for active endpoints and authorization rules.
* **Security & Hardening:**
  * *Brute Force Detection:* A memory-based tracker locks client IPs for 5 minutes (`HTTP 423`) after successive failed Bearer secret validations.
  * *CORS Origin Verification:* Enforces whitelisted origin hosts declared in the configuration.
  * **Rate Limiting:** Restricts requests per minute on a per-session/IP basis, responding with `HTTP 429` on overload.

### 12. Web Search & Sitemap Caching Engine
* **Keyless Search Execution:** When the LLM requests a search (via `[WEB_SEARCH: query]`), the Rust engine queries DuckDuckGo Lite (via clean scraping of `html.duckduckgo.com`) and Wikipedia (via open API), merging the top results (titles, snippets, URLs) and formatting them into standard markdown injects.
* **Asynchronous Sitemap Crawling:** For top search results or scraped pages, the engine spawns background tasks to resolve the domain's sitemap. It queries the `/robots.txt` file for sitemap paths, falling back to `/sitemap.xml`, `/sitemap.txt`, and `/sitemap_index.xml`.
* **Local Sitemap Caching:** Sitemap URLs are extracted using regex and cached in JSON format in the global cache directory: `global_bin_dir/cache/{project_hash}/web-sitemaps/{domain}.json`. This protects project directories from write pollutions.
* **Sitemap Semantic Injection:** Subsequent web searches scan this cache folder for matching keywords, injecting relevant cached URLs to guide the agent in locating documentation structure.

---

## Privacy Model

| Data | Where it goes |
|---|---|
| Source code | Stays local (vector index on disk) |
| Terminal output | Stays local |
| Prompts (local mode) | Stays local |
| Prompts (cloud brain) | Sent to your configured provider only |
| API keys | Stored in local config; never transmitted |
| Activity logs | Stored on disk; never transmitted |
| Telemetry | **None**. No analytics or remote tracking |

---

## License

Uneven Agent is proprietary software. The source code is not open. See [LICENSE](../LICENSE) for terms.

© 2026 KR Riley Soluções. All rights reserved.
