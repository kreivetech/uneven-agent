# Uneven Engine Error Codes Master Dictionary

This document maps unique error codes (`UE-XXXXXXXX`) to their technical causes and user-facing messages.

### Rationale & Design Goal
Error codes and sanitization are used to avoid exposing internal system/engine error traces to the user. Instead of seeing detailed paths, stack traces, database schemas, or LLM connection diagnostics, the user receives a sanitized, friendly error message along with a unique error code (e.g., `[UE-06000013]`). Developers use these codes internally to locate the failing method or module in the codebase and apply corrections without exposing internal architecture or security-sensitive details to the end-user.

## Category Overview

| Range | Category | Description |
|-------|----------|-------------|
| `01` | Core & Lifecycle | Engine initialization, shutdowns, and unknown internal failures. |
| `02` | Config & Security | Configuration errors and security policy violations. |
| `03` | Infrastructure | IO, Serialization, Git, and Database operations. |
| `04` | Knowledge Base | Document loading, chunking, and semantic retrieval. |
| `05` | AI & LLM | Inference failures, timeouts, OOM, and model management. |
| `06` | Automation | Agent loops, auto-fix, and file system watchers. |
| `07` | Pentester | Security scanning, SBOM, and vulnerability analysis. |

---

## Active Error Codes

### [01] Core & Lifecycle

| Code | File | User Message | Technical Detail |
|------|------|--------------|------------------|
| `UE-01000001` | `engine.rs` | Engine already initialized. | Attempted to initialize the native engine while it was already running. |
| `UE-01000002` | `engine.rs` | Engine not initialized. | Attempted to use engine features before calling `init_engine`. |
| `UE-01000003` | `engine.rs` | Engine initialization failed. | Generic failure during the native engine startup sequence. |
| `UE-01000004` | `bridge.ts` | Native bridge loading failed. | Failed to load the `.node` binary or resolve native symbols. |
| `UE-01000005` | `gate.ts` | Pro license required. | Attempted to use a Pro feature on a free-tier license. |
| `UE-01000006` | `gate.ts` | Team license required. | Attempted to use a Team feature without the required license tier. |
| `UE-01000007` | `process-lock.ts` | Uneven is already running. | Another Uneven process holds the file lock for this project. |
| `UE-01999999` | `error.rs` | An unknown internal error occurred. | Catch-all for `UnevenError::Unknown`. |

### [02] Config & Security

| Code | File | User Message | Technical Detail |
|------|------|--------------|------------------|
| `UE-02000001` | `error.rs` | Configuration error. | Invalid or missing configuration parameters. |
| `UE-02000002` | `error.rs` | Security policy violation. | Action blocked by security rules (e.g., blocked columns). |
| `UE-02000003` | `guard.ts` | Rate limit exceeded. | Request blocked due to exceeding the configured rate limit (RPM/Burst). |
| `UE-02000004` | `guard.ts` | Brute-force lockout. | Multiple authentication failures detected; IP or Session temporarily locked. |
| `UE-02000005` | `guard.ts` | Webhook SSRF protection. | Attempted notification to a private or loopback IP address was blocked. |
| `UE-02000006` | `server.ts` | CORS policy violation. | Request origin is not allowed by the configured Remote Shell CORS policy. |

### [03] Infrastructure

| Code | File | User Message | Technical Detail |
|------|------|--------------|------------------|
| `UE-03000001` | `error.rs` | IO error. | File system or network IO failure. |
| `UE-03000002` | `error.rs` | Serialization error. | JSON parsing or serialization failure. |
| `UE-03000003` | `error.rs` | Git error. | Failure during Git operations (diff, log, status). |
| `UE-03000004` | `database.rs` | Database not initialized. | Attempted to use database features before call `db_connect`. |
| `UE-03000005` | `database.rs` | Database connect failed. | Failed to establish connection with the database URL. |
| `UE-03000006` | `database.rs` | Failed to list tables. | Error retrieving table metadata from the connected database. |
| `UE-03000007` | `database.rs` | Introspection failed. | Error analyzing table schema and column types. |
| `UE-03000008` | `database.rs` / `analyzer.ts` | Query failed. | SQL execution error or result processing failure. |
| `UE-03000009` | `database.rs` | Serialization failed. | Error converting database results to JSON for export. |
| `UE-03000010` | `database.rs` | Export failed. | File system error during report generation or packaging. |
| `UE-03000011` | `analyzer.ts` | No relationships found. | Analysis requires at least one relationship between selected tables. |
| `UE-03000012` | `analyzer.ts` | Database access restricted. | No whitelisted tables found in uneven.config.ts. |

### [04] Knowledge Base

| Code | File | User Message | Technical Detail |
|------|------|--------------|------------------|
| `UE-04000001` | `error.rs` | Knowledge base error. | Failure loading or processing knowledge documents. |
| `UE-04000002` | `error.rs` | Retrieval error. | Semantic search or context retrieval failure. |

### [05] AI & LLM

| Code | File | User Message | Technical Detail |
|------|------|--------------|------------------|
| `UE-05000001` | `error.rs` | LLM error. | General failure during LLM inference. |
| `UE-05000006` | `llm.rs` | Embedding failed. | Error generating semantic embedding for text. |
| `UE-05000007` | `llm.rs` | Batch embedding failed. | Error generating embeddings for multiple texts. |
| `UE-05000008` | `llm.rs` | Inference failed. | Native engine failure during text generation. |
| `UE-05000009` | `llm.rs` | Streaming failed. | Error during token generation or callback execution. |
| `UE-05000002` | `error.rs` | Model not found. | Requested LLM model is missing or not downloaded. |
| `UE-05000003` | `error.rs` | Out of memory (OOM). | Insufficient memory to load the model or perform inference. |
| `UE-05000004` | `error.rs` | Inference interrupted by user. | The inference process was manually stopped. |
| `UE-05000005` | `error.rs` | Inference timed out. | The inference took longer than the configured timeout. |

### [06] Automation

| Code | File | User Message | Technical Detail |
|------|------|--------------|------------------|
| `UE-06000001` | `error.rs` | Watcher error. | Failure in the file system watcher (inotify/fsevents). |
| `UE-06000002` | `error.rs` | Agent error. | Failure in the autonomous agent logic or fix loop. |
| `UE-06000004` | `helpers.ts` | API Quota exceeded. | LLM provider rate limit or quota reached. |
| `UE-06000005` | `helpers.ts` | Invalid API key. | Provided LLM credentials were rejected. |
| `UE-06000006` | `helpers.ts` | Model not found. | Requested model ID is invalid or unavailable for this provider. |
| `UE-06000007` | `helpers.ts` | Provider overloaded. | LLM provider servers are currently too busy to handle the request. |
| `UE-06000008` | `helpers.ts` | Safety filter blocked. | Response blocked by provider's safety or content filtering policies. |
| `UE-06000009` | `helpers.ts` | Analysis truncated. | Response cut off because it reached the maximum token limit. |
| `UE-06000010` | `bridge.ts` | Could not apply the suggested fix. | The patch operation failed. |
| `UE-06000011` | `fix.rs` | File state has changed. | The file has been modified externally; aborting patch application. |
| `UE-06000012` | `multi_agents.rs` | Subagent LLM initialization failed. | LLM initialization failed for a subagent execution. |
| `UE-06000013` | `multi_agents.rs` | Subagent inference failed. | Inference engine or API call failed for a subagent. |
| `UE-06000014` | `multi_agents.rs` | Subagent file write failed. | Failed to write subagent modifications to the file system. |
| `UE-06000015` | `multi_agents.rs` | Subagent file modification limit exceeded. | Subagent attempted to modify more than the limit of 3 files. |

### [07] Pentester

| Code | File | User Message | Technical Detail |
|------|------|--------------|------------------|
| `UE-07000001` | `pentest.ts` | Supply chain audit failed. | Generic failure in the supply chain module bridge. |
| `UE-07000002` | `sbom.rs` | SBOM generation failed. | Error during CycloneDX JSON serialization or file write. |
| `UE-07000003` | `error.rs` | Pentester error. | General failure in security scanning or analysis. |
| `UE-07000004` | `pentester.rs` | Active testing failed. | Failure during active security probing or network scanning. |
| `UE-07000005` | `pentester.rs` | Report packaging failed. | Failure when converting reports to standalone format. |
| `UE-07000006` | `pentest.ts (CLI)` | Scope authorization required. | Active pentesting attempted without a valid authorized scope. |
| `UE-07000007` | `pentest.ts (CLI)` | Scope declaration failed. | Failure during the interactive scope declaration process. |
| `UE-07000008` | `pentest.ts (CLI)` | Team license required. | Attempted to use active mode without a valid Team/Pro license. |
