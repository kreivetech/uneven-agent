use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let code_str = error.code.as_deref().unwrap_or("");
    let is_asm = ["asm", "s"].contains(&ext.as_str()) || code_str == "ASM_SYNTAX";

    if !is_asm {
        return None;
    }

    let msg_lower = error.message.to_lowercase();

    // 1. Mismatched operand size
    if msg_lower.contains("size") || msg_lower.contains("mismatch") || msg_lower.contains("operand type") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "; Verify operand sizes match".to_string(),
            confidence: 0.3,
            append_eof: false,
            top_of_file: false,
            explanation: "Assembly: Instruction operand size mismatch".to_string(),
            suggested_fix: "Ensure register size (e.g., RAX vs EAX vs AX vs AL) matches the memory pointer or source operand size".to_string(),
        });
    }

    // 2. Invalid register
    if msg_lower.contains("register") || msg_lower.contains("invalid operand") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "; Check register naming or addressing mode".to_string(),
            confidence: 0.3,
            append_eof: false,
            top_of_file: false,
            explanation: "Assembly: Invalid register name or addressing mode".to_string(),
            suggested_fix: "Check for typos in register names or verify the addressing mode is supported by the target architecture".to_string(),
        });
    }

    // 3. Missing or undefined label/symbol
    if msg_lower.contains("undefined symbol") || msg_lower.contains("label") || msg_lower.contains("not declared") {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "; Ensure label is defined or declared extern".to_string(),
            confidence: 0.3,
            append_eof: false,
            top_of_file: false,
            explanation: "Assembly: Undefined symbol or label reference".to_string(),
            suggested_fix: "Verify the label is declared in the current file or exported/imported via global/extern directives".to_string(),
        });
    }

    if msg_lower.contains("invalid mnemonic") || msg_lower.contains("unrecognized instruction") {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            return Some(FixSuggestion {
                file: error.file.clone(),
                line: error.line,
                before: original_line.to_string(),
                after: format!("; {}", original_line),


                confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                explanation: "Assembly: Invalid mnemonic or instruction".to_string(),
                suggested_fix: "Commented out the invalid instruction".to_string(),
            });
        }
    }

    // 4. Generic Assembly compile error
    Some(FixSuggestion {
        file: error.file.clone(),
        line: error.line,
        before: "".to_string(),
        after: format!("; Review assembly syntax near line {}", error.line),
        confidence: 0.3,
            append_eof: false,
            top_of_file: false,
        explanation: format!("Assembly: Syntax or assembly error — {}", error.message),
        suggested_fix: format!("Check assembly instructions and alignment near line {}", error.line),
    })
}
