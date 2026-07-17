use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_sol = ext == "sol" || code_str == "SOLIDITY_ERR" || error.message.contains("solc");

    if !is_sol {
        return None;
    }

    // 1. Missing SPDX License Identifier
    if error.message.contains("SPDX license identifier") || error.message.contains("SPDX-License-Identifier") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: 1,
            before: "".to_string(),
            after: "// SPDX-License-Identifier: MIT\n".to_string(),


            confidence: 0.95,
            append_eof: false,
            top_of_file: true,
            explanation: "Solidity: SPDX license identifier is missing at the beginning of the source file.".to_string(),
            suggested_fix: "Injected // SPDX-License-Identifier: MIT at the top of the file".to_string(),
        });
    }

    // 2. Missing Visibility Specifier
    if error.message.contains("No visibility specified") || error.message.contains("Visibility must be") || error.message.contains("visibility") {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let target = ")";
            let replacement = ") public";
            if original_line.contains(target) {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: original_line.replace(target, replacement),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "Solidity: State variable or function is missing explicit visibility specifier.".to_string(),
                    suggested_fix: "Injected 'public' visibility specifier".to_string(),
                });
            }
        }
    }

    // 3. State Mutability Warning
    if error.message.contains("restricted to pure") || error.message.contains("restricted to view") {
        let is_pure = error.message.contains("pure");
        let kw = if is_pure { "pure" } else { "view" };
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: format!("// Example:\n// function myFunc() public {} {{\n// }}", kw),
            confidence: 0.2,
            append_eof: false,
            top_of_file: false,
            explanation: format!("Solidity: Function state mutability can be restricted to {}.", kw),
            suggested_fix: format!("Add the \"{}\" keyword to the function signature", kw),
        });
    }

    None
}
