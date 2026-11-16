use serde::{Deserialize, Serialize};

/// Represents an action command parsed from the agent's thoughts or shell response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ShellActionResponse {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub paths: Vec<String>,
}

/// Parses declarative markdown action blocks (e.g., ```shell-action ... ```, ```action ... ```, or ```shell ... ```).
pub fn parse_shell_action_block(text: &str) -> Option<ShellActionResponse> {
    let lower = text.to_lowercase();
    let (block_start_idx, fence_len) = if let Some(idx) = lower.find("```shell-action") {
        (idx + "```shell-action".len(), 15)
    } else if let Some(idx) = lower.find("```action") {
        (idx + "```action".len(), 9)
    } else {
        let idx = lower.find("```shell")?;
        (idx + "```shell".len(), 8)
    };

    let after_start = &text[block_start_idx..];
    let (block_content, after_block) = if let Some(end_idx) = after_start.find("```") {
        (&after_start[..end_idx], &after_start[end_idx + 3..])
    } else {
        (after_start, "")
    };

    let before_fence_end = block_start_idx.saturating_sub(fence_len);
    let before_block = if before_fence_end <= text.len() {
        &text[..before_fence_end]
    } else {
        ""
    };
    let outer_msg = format!("{}\n{}", before_block.trim(), after_block.trim())
        .trim()
        .to_string();

    let mut command = String::new();
    let mut args = Vec::new();
    let mut paths = Vec::new();
    let mut message = if !outer_msg.is_empty() {
        outer_msg
    } else {
        String::new()
    };

    for line in block_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, val)) = trimmed.split_once(':') {
            let key = key.trim().to_lowercase();
            let val = val.trim().trim_matches(|c| c == '`' || c == '"' || c == '\'');

            match key.as_str() {
                "command" | "cmd" | "action" => {
                    command = val.to_string();
                }
                "args" | "arg" | "task" | "question" => {
                    if val.starts_with('[') && val.ends_with(']') {
                        let inner = &val[1..val.len() - 1];
                        for item in inner.split(',') {
                            let clean_item =
                                item.trim().trim_matches(|c| c == '`' || c == '"' || c == '\'');
                            if !clean_item.is_empty() {
                                args.push(clean_item.to_string());
                            }
                        }
                    } else if !val.is_empty() {
                        args.push(val.to_string());
                    }
                }
                "paths" | "path" | "file" | "files" => {
                    if val.starts_with('[') && val.ends_with(']') {
                        let inner = &val[1..val.len() - 1];
                        for item in inner.split(',') {
                            let clean_item =
                                item.trim().trim_matches(|c| c == '`' || c == '"' || c == '\'');
                            if !clean_item.is_empty() {
                                paths.push(clean_item.to_string());
                            }
                        }
                    } else if !val.is_empty() {
                        paths.push(val.to_string());
                    }
                }
                "message" | "msg" | "reply" if message.is_empty() => {
                    message = val.to_string();
                }
                _ => {}
            }
        } else if let Some(item) = trimmed.strip_prefix("- ") {
            let item = item
                .trim()
                .trim_matches(|c| c == '`' || c == '"' || c == '\'');
            if !item.is_empty() {
                args.push(item.to_string());
            }
        }
    }

    if !command.is_empty() {
        Some(ShellActionResponse {
            command,
            args,
            message,
            paths,
        })
    } else {
        None
    }
}

/// Extracts a [`ShellActionResponse`] from either a declarative markdown block or raw/embedded JSON.
pub fn extract_action_response(text: &str) -> Option<ShellActionResponse> {
    if let Some(resp) = parse_shell_action_block(text) {
        return Some(resp);
    }
    if let Ok(direct) = serde_json::from_str::<ShellActionResponse>(text) {
        return Some(direct);
    }
    let start = text.find('{')?;
    let mut depth = 0;
    for (i, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let candidate = &text[start..start + i + 1];
                    if let Ok(parsed) = serde_json::from_str::<ShellActionResponse>(candidate) {
                        return Some(parsed);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_declarative_action_block() {
        let input = "I will plan this refactor.\n\n```shell-action\ncommand: think\nargs: [\"migrate auth to oauth2\"]\npaths: [\"src/auth.rs\", \"src/routes.rs\"]\nmessage: \"Plan generated\"\n```\n";
        let parsed = parse_shell_action_block(input).expect("Should parse declarative action");
        assert_eq!(parsed.command, "think");
        assert_eq!(parsed.args, vec!["migrate auth to oauth2"]);
        assert_eq!(parsed.paths, vec!["src/auth.rs", "src/routes.rs"]);
    }

    #[test]
    fn test_extract_embedded_json() {
        let input = "Some text before {\"command\": \"dde\", \"args\": [\"apply changes\"], \"paths\": []} and text after";
        let parsed = extract_action_response(input).expect("Should extract embedded JSON");
        assert_eq!(parsed.command, "dde");
        assert_eq!(parsed.args, vec!["apply changes"]);
    }
}
