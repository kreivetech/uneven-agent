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

/// Escapes HTML special characters to prevent XSS and formatting corruption
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

/// Checks whether a JSON value is a number or can be parsed as a floating point number
pub fn is_numeric(v: &serde_json::Value) -> bool {
    v.is_number() || (v.is_string() && v.as_str().and_then(|s| s.parse::<f64>().ok()).is_some())
}

/// Determines whether a column or value appears to represent a timestamp or date
pub fn is_date_like(col: &str, v: &serde_json::Value) -> bool {
    let name_hint = col.to_lowercase().ends_with("date")
        || col.to_lowercase().ends_with("time")
        || col.to_lowercase().ends_with("at")
        || col.to_lowercase().ends_with("day")
        || col.to_lowercase().contains("created")
        || col.to_lowercase().contains("updated");
    let val_hint = v.is_string() && v.as_str().map(|s| s.len() >= 10 && s[0..4].chars().all(|c| c.is_ascii_digit())).unwrap_or(false);
    name_hint || val_hint
}

/// Converts identifier or column name (e.g. total_revenue, user_count) into human readable title
pub fn humanize_identifier(s: &str) -> String {
    let clean = s.trim_start_matches("u.").trim_start_matches("o.").trim_start_matches("i.");
    let mut out = String::new();
    let mut prev_char: Option<char> = None;
    for c in clean.chars() {
        if c == '_' {
            out.push(' ');
        } else if c.is_uppercase() {
            if let Some(prev) = prev_char
                && !prev.is_whitespace() && !prev.is_uppercase() {
                    out.push(' ');
                }
            out.push(c);
        } else {
            out.push(c);
        }
        prev_char = Some(c);
    }
    
    out.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// Checks whether a column name represents a monetary or financial metric
pub fn is_money_col(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("price") || n.contains("total") || n.contains("revenue") || n.contains("salary")
        || n.contains("amount") || n.contains("tax") || n.contains("cost") || n.contains("fee")
        || n.contains("sum") || n.contains("balance") || n.contains("vlr") || n.contains("valor")
        || n.contains("pagamento")
}

/// Formats a generic JSON value for display in reports, tables, and KPIs with localized currency & precision
pub fn format_report_value(v: &serde_json::Value, col: &str) -> String {
    if v.is_null() {
        return "-".to_string();
    }
    
    let num_val = if v.is_number() {
        v.as_f64()
    } else if v.is_string() {
        v.as_str().and_then(|s| s.parse::<f64>().ok())
    } else {
        None
    };

    if let Some(n) = num_val {
        if is_money_col(col) {
            let is_brl = col.to_lowercase().contains("vlr")
                || col.to_lowercase().contains("valor")
                || col.to_lowercase().contains("real")
                || col.to_lowercase().contains("brl")
                || col.to_lowercase().contains("pagamento");
            let symbol = if is_brl { "R$" } else { "$" };
            return format!("{} {:.2}", symbol, n);
        }
        if n.fract() == 0.0 && n.abs() < 1e15 {
            return format!("{}", n as i64);
        }
        return format!("{:.2}", n);
    }

    if v.is_boolean() {
        return v.as_bool().unwrap().to_string();
    }

    v.as_str().unwrap_or(&v.to_string()).to_string()
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
    fn test_escape_html() {
        assert_eq!(escape_html("<script>alert('x')</script>"), "&lt;script&gt;alert(&#039;x&#039;)&lt;/script&gt;");
    }

    #[test]
    fn test_humanize_identifier() {
        assert_eq!(humanize_identifier("total_gross_revenue"), "Total Gross Revenue");
        assert_eq!(humanize_identifier("orderCount"), "Order Count");
    }

    #[test]
    fn test_format_report_value() {
        assert_eq!(format_report_value(&serde_json::json!(450000.5), "revenue"), "$ 450000.50");
        assert_eq!(format_report_value(&serde_json::json!(1240.0), "orders"), "1240");
        assert_eq!(format_report_value(&serde_json::Value::Null, "status"), "-");
    }
}
