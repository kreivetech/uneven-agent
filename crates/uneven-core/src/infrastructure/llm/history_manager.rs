//! History Context Optimizer
//! Manages conversation history dynamically by avoiding redundant injection
//! of system laws and project file maps.

pub trait HistoryItem {
    fn get_role(&self) -> &str;
    fn get_content(&self) -> &str;
    fn set_content(&mut self, content: String);
}

impl HistoryItem for (String, String) {
    fn get_role(&self) -> &str { &self.0 }
    fn get_content(&self) -> &str { &self.1 }
    fn set_content(&mut self, content: String) { self.1 = content; }
}

const LAW_START: &str = "SYSTEM_LAWS_ACTIVE";
const LAW_END: &str = "SYSTEM_LAWS_ACTIVE_END";
const MAP_START: &str = "PROJECT_FILES_MAP_ACTIVE";
const MAP_END: &str = "PROJECT_FILES_MAP_ACTIVE_END";

fn extract_content_between(text: &str, start_tag: &str, end_tag: &str) -> Option<String> {
    let start_idx = text.find(start_tag)? + start_tag.len();
    let end_idx = text.find(end_tag)?;
    if start_idx < end_idx {
        Some(text[start_idx..end_idx].trim().to_string())
    } else {
        None
    }
}

fn update_content_between(text: &str, start_tag: &str, end_tag: &str, new_content: &str) -> Option<String> {
    let start_idx = text.find(start_tag)?;
    let end_idx = text.find(end_tag)? + end_tag.len();
    if start_idx < end_idx {
        let mut updated = text[..start_idx].to_string();
        updated.push_str(start_tag);
        updated.push('\n');
        updated.push_str(new_content);
        updated.push('\n');
        updated.push_str(end_tag);
        updated.push_str(&text[end_idx..]);
        Some(updated)
    } else {
        None
    }
}

/// Optimizes the prompt to avoid redundant system rules (laws) and file maps.
/// If laws or file maps are already present in the active conversation history,
/// they are skipped in the current prompt. If the files map in the history is
/// outdated compared to the current workspace, it is updated in-place.
pub fn optimize_context<T: HistoryItem>(
    history: &mut [T],
    prompt: &str,
    laws: &str,
    files_map: Option<&str>,
) -> String {
    let mut has_laws = false;
    let mut has_map = false;

    // 1. Scan the history for existing tags
    for item in history.iter() {
        let content = item.get_content();
        if content.contains(LAW_START) && content.contains(LAW_END) {
            has_laws = true;
        }
        if content.contains(MAP_START) && content.contains(MAP_END) {
            has_map = true;
        }
    }

    // 2. If map exists, check if it needs update in-place in the history
    if let Some(new_map) = files_map {
        if has_map {
            for item in history.iter_mut() {
                let content = item.get_content();
                if content.contains(MAP_START) && content.contains(MAP_END) {
                    if let Some(old_map) = extract_content_between(content, MAP_START, MAP_END) {
                        if old_map != new_map {
                            if let Some(updated_content) = update_content_between(content, MAP_START, MAP_END, new_map) {
                                item.set_content(updated_content);
                                tracing::info!("HistoryOptimizer: Updated project files map in-place in conversation history.");
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    // 3. Assemble the prompt wrapper for the current message
    let mut prefix = String::new();

    if !has_laws {
        prefix.push_str(LAW_START);
        prefix.push('\n');
        prefix.push_str(laws);
        prefix.push('\n');
        prefix.push_str(LAW_END);
        prefix.push_str("\n\n");
    }

    if let Some(new_map) = files_map {
        if !has_map {
            prefix.push_str(MAP_START);
            prefix.push('\n');
            prefix.push_str(new_map);
            prefix.push('\n');
            prefix.push_str(MAP_END);
            prefix.push_str("\n\n");
        }
    }

    if prefix.is_empty() {
        prompt.to_string()
    } else {
        format!("{}{}", prefix, prompt)
    }
}
