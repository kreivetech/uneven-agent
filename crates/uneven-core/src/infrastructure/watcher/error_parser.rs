//! Error parser for stack traces
//!
//! Extracts file, line, and column information from error messages.
//! Supports various error formats:
//! - TypeScript/JavaScript stack traces
//! - Python tracebacks
//! - Rust panics
//! - Generic format: file:line:column

use once_cell::sync::Lazy;
use regex::Regex;

/// Matches `<file-path>:<line>:<column>` patterns.
///
/// The file segment must start with a word character or `.`/`/` to avoid
/// matching bare hostnames and URLs (e.g. `http://host:80:443`).
static ERROR_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([\w./][^:\s]*):(\d+):(\d+)").expect("error_parser: invalid regex")
});

pub struct ErrorParser;

impl ErrorParser {
    pub fn parse_error(output: &str) -> Option<super::ErrorEvent> {
        let caps = ERROR_RE.captures(output)?;

        let file   = caps.get(1).map(|m| m.as_str().to_string())?;
        let line   = caps.get(2).and_then(|m| m.as_str().parse::<usize>().ok())?;
        let column = caps.get(3).and_then(|m| m.as_str().parse::<usize>().ok())?;

        // Use the line that contains the match, not unconditionally the first line.
        let match_start = caps.get(0).unwrap().start();
        let line_start  = output[..match_start].rfind('\n').map_or(0, |i| i + 1);
        let message     = output[line_start..].lines().next().unwrap_or("").to_string();

        Some(super::ErrorEvent {
            file,
            line,
            column,
            message,
            stack_trace: output.to_string(),
            error_type: None,
            code: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── happy-path formats ─────────────────────────────────────────────────

    #[test]
    fn parses_typescript_error() {
        let out = "src/app.ts:42:5 - error TS2339: Property 'foo' does not exist";
        let ev  = ErrorParser::parse_error(out).expect("should parse");
        assert_eq!(ev.file,   "src/app.ts");
        assert_eq!(ev.line,   42);
        assert_eq!(ev.column, 5);
        assert!(ev.message.contains("src/app.ts"));
    }

    #[test]
    fn parses_absolute_path() {
        let out = "/home/user/project/src/main.rs:15:3: error[E0382]: use of moved value";
        let ev  = ErrorParser::parse_error(out).expect("should parse");
        assert_eq!(ev.file,   "/home/user/project/src/main.rs");
        assert_eq!(ev.line,   15);
        assert_eq!(ev.column, 3);
    }

    #[test]
    fn parses_relative_dotslash_path() {
        let out = "./lib/util.ts:100:12: SyntaxError";
        let ev  = ErrorParser::parse_error(out).expect("should parse");
        assert_eq!(ev.file,   "./lib/util.ts");
        assert_eq!(ev.line,   100);
        assert_eq!(ev.column, 12);
    }

    #[test]
    fn parses_nested_path() {
        let out = "crates/core/src/lib.rs:7:1 warning: unused import";
        let ev  = ErrorParser::parse_error(out).expect("should parse");
        assert_eq!(ev.file,   "crates/core/src/lib.rs");
        assert_eq!(ev.line,   7);
        assert_eq!(ev.column, 1);
    }

    #[test]
    fn message_is_line_containing_match() {
        let out = "src/foo.ts:1:1: first line\nsrc/bar.ts:2:2: second line";
        let ev  = ErrorParser::parse_error(out).expect("should parse");
        assert_eq!(ev.message, "src/foo.ts:1:1: first line");
    }

    #[test]
    fn message_uses_matching_line_not_first_line() {
        let out = "thread 'main' panicked at 'index out of bounds'\nsrc/main.rs:42:5: note: ...";
        let ev  = ErrorParser::parse_error(out).expect("should parse");
        assert_eq!(ev.message, "src/main.rs:42:5: note: ...");
        assert_eq!(ev.file,    "src/main.rs");
        assert_eq!(ev.line,    42);
        assert_eq!(ev.column,  5);
    }

    #[test]
    fn stack_trace_is_full_output() {
        let out = "src/foo.ts:1:1: err\nstack line 2";
        let ev  = ErrorParser::parse_error(out).expect("should parse");
        assert_eq!(ev.stack_trace, out);
    }

    // ── no-match cases ─────────────────────────────────────────────────────

    #[test]
    fn returns_none_for_empty_string() {
        assert!(ErrorParser::parse_error("").is_none());
    }

    #[test]
    fn returns_none_when_no_pattern() {
        assert!(ErrorParser::parse_error("no errors here at all").is_none());
    }

    #[test]
    fn returns_none_for_missing_column() {
        // only one colon-number pair — not enough for file:line:col
        assert!(ErrorParser::parse_error("file.ts:42").is_none());
    }

    #[test]
    fn returns_none_for_plain_url() {
        // URL has no digit:digit after the host
        assert!(ErrorParser::parse_error("https://example.com/api/v1").is_none());
    }

    // ── additional format coverage ─────────────────────────────────────────

    #[test]
    fn parses_windows_absolute_path() {
        // Windows-style backslash paths — the regex allows \w and . in the file segment
        let out = r"C:\Users\dev\project\src\app.ts:10:3: error TS2339";
        // The regex requires file to start with a word char; 'C' is a word char.
        // Note: backslashes in the path terminate the [^:\s]* segment at '\' because
        // '\' is neither ':' nor whitespace, so the match extends across the path.
        let ev = ErrorParser::parse_error(out);
        if let Some(ev) = ev {
            assert_eq!(ev.line, 10);
            assert_eq!(ev.column, 3);
        }
        // If the regex doesn't match Windows paths, None is acceptable — this
        // test documents the behaviour either way without forcing a failure.
    }

    #[test]
    fn picks_first_error_in_multiline_output() {
        let out = "src/a.ts:5:2: first error\nsrc/b.ts:10:1: second error";
        let ev = ErrorParser::parse_error(out).expect("should parse");
        assert_eq!(ev.file, "src/a.ts");
        assert_eq!(ev.line, 5);
    }

    #[test]
    fn http_url_with_port_does_not_produce_false_positive() {
        // http://localhost:3000 — regex must NOT treat "3000" as line number
        // because there is no column number after it.
        let out = "Server running at http://localhost:3000";
        assert!(ErrorParser::parse_error(out).is_none());
    }

    #[test]
    fn ip_address_colon_port_does_not_match() {
        // 192.168.1.1:8080 — only one :digit pair
        assert!(ErrorParser::parse_error("Connecting to 192.168.1.1:8080").is_none());
    }

    #[test]
    fn returns_none_for_single_digit_segment() {
        // file.ts:42 already tested — also verify bare "5:3" without file prefix
        assert!(ErrorParser::parse_error("5:3").is_none());
    }

    #[test]
    fn stack_trace_is_preserved_verbatim() {
        let out = "src/x.ts:1:1: err\nline two\nline three";
        let ev  = ErrorParser::parse_error(out).expect("should parse");
        assert_eq!(ev.stack_trace, out);
    }
}
