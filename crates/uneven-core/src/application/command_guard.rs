use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

/// Global command execution lock to prevent parallel execution of heavy build/check
/// commands across the Master agent and all Pod Primes that would freeze the user's host machine.
static HEAVY_COMMAND_LOCK: Lazy<Arc<Mutex<()>>> = Lazy::new(|| Arc::new(Mutex::new(())));

/// Checks whether a command string represents a heavy compilation, testing, or build step.
pub fn is_heavy_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    lower.contains("cargo check")
        || lower.contains("cargo build")
        || lower.contains("cargo test")
        || lower.contains("cargo clippy")
        || lower.contains("npm run build")
        || lower.contains("npm test")
        || lower.contains("yarn build")
        || lower.contains("pnpm build")
        || lower.contains("tsc")
        || lower.contains("pytest")
        || lower.contains("go build")
        || lower.contains("go test")
        || lower.contains("mvn")
        || lower.contains("gradle")
}

/// Acquires the global lock for heavy command execution.
/// If another Pod, Prime, or Master is currently running a heavy command,
/// this will asynchronously await until the previous run finishes.
pub async fn acquire_heavy_command_guard() -> MutexGuard<'static, ()> {
    HEAVY_COMMAND_LOCK.lock().await
}

/// System prompt instruction injected into Master and all Pod Primes
/// to ensure agents avoid hammering compiler checks and respect single-threaded host limits.
pub const RESOURCE_CONCURRENCY_DIRECTIVE: &str = r#"
[HOST RESOURCE & CONCURRENCY CONSTRAINTS]
- The host system has limited CPU and memory resources.
- STRICTLY AVOID running multiple, redundant, or concurrent compilation/build commands (e.g. `cargo check`, `cargo build`, `npm run build`, `pytest`).
- If a check or build is already executing, wait for it to finish before requesting another.
- Prioritize surgical static code reasoning, syntax analysis, and patch precision before resorting to full compiler checks.
"#;
