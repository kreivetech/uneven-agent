use serde::{Serialize, Deserialize};
use crate::infrastructure::patch::clean_code_fence;
use crate::domain::dashboard_spec::{DashboardSpec, NamedQuery};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedQuery {
    pub sql: String,
    pub queries: Vec<NamedQuery>,
    pub explanation: String,
    pub executive_report: String,
    pub cannot_generate: bool,
    pub visual_suggestions: Option<serde_json::Value>,
    pub dashboard_spec: Option<DashboardSpec>,
}

pub struct ResponseParser;

impl ResponseParser {
    pub fn parse(content: &str) -> GeneratedQuery {
        let text = content.trim();

        // 1. Try JSON parsing first ONLY if the root object explicitly contains "sql" or "cannotGenerate"
        if let Some(start) = text.find('{')
            && let Some(end) = text.rfind('}')
            && start < end
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&text[start..=end])
            && json.is_object()
            && (json.get("sql").is_some() || json.get("cannotGenerate").is_some())
        {
            return Self::from_json(json);
        }

        // 2. Text / Markdown Parsing
        Self::from_text(text)
    }

    fn from_json(json: serde_json::Value) -> GeneratedQuery {
        let sql = json.get("sql").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let explanation = json.get("explanation").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let executive_report = json.get("executiveReport")
            .or_else(|| json.get("report"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cannot_generate = json.get("cannotGenerate").and_then(|v| v.as_bool())
            .or_else(|| json.get("error").map(|_| true))
            .unwrap_or(false);
        let visual_suggestions = json.get("visualSuggestions")
            .or_else(|| json.get("visuals"))
            .cloned();

        let sql_clone = sql.clone();
        GeneratedQuery {
            sql,
            queries: if !sql_clone.is_empty() {
                vec![NamedQuery {
                    name: "dataset_main".to_string(),
                    sql: sql_clone,
                    description: None,
                }]
            } else {
                Vec::new()
            },
            explanation,
            executive_report,
            cannot_generate,
            visual_suggestions,
            dashboard_spec: None,
        }
    }

    fn from_text(text: &str) -> GeneratedQuery {
        if text.contains("CANNOT_GENERATE") {
            let reason = text.replace("CANNOT_GENERATE", "").trim().to_string();
            let final_reason = if reason.is_empty() {
                "LLM could not generate a query".to_string()
            } else {
                reason
            };
            return GeneratedQuery {
                sql: String::new(),
                explanation: final_reason,
                cannot_generate: true,
                ..Default::default()
            };
        }

        if text.contains("SECURITY_BLOCKED") {
            return GeneratedQuery {
                sql: String::new(),
                explanation: "Request blocked by security policy".to_string(),
                cannot_generate: true,
                ..Default::default()
            };
        }

        #[derive(PartialEq)]
        enum Section {
            None,
            Explanation,
            Report,
            Visual,
        }

        let mut current_section = Section::None;
        let mut explanation_lines = Vec::new();
        let mut report_lines = Vec::new();
        let mut visual_lines = Vec::new();
        let mut sql = String::new();
        let mut named_queries: Vec<NamedQuery> = Vec::new();
        let mut dashboard_yaml = String::new();

        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // 1. Code blocks (extract ```sql, ```sql:dataset_name, or ```dashboard)
            if trimmed.starts_with("```") {
                let tag = trimmed.trim_start_matches('`').trim().to_lowercase();
                let mut block_lines = Vec::new();
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with("```") {
                    block_lines.push(lines[i]);
                    i += 1;
                }
                let code_content = block_lines.join("\n").trim().to_string();

                if tag.starts_with("dashboard") || tag.starts_with("bi") {
                    dashboard_yaml = code_content;
                } else if tag.starts_with("sql") {
                    let dataset_name = if let Some((_, name)) = tag.split_once(':') {
                        name.trim().to_string()
                    } else if let Some((_, name)) = tag.split_once(' ') {
                        name.trim().trim_start_matches("name=").trim_matches('"').to_string()
                    } else if named_queries.is_empty() {
                        "dataset_main".to_string()
                    } else {
                        format!("dataset_{}", named_queries.len() + 1)
                    };

                    named_queries.push(NamedQuery {
                        name: dataset_name,
                        sql: code_content.clone(),
                        description: None,
                    });

                    if sql.is_empty() {
                        sql = code_content;
                    }
                } else {
                    let upper = code_content.to_uppercase();
                    if sql.is_empty()
                        && (upper.contains("SELECT") || upper.contains("WITH") || upper.contains("PRAGMA"))
                    {
                        sql = code_content.clone();
                        named_queries.push(NamedQuery {
                            name: "dataset_main".to_string(),
                            sql: code_content,
                            description: None,
                        });
                    }
                }

                if i < lines.len() {
                    i += 1;
                }
                continue;
            }

            // 2. Section detection (using line-by-line inspection like askf)
            let lower = trimmed.to_lowercase();
            if lower.starts_with("### explanation")
                || lower.starts_with("## explanation")
                || lower.starts_with("**explanation")
                || lower.starts_with("explanation:")
            {
                current_section = Section::Explanation;
                if let Some((_, rest)) = line.split_once(':') {
                    let r = rest.trim();
                    if !r.is_empty() && !r.starts_with('*') {
                        explanation_lines.push(r);
                    }
                }
                i += 1;
                continue;
            }

            if lower.starts_with("### report")
                || lower.starts_with("## report")
                || lower.starts_with("**report")
                || lower.starts_with("report:")
            {
                current_section = Section::Report;
                if let Some((_, rest)) = line.split_once(':') {
                    let r = rest.trim();
                    if !r.is_empty() && !r.starts_with('*') {
                        report_lines.push(r);
                    }
                }
                i += 1;
                continue;
            }

            if lower.starts_with("### visual")
                || lower.starts_with("## visual")
                || lower.starts_with("**visual")
                || lower.starts_with("visual:")
            {
                current_section = Section::Visual;
                if let Some((_, rest)) = line.split_once(':') {
                    let r = rest.trim();
                    if !r.is_empty() && !r.starts_with('*') {
                        visual_lines.push(r);
                    }
                }
                i += 1;
                continue;
            }

            // 3. Accumulate lines based on current section
            match current_section {
                Section::Explanation => explanation_lines.push(line),
                Section::Report => report_lines.push(line),
                Section::Visual => visual_lines.push(line),
                Section::None => {
                    let upper = trimmed.to_uppercase();
                    if !sql.is_empty()
                        || upper.starts_with("SELECT ")
                        || upper.starts_with("WITH ")
                        || upper.starts_with("PRAGMA ")
                    {
                        sql.push_str(line);
                        sql.push('\n');
                    }
                }
            }

            i += 1;
        }

        let mut final_sql = clean_code_fence(&sql).trim().to_string();
        let start_re = regex::Regex::new(r"(?i)\b(SELECT|WITH|PRAGMA)\b").unwrap();
        if let Some(m) = start_re.find(&final_sql) {
            final_sql = final_sql[m.start()..].trim().to_string();
        }

        let explanation = if explanation_lines.is_empty() {
            "Query generated from schema context".to_string()
        } else {
            explanation_lines.join("\n").trim().to_string()
        };

        let executive_report = report_lines.join("\n").trim().to_string();
        let visual_text = visual_lines.join("\n").trim().to_string();
        let visual_suggestions = Self::parse_visual_text(&visual_text);

        let dashboard_spec: Option<DashboardSpec> = if !dashboard_yaml.is_empty() {
            serde_json::from_str(&dashboard_yaml).ok()
        } else {
            None
        };

        GeneratedQuery {
            sql: final_sql,
            queries: named_queries,
            explanation,
            executive_report,
            cannot_generate: false,
            visual_suggestions,
            dashboard_spec,
        }
    }

    pub fn extract_theme(content: &str) -> Option<serde_json::Value> {
        let text = content.trim();
        if let Some(start) = text.find('{')
            && let Some(end) = text.rfind('}')
                && start < end
                    && let Ok(json) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) {
                        let mut map = serde_json::Map::new();
                        if let Some(title) = json.get("title") {
                            map.insert("title".to_string(), title.clone());
                        }
                        if let Some(theme) = json.get("themeMode").or_else(|| json.get("theme")) {
                            map.insert("themeMode".to_string(), theme.clone());
                        }
                        if let Some(layout) = json.get("layout") {
                            map.insert("layout".to_string(), layout.clone());
                        }
                        if let Some(colors) = json.get("colors") {
                            map.insert("colors".to_string(), colors.clone());
                        }
                        return Some(serde_json::Value::Object(map));
                    }

        let visual_re = regex::Regex::new(r"(?i)VISUAL:").unwrap();
        let parts: Vec<&str> = visual_re.split(text).collect();
        let mut visual_text = String::new();
        if parts.len() > 1 {
            visual_text = parts[1..].join(" ").trim().to_string();
        } else if text.contains("colors=") {
            visual_text = text.to_string();
        }

        if visual_text.is_empty() {
            None
        } else {
            Self::parse_visual_text(&visual_text)
        }
    }

    fn parse_visual_text(visual_text: &str) -> Option<serde_json::Value> {
        if visual_text.is_empty() {
            return None;
        }

        let title_re = regex::Regex::new(r"(?i)title=<(.*?)>|title=(.*?)(?:\s*\||$)").unwrap();
        let theme_re = regex::Regex::new(r"(?i)theme=<(.*?)>|theme=(.*?)(?:\s*\||$)").unwrap();
        let layout_re = regex::Regex::new(r"(?i)layout=(\[[\s\S]*?\])|layout=<(.*?)>").unwrap();
        let colors_re = regex::Regex::new(r"(?i)colors=<(.*?)>|colors=(.*?)(?:\s*\||$)").unwrap();

        let title = title_re.captures(visual_text)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| "Strategic Analysis".to_string());

        let theme_mode = theme_re.captures(visual_text)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .map(|m| m.as_str().trim().to_lowercase())
            .filter(|t| t == "light" || t == "dark");

        let colors_str = colors_re.captures(visual_text)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .map(|m| m.as_str().trim().to_string());
        
        let colors: Option<Vec<String>> = colors_str.map(|s| {
            s.split(',')
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .collect()
        });

        let layout_str = layout_re.captures(visual_text)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();

        let mut layout: Option<serde_json::Value> = None;
        if !layout_str.is_empty() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&layout_str)
                && val.is_array() {
                    layout = Some(val);
                }

            if layout.is_none() {
                // Fallback for custom formats: kpi:col,chart:bar:label:val
                let mut list = Vec::new();
                for p in layout_str.split(',') {
                    let p = p.trim();
                    if p.starts_with("kpi:") {
                        let col = p.strip_prefix("kpi:").unwrap_or(p);
                        list.push(serde_json::json!({
                            "type": "card",
                            "config": {
                                "metricCol": col,
                                "colors": colors,
                                "colSpan": 3
                            }
                        }));
                    } else if p.starts_with("chart:") {
                        let parts: Vec<&str> = p.split(':').collect();
                        if parts.len() >= 4 {
                            let chart_type = parts[1];
                            let label = parts[2];
                            let value = parts[3];
                            let size = parts.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(6);
                            list.push(serde_json::json!({
                                "type": "chart",
                                "config": {
                                    "chartType": chart_type,
                                    "labelCol": label,
                                    "valueCols": vec![value],
                                    "colors": colors,
                                    "title": value,
                                    "colSpan": size
                                }
                            }));
                        }
                    } else if p.starts_with("table") {
                        let parts: Vec<&str> = p.split(':').collect();
                        let size = parts.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(12);
                        list.push(serde_json::json!({
                            "type": "table",
                            "config": {
                                "title": "Detailed Explorer",
                                "colSpan": size
                            }
                        }));
                    }
                }
                if !list.is_empty() {
                    layout = Some(serde_json::Value::Array(list));
                }
            }
        }

        let mut map = serde_json::Map::new();
        map.insert("title".to_string(), serde_json::Value::String(title));
        if let Some(t) = theme_mode {
            map.insert("themeMode".to_string(), serde_json::Value::String(t));
        }
        if let Some(l) = layout {
            map.insert("layout".to_string(), l);
        }
        if let Some(cols) = colors {
            map.insert("colors".to_string(), serde_json::json!(cols));
        }

        Some(serde_json::Value::Object(map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_sql_response() {
        let md = r#"
```sql
SELECT department, COUNT(*) as count, SUM(salary) as total_salary
FROM employees
GROUP BY department
ORDER BY total_salary DESC
LIMIT 10;
```

### EXPLANATION
This query aggregates total salary and employee count per department.

### REPORT
Department level financial summary for quarterly planning.

### VISUAL
title=Department Salaries | layout=[{"type":"chart","config":{"chartType":"bar","labelCol":"department","valueCols":["total_salary"]}}] | colors=#10b981,#3b82f6
"#;

        let parsed = ResponseParser::parse(md);
        assert!(!parsed.cannot_generate);
        assert!(parsed.sql.starts_with("SELECT department"));
        assert!(parsed.sql.contains("GROUP BY department"));
        assert!(parsed.explanation.contains("This query aggregates"));
        assert!(parsed.executive_report.contains("Department level financial"));
        assert!(parsed.visual_suggestions.is_some());
    }

    #[test]
    fn test_parse_cannot_generate() {
        let text = "CANNOT_GENERATE Table does not exist in schema.";
        let parsed = ResponseParser::parse(text);
        assert!(parsed.cannot_generate);
        assert!(parsed.explanation.contains("Table does not exist"));
    }

    #[test]
    fn test_parse_security_blocked() {
        let text = "SECURITY_BLOCKED";
        let parsed = ResponseParser::parse(text);
        assert!(parsed.cannot_generate);
        assert!(parsed.explanation.contains("blocked by security policy"));
    }

    #[test]
    fn test_parse_multi_sql_and_dashboard_spec() {
        let md = r##"
### Explanation
Cross-table analysis comparing revenue per territory and budget variances.

### Report
Sales outperformed forecasts in North America while Marketing exceeded its operational allocation.

```sql:sales_summary
SELECT territory, SUM(amount) AS revenue, COUNT(id) AS orders
FROM sales
GROUP BY territory;
```

```sql:budget_overview
SELECT department, allocated_budget, spent_budget, (allocated_budget - spent_budget) AS variance
FROM budgets;
```

```dashboard
{
  "title": "Quarterly Performance & Budget Audit",
  "theme": "dark",
  "palette": ["#10b981", "#06b6d4", "#8b5cf6"],
  "layout": [
    {
      "type": "kpi",
      "dataset": "sales_summary",
      "metric": "revenue",
      "label": "Gross Sales",
      "format": "currency",
      "span": 6
    },
    {
      "type": "chart",
      "dataset": "sales_summary",
      "chartType": "bar",
      "title": "Revenue by Territory",
      "x": "territory",
      "y": "revenue",
      "span": 6
    }
  ]
}
```
"##;

        let parsed = ResponseParser::parse(md);
        assert!(!parsed.cannot_generate);
        assert_eq!(parsed.queries.len(), 2);
        assert_eq!(parsed.queries[0].name, "sales_summary");
        assert!(parsed.queries[0].sql.contains("FROM sales"));
        assert_eq!(parsed.queries[1].name, "budget_overview");
        assert!(parsed.queries[1].sql.contains("FROM budgets"));

        assert!(parsed.dashboard_spec.is_some());
        let spec = parsed.dashboard_spec.unwrap();
        assert_eq!(spec.title, "Quarterly Performance & Budget Audit");
        assert_eq!(spec.layout.len(), 2);
    }
}
