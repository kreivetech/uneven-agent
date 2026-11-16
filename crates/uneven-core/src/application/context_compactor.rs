use serde::{Deserialize, Serialize};
use crate::infrastructure::llm::history_manager::BrainTurn;

/// Default token threshold that triggers automatic context compaction.
pub const DEFAULT_COMPACTION_TOKEN_THRESHOLD: usize = 8_000;

/// Default turn count threshold that triggers automatic context compaction.
pub const DEFAULT_COMPACTION_TURN_THRESHOLD: usize = 10;

/// Number of recent turns to always preserve verbatim for conversational continuity.
pub const PRESERVED_RECENT_TURNS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactionOutcome {
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub tokens_saved: usize,
    pub was_compacted: bool,
}

pub struct ContextCompactorService;

impl ContextCompactorService {
    /// Estimates token count of a given text using a fast 4-char heuristic.
    pub fn estimate_str_tokens(text: &str) -> usize {
        (text.len() / 4).max(1)
    }

    /// Estimates the total tokens across all turns in a conversation history.
    pub fn estimate_history_tokens(history: &[BrainTurn]) -> usize {
        history.iter().map(|turn| Self::estimate_str_tokens(&turn.content)).sum()
    }

    /// Determines whether the conversation history should be automatically compacted.
    pub fn should_compact(history: &[BrainTurn]) -> bool {
        if history.len() <= PRESERVED_RECENT_TURNS {
            return false;
        }

        let total_tokens = Self::estimate_history_tokens(history);
        total_tokens >= DEFAULT_COMPACTION_TOKEN_THRESHOLD || history.len() >= DEFAULT_COMPACTION_TURN_THRESHOLD
    }

    /// Automatically compacts the conversation history in-place if it exceeds thresholds.
    /// Preserves the most recent turns verbatim and condenses older turns into a dense summary.
    pub fn auto_compact(history: &mut Vec<BrainTurn>) -> CompactionOutcome {
        if !Self::should_compact(history) {
            let tokens = Self::estimate_history_tokens(history);
            return CompactionOutcome {
                tokens_before: tokens,
                tokens_after: tokens,
                tokens_saved: 0,
                was_compacted: false,
            };
        }

        let tokens_before = Self::estimate_history_tokens(history);

        // Split into older turns to compact and recent turns to preserve verbatim
        let split_idx = history.len().saturating_sub(PRESERVED_RECENT_TURNS);
        let older_turns: Vec<BrainTurn> = history.drain(0..split_idx).collect();

        // Synthesize the older turns into a structured summary
        let summary_content = Self::distill_older_turns(&older_turns);

        // Insert the compacted summary turn at the beginning of the history
        let compacted_turn = BrainTurn {
            role: "system".to_string(),
            content: summary_content,
        };

        history.insert(0, compacted_turn);

        let tokens_after = Self::estimate_history_tokens(history);
        let tokens_saved = tokens_before.saturating_sub(tokens_after);

        CompactionOutcome {
            tokens_before,
            tokens_after,
            tokens_saved,
            was_compacted: true,
        }
    }

    /// Distills older turns into a high-density structured executive summary.
    fn distill_older_turns(turns: &[BrainTurn]) -> String {
        let mut user_requests = Vec::new();
        let mut assistant_actions = Vec::new();

        for turn in turns {
            let content = turn.content.trim();
            if content.is_empty() {
                continue;
            }

            match turn.role.as_str() {
                "user" => {
                    // Extract first line or clean sentence from user request
                    let first_line = content.lines().next().unwrap_or(content).trim();
                    let sanitized = if first_line.len() > 120 {
                        format!("{}…", &first_line[..117])
                    } else {
                        first_line.to_string()
                    };
                    if !user_requests.contains(&sanitized) {
                        user_requests.push(sanitized);
                    }
                }
                "assistant" => {
                    // Extract actions, touched files, or conclusions
                    let mut action_highlights = Vec::new();
                    for line in content.lines() {
                        let trimmed = line.trim();
                        let is_action = trimmed.starts_with("- Created `")
                            || trimmed.starts_with("- Edited `")
                            || trimmed.starts_with("- Renamed `")
                            || trimmed.starts_with("- Deleted `")
                            || trimmed.starts_with("### File:")
                            || trimmed.contains("fixed")
                            || trimmed.contains("resolved")
                            || trimmed.contains("implemented");
                        if is_action && !action_highlights.contains(&trimmed.to_string()) {
                            action_highlights.push(trimmed.to_string());
                        }
                    }

                    if action_highlights.is_empty() {
                        let first_line = content.lines().next().unwrap_or(content).trim();
                        let summary_line = if first_line.len() > 120 {
                            format!("{}…", &first_line[..117])
                        } else {
                            first_line.to_string()
                        };
                        assistant_actions.push(summary_line);
                    } else {
                        for act in action_highlights.into_iter().take(4) {
                            if !assistant_actions.contains(&act) {
                                assistant_actions.push(act);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let mut output = String::new();
        output.push_str("◈ [COMPACTED CONTEXT SUMMARY]\n");
        output.push_str("The preceding conversation has been automatically compacted for context optimization.\n\n");

        if !user_requests.is_empty() {
            output.push_str("Key User Inquiries & Goals:\n");
            for req in user_requests.iter().take(6) {
                output.push_str(&format!("• {}\n", req));
            }
            output.push('\n');
        }

        if !assistant_actions.is_empty() {
            output.push_str("Decisions, Actions & Modified Files:\n");
            for act in assistant_actions.iter().take(8) {
                output.push_str(&format!("• {}\n", act));
            }
        }

        output.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_compaction_on_short_history() {
        let mut history = vec![
            BrainTurn {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
            BrainTurn {
                role: "assistant".to_string(),
                content: "Hi there".to_string(),
            },
        ];

        let outcome = ContextCompactorService::auto_compact(&mut history);
        assert!(!outcome.was_compacted);
        assert_eq!(outcome.tokens_saved, 0);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_auto_compaction_triggered_by_turn_count() {
        let mut history = Vec::new();
        for i in 0..12 {
            history.push(BrainTurn {
                role: if i % 2 == 0 { "user".to_string() } else { "assistant".to_string() },
                content: format!("Message turn index {}", i),
            });
        }

        assert!(ContextCompactorService::should_compact(&history));
        let outcome = ContextCompactorService::auto_compact(&mut history);

        assert!(outcome.was_compacted);
        assert_eq!(history.len(), PRESERVED_RECENT_TURNS + 1);
        assert_eq!(history[0].role, "system");
        assert!(history[0].content.contains("COMPACTED CONTEXT SUMMARY"));
    }

    #[test]
    fn test_auto_compaction_triggered_by_token_threshold() {
        let long_payload = "A".repeat(35_000); // ~8,750 tokens
        let mut history = vec![
            BrainTurn { role: "user".to_string(), content: "Initial query".to_string() },
            BrainTurn { role: "assistant".to_string(), content: long_payload },
            BrainTurn { role: "user".to_string(), content: "Recent 1".to_string() },
            BrainTurn { role: "assistant".to_string(), content: "Recent 2".to_string() },
            BrainTurn { role: "user".to_string(), content: "Recent 3".to_string() },
            BrainTurn { role: "assistant".to_string(), content: "Recent 4".to_string() },
        ];

        assert!(ContextCompactorService::should_compact(&history));
        let outcome = ContextCompactorService::auto_compact(&mut history);

        assert!(outcome.was_compacted);
        assert!(outcome.tokens_saved > 8_000);
        assert_eq!(history.len(), PRESERVED_RECENT_TURNS + 1);
    }
}
