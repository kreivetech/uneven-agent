use crate::infrastructure::watcher::ErrorEvent;
use crate::domain::parsed_error::ErrorType;

/// Runs a fast, in-memory syntax check to avoid calling the AI for clean files
/// Returns an ErrorEvent if a syntax error is found, otherwise None.
pub fn fast_syntax_check(content: &str, file_path: &str) -> Option<ErrorEvent> {
    if content.trim().is_empty() {
        return None;
    }

    let mut open_braces = 0;
    let mut open_parens = 0;
    let mut open_brackets = 0;
    
    let mut in_string = false;
    let mut string_char = ' ';
    let mut in_comment_line = false;
    let mut in_comment_block = false;

    let chars: Vec<char> = content.chars().collect();
    let mut line_num = 1;

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next_c = if i + 1 < chars.len() { chars[i + 1] } else { ' ' };

        if c == '\n' {
            line_num += 1;
            in_comment_line = false;
        }

        if in_comment_block {
            if c == '*' && next_c == '/' {
                in_comment_block = false;
                i += 1;
            }
            i += 1;
            continue;
        }

        if in_comment_line {
            i += 1;
            continue;
        }

        if !in_string {
            if c == '/' && next_c == '/' {
                in_comment_line = true;
                i += 1;
                i += 1;
                continue;
            }
            if c == '/' && next_c == '*' {
                in_comment_block = true;
                i += 1;
                i += 1;
                continue;
            }
        }

        if c == '"' || c == '\'' || c == '`' {
            if !in_string {
                in_string = true;
                string_char = c;
            } else if string_char == c {
                let mut escapes = 0;
                let mut b = i.saturating_sub(1);
                while b > 0 && chars[b] == '\\' {
                    escapes += 1;
                    b -= 1;
                }
                if escapes % 2 == 0 {
                    in_string = false;
                }
            }
        }

        if !in_string {
            match c {
                '{' => open_braces += 1,
                '}' => open_braces -= 1,
                '(' => open_parens += 1,
                ')' => open_parens -= 1,
                '[' => open_brackets += 1,
                ']' => open_brackets -= 1,
                _ => {}
            }
        }

        if open_braces < 0 {
            return Some(ErrorEvent {
                file: file_path.to_string(),
                line: line_num,
                column: 0,
                message: "SyntaxError: Unexpected token '}'".to_string(),
                stack_trace: "SyntaxError".to_string(),
                error_type: Some(ErrorType::Compiler),
                code: Some("SYNTAX".to_string()),
            });
        }
        if open_parens < 0 {
            return Some(ErrorEvent {
                file: file_path.to_string(),
                line: line_num,
                column: 0,
                message: "SyntaxError: Unexpected token ')'".to_string(),
                stack_trace: "SyntaxError".to_string(),
                error_type: Some(ErrorType::Compiler),
                code: Some("SYNTAX".to_string()),
            });
        }
        if open_brackets < 0 {
            return Some(ErrorEvent {
                file: file_path.to_string(),
                line: line_num,
                column: 0,
                message: "SyntaxError: Unexpected token ']'".to_string(),
                stack_trace: "SyntaxError".to_string(),
                error_type: Some(ErrorType::Compiler),
                code: Some("SYNTAX".to_string()),
            });
        }

        i += 1;
    }

    if open_braces > 0 {
        return Some(ErrorEvent {
            file: file_path.to_string(),
            line: line_num,
            column: 0,
            message: "SyntaxError: Unexpected end of input, expected '}'".to_string(),
            stack_trace: "SyntaxError".to_string(),
            error_type: Some(ErrorType::Compiler),
            code: Some("SYNTAX".to_string()),
        });
    }

    if open_parens > 0 {
        return Some(ErrorEvent {
            file: file_path.to_string(),
            line: line_num,
            column: 0,
            message: "SyntaxError: Unexpected end of input, expected ')'".to_string(),
            stack_trace: "SyntaxError".to_string(),
            error_type: Some(ErrorType::Compiler),
            code: Some("SYNTAX".to_string()),
        });
    }

    None
}
