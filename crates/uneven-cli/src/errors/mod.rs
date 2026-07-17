use regex::Regex;
use anyhow::Error;

fn clean_api_error_json(raw: &str) -> Option<String> {
    let start = raw.find(['{', '['])?;
    let end = raw.rfind(['}', ']'])?;
    if start > end {
        return None;
    }
    let json_str = &raw[start..=end];
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
        // Case 1: Array of objects (e.g. Gemini)
        if let Some(arr) = val.as_array() {
            if let Some(first) = arr.first() {
                if let Some(msg) = first.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
                    return Some(msg.to_string());
                }
            }
        }
        // Case 2: Direct object (OpenAI / Claude / Provider compat)
        if let Some(msg) = val.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
            return Some(msg.to_string());
        }
    }
    None
}

/// Sanitizes the raw error message to hide sensitive technical details
/// while preserving UE-XXXXXXXX codes and presenting friendly messages.
pub fn sanitize_error_msg(raw: &str) -> String {
    // 1. Extract UE-Code if present (e.g. [UE-05000001])
    let code_re = Regex::new(r"\[UE-[0-9]{8}\]").unwrap();
    let code = if let Some(m) = code_re.find(raw) {
        m.as_str().to_string()
    } else {
        "[UE-01999999]".to_string()
    };

    // Fast-path friendly whitelist pre-check
    let friendly_re = Regex::new(
        r"(?i)pro license|team license|requires uneven|policy violation|interrupted by user|not initialized|not a git repository|no relationships found|access restricted|file state has changed|license|lockout|webhook|insight|remoteshell|subagent|multi-agent"
    ).unwrap();

    if raw.starts_with("[UE-") && friendly_re.is_match(raw) {
        return raw.to_string();
    }

    // 2. Specialized user-friendly messages for common failures using Regex
    let model_re = Regex::new(r"(?i)(local\s+model|llama|mistral).*not\s+(found|loaded)|cannot\s+open.*models|no\s+such\s+file").unwrap();
    let tokenizer_re = Regex::new(r"(?i)tokenizer not found").unwrap();
    let oom_re = Regex::new(r"(?i)out of memory|insufficient memory|oom").unwrap();
    let timeout_re = Regex::new(r"(?i)timed out|timeout").unwrap();
    let interrupted_re = Regex::new(r"(?i)interrupted").unwrap();
    let permission_re = Regex::new(r"(?i)permission denied|eacces").unwrap();
    let insight_re = Regex::new(r"(?i)insight").unwrap();
    let safe_passthrough_re = Regex::new(r"(?i)gemini|openai|claude|api|fetch|network|connection|write lock").unwrap();
    let db_re = Regex::new(r"(?i)sql|sqlite|database|table|column|relationship").unwrap();

    if model_re.is_match(raw) {
        return format!("{} Local model not found. Run `uneven init` to download it.", code);
    }

    if tokenizer_re.is_match(raw) {
        return format!("{} Model tokenizer missing. Run `uneven init` to reinstall.", code);
    }

    if oom_re.is_match(raw) {
        return format!("{} Not enough memory for local inference. Try a smaller model.", code);
    }

    if timeout_re.is_match(raw) {
        return format!("{} Operation timed out. The hardware may be under too much load.", code);
    }

    if interrupted_re.is_match(raw) {
        return format!("{} Operation was interrupted.", code);
    }

    if permission_re.is_match(raw) {
        return format!("{} Permission denied. Check your file system permissions.", code);
    }

    // Friendly explicit guidance messages take precedence over database masking
    if raw.contains("[UE-03000011]") || insight_re.is_match(raw) {
        return if raw.starts_with("[UE-") { raw.to_string() } else { format!("{} {}", code, raw) };
    }

    // Pass-through for safe networking/API messages, but MASK database internals
    if safe_passthrough_re.is_match(raw) {
        let search_start = if raw.starts_with("[UE-") { 13.min(raw.len()) } else { 0 };
        if let Some(offset_idx) = raw[search_start..].find(['{', '[']) {
            let start_idx = search_start + offset_idx;
            let prefix = raw[..start_idx].trim_end_matches([' ', ':', ',']);
            let json_part = &raw[start_idx..];
            if let Some(clean_msg) = clean_api_error_json(json_part) {
                let formatted = format!("{}: {}", prefix, clean_msg);
                return if formatted.starts_with("[UE-") { formatted } else { format!("{} {}", code, formatted) };
            }
        }
        return if raw.starts_with("[UE-") { raw.to_string() } else { format!("{} {}", code, raw) };
    }

    if db_re.is_match(raw) {
        return format!("{} Database operation failed. Check your config and whitelisted tables.", code);
    }

    // 3. Final Fallback: Mask raw technical details unless it matches a friendly whitelist
    let friendly_re = Regex::new(
        r"(?i)pro license|team license|requires uneven|policy violation|interrupted by user|not initialized|not a git repository|no relationships found|access restricted|file state has changed|license|lockout|webhook|insight|remoteshell|subagent|multi-agent"
    ).unwrap();

    if raw.starts_with("[UE-") {
        if friendly_re.is_match(raw) {
            raw.to_string()
        } else {
            // Extract the category prefix (e.g., characters at index 4 and 5 in "[UE-03000001]")
            let category = if code.len() >= 6 {
                &code[4..6]
            } else {
                "01"
            };
            match category {
                "02" => format!("{} Configuration or security error.", code),
                "03" => format!("{} Infrastructure error.", code),
                "04" => format!("{} Knowledge base error.", code),
                "05" => format!("{} LLM inference error.", code),
                "06" => format!("{} Automation or watcher error.", code),
                "07" => format!("{} Security analyzer error.", code),
                _ => format!("{} Internal engine error.", code),
            }
        }
    } else {
        format!("{} Internal engine error.", code)
    }
}

pub fn handle_error(err: Error) {
    let root_cause = err.root_cause();
    let sanitized_msg = sanitize_error_msg(&root_cause.to_string());
    
    // Extrapolate the label/context from the anyhow context chain
    let full_msg = err.to_string();
    let root_msg = root_cause.to_string();
    
    let label = if full_msg.len() > root_msg.len() {
        let suffix = format!(": {}", root_msg);
        if full_msg.ends_with(&suffix) {
            full_msg[..full_msg.len() - suffix.len()].to_string()
        } else {
            "Command failed".to_string()
        }
    } else {
        "Error".to_string()
    };

    eprintln!("\n  {} {}: {}", 
        crate::ui::truecolor_bold("✗", 255, 107, 53),
        crate::ui::truecolor_bold(&label, 255, 107, 53),
        crate::ui::truecolor(&sanitized_msg, 232, 237, 245)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_error_msg_model_not_found() {
        let raw = "Error: local model llama-3.2 not found";
        let sanitized = sanitize_error_msg(raw);
        assert!(sanitized.contains("Local model not found. Run `uneven init` to download it."));
        assert!(sanitized.contains("[UE-01999999]"));
    }

    #[test]
    fn test_sanitize_error_msg_oom() {
        let raw = "[UE-05000003] out of memory during inference";
        let sanitized = sanitize_error_msg(raw);
        assert!(sanitized.contains("Not enough memory for local inference."));
        assert!(sanitized.starts_with("[UE-05000003]"));
    }

    #[test]
    fn test_sanitize_error_msg_database() {
        let raw = "no such table: users in SQL execution";
        let sanitized = sanitize_error_msg(raw);
        assert!(sanitized.contains("Database operation failed."));
        assert!(sanitized.contains("[UE-01999999]"));
    }

    #[test]
    fn test_sanitize_error_msg_whitelist_pro_license() {
        let raw = "[UE-01000005] This feature requires Uneven Pro.";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-01000005] This feature requires Uneven Pro.");
    }

    #[test]
    fn test_sanitize_error_msg_mask_internal_io() {
        let raw = "[UE-03000001] IO error: file not found (os error 2) at /path/to/some/file";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-03000001] Infrastructure error.");
    }

    #[test]
    fn test_sanitize_error_msg_knowledge_base() {
        let raw = "[UE-04000001] Knowledge base error: Search index parsing failed: unexpected token on line 1";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-04000001] Knowledge base error.");
    }

    #[test]
    fn test_sanitize_error_msg_mask_unknown_category() {
        let raw = "[UE-99000001] Unknown runtime exception: thread panicked";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-99000001] Internal engine error.");
    }

    #[test]
    fn test_sanitize_error_msg_whitelist_remoteshell() {
        let raw = "[UE-02000001] remoteShell is not configured. Please use '/' in the interactive shell to configure settings.";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-02000001] remoteShell is not configured. Please use '/' in the interactive shell to configure settings.");
    }

    #[test]
    fn test_sanitize_error_msg_gemini_quota() {
        let raw = "[UE-05000001] LLM error: Gemini API error (429 Too Many Requests): [{\n  \"error\": {\n    \"code\": 429,\n    \"message\": \"Quota exceeded.\",\n    \"status\": \"RESOURCE_EXHAUSTED\"\n  }\n}]";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-05000001] LLM error: Gemini API error (429 Too Many Requests): Quota exceeded.");
    }

    #[test]
    fn test_sanitize_error_msg_openai_insufficient_balance() {
        let raw = "[UE-05000001] LLM error: Provider API error (402 Payment Required): {\"error\":{\"message\":\"Insufficient Balance\",\"type\":\"unknown_error\",\"param\":null,\"code\":\"invalid_request_error\"}}";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-05000001] LLM error: Provider API error (402 Payment Required): Insufficient Balance");
    }

    #[test]
    fn test_sanitize_error_msg_subagents() {
        let raw = "[UE-06000012] Subagent LLM initialization failed: provider connection timed out";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-06000012] Subagent LLM initialization failed: provider connection timed out");

        let raw = "[UE-06000013] Subagent inference failed: API error (503)";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-06000013] Subagent inference failed: API error (503)");

        let raw = "[UE-06000014] Subagent file write failed: read-only filesystem";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-06000014] Subagent file write failed: read-only filesystem");

        let raw = "[UE-06000015] Subagent file modification limit exceeded";
        let sanitized = sanitize_error_msg(raw);
        assert_eq!(sanitized, "[UE-06000015] Subagent file modification limit exceeded");
    }
}
