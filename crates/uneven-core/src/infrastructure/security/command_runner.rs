use crate::UnevenError;
use std::path::Path;
use std::time::Duration;

pub use shieldcraft::CommandExecutionResult;

/// Executes a safe terminal command inside the specified working directory.
/// Validates the command using `shieldcraft::execute_safe_command`.
pub async fn execute_safe_command(
    cmd: &str,
    working_dir: &Path,
    timeout_duration: Option<Duration>,
) -> Result<CommandExecutionResult, UnevenError> {
    shieldcraft::execute_safe_command(cmd, working_dir, timeout_duration)
        .await
        .map_err(|e| match e {
            shieldcraft::ShieldError::CommandBlocked(msg) => UnevenError::SecurityError(msg),
            shieldcraft::ShieldError::Timeout(msg) => UnevenError::Timeout(msg),
            other => UnevenError::AgentError(other.to_string()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_execute_safe_command_echo() {
        let dir = tempdir().unwrap();
        let result = execute_safe_command("echo hello_world", dir.path(), None).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("hello_world"));
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_execute_blocked_command() {
        let dir = tempdir().unwrap();
        let result = execute_safe_command("rm -rf /", dir.path(), None).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UnevenError::SecurityError(_)));
    }
}
