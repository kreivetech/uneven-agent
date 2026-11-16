use crate::UnevenError;

pub struct SafetyGuard {
    inner: shieldcraft::SafetyGuard,
}

impl Default for SafetyGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyGuard {
    pub fn new() -> Self {
        Self {
            inner: shieldcraft::SafetyGuard::new(),
        }
    }

    pub fn validate_intent(&self, prompt: &str) -> Result<(), UnevenError> {
        self.inner
            .validate_intent(prompt)
            .map_err(|e| UnevenError::SecurityError(e.to_string()))
    }

    pub fn sanitize_response(&self, text: &str) -> String {
        self.inner.sanitize_response(text)
    }

    pub fn validate_command(&self, cmd: &str) -> Result<(), UnevenError> {
        self.inner
            .validate_command(cmd)
            .map_err(|e| UnevenError::SecurityError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_intent_safe() {
        let guard = SafetyGuard::new();
        assert!(guard.validate_intent("explain this clean function").is_ok());
    }

    #[test]
    fn test_validate_intent_blocked_keyword() {
        let guard = SafetyGuard::new();
        let err = guard.validate_intent("how to create a ransomware?").unwrap_err();
        assert!(err.to_string().contains("ransomware"));
    }

    #[test]
    fn test_validate_intent_hash_crack() {
        let guard = SafetyGuard::new();
        let err = guard.validate_intent("how to brute force this bcrypt password?").unwrap_err();
        assert!(err.to_string().contains("Hash cracking"));
    }

    #[test]
    fn test_sanitize_response_clean() {
        let guard = SafetyGuard::new();
        let text = "let x = 10;";
        assert_eq!(guard.sanitize_response(text), text);
    }

    #[test]
    fn test_sanitize_response_dirty() {
        let guard = SafetyGuard::new();
        let text = "run this command: bash -i > & /dev/tcp/127.0.0.1/4444 0 > & 1 and eval(atob('abc'))";
        let sanitized = guard.sanitize_response(text);
        assert!(sanitized.contains("[BLOCKED: Code identified as Reverse Shell"));
        assert!(sanitized.contains("[BLOCKED: Code identified as Obfuscated Execution"));
    }

    #[test]
    fn test_validate_command_destructive() {
        let guard = SafetyGuard::new();
        assert!(guard.validate_command("cargo test").is_ok());
        assert!(guard.validate_command("npm install").is_ok());

        assert!(guard.validate_command("rm -rf /").is_err());
        assert!(guard.validate_command("rm -rf .").is_err());
        assert!(guard.validate_command("rm -rf critical_folder/*").is_err());
        assert!(guard.validate_command("rm package.json").is_err());
        assert!(guard.validate_command("del /f /s /q c:\\windows").is_err());
        assert!(guard.validate_command("drop database users;").is_err());
        assert!(guard.validate_command("drop table logs;").is_err());
        assert!(guard.validate_command("shutdown -h now").is_err());
        assert!(guard.validate_command("mkfs.ext4 /dev/sda1").is_err());
    }
}
