#![allow(dead_code)]

use std::time::Duration;

/// Formats execution duration for display in the UI status bar.
pub fn format_duration(d: Duration) -> String {
    let secs_f = d.as_secs_f64();
    if secs_f < 1.0 {
        let ms = (secs_f * 1000.0).round() as u64;
        format!("{}ms", ms)
    } else if secs_f < 60.0 {
        let secs = secs_f.round() as u64;
        format!("{}s", secs)
    } else {
        let total_secs = secs_f.round() as u64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if secs == 0 {
            format!("{}m", mins)
        } else {
            format!("{}m {}s", mins, secs)
        }
    }
}

/// Formats currency values in USD.
pub fn format_usd(val: f64) -> String {
    format!("${:.5}", val)
}

/// Formats large numbers with thousand separators.
pub fn format_number(val: usize) -> String {
    let s = val.to_string();
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, c) in chars.into_iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Strips markdown syntax like `**` and `###` from text if plain text is needed.
pub fn strip_markdown_markers(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let cleaned_line = if trimmed.starts_with('#') {
            trimmed.trim_start_matches('#').trim()
        } else {
            line
        };
        let without_bold = cleaned_line.replace("**", "").replace("__", "");
        out.push_str(&without_bold);
        out.push('\n');
    }
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_markdown_markers() {
        let input = "### Title\nThis is **very important** and __relevant__.\n#### Subtitle";
        let cleaned = strip_markdown_markers(input);
        assert!(!cleaned.contains("###"));
        assert!(!cleaned.contains("####"));
        assert!(!cleaned.contains("**"));
        assert!(!cleaned.contains("__"));
        assert!(cleaned.contains("Title"));
        assert!(cleaned.contains("very important"));
    }
}
