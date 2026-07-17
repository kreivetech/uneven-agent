/// Formats text content by prefixing each line with its 1-indexed line number (padded to 4 spaces).
pub fn format_with_line_numbers(content: &str) -> String {
    format_with_line_numbers_starting_at(content, 1)
}

/// Formats text content by prefixing each line with line numbers starting at `start_line` (padded to 4 spaces).
pub fn format_with_line_numbers_starting_at(content: &str, start_line: usize) -> String {
    let mut formatted = String::new();
    for (line_idx, line) in content.lines().enumerate() {
        formatted.push_str(&format!("{:>4} {}\n", start_line + line_idx, line));
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_with_line_numbers_basic() {
        let content = "line one\nline two\nline three";
        let formatted = format_with_line_numbers(content);
        let expected = "   1 line one\n   2 line two\n   3 line three\n";
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_with_line_numbers_starting_at() {
        let content = "line ten\nline eleven";
        let formatted = format_with_line_numbers_starting_at(content, 10);
        let expected = "  10 line ten\n  11 line eleven\n";
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_with_line_numbers_empty() {
        let content = "";
        let formatted = format_with_line_numbers(content);
        assert_eq!(formatted, "");
    }
}
