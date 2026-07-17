use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedQuery {
    pub sql: String,
    pub explanation: String,
    pub executive_report: String,
    pub cannot_generate: bool,
    pub visual_suggestions: Option<serde_json::Value>,
}

pub struct ResponseParser;

impl ResponseParser {
    pub fn parse(content: &str) -> GeneratedQuery {
        let text = content.trim();

        // 1. Try JSON parsing first (Cloud Native Mode)
        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                if start < end {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) {
                        return Self::from_json(json);
                    }
                }
            }
        }

        // 2. Text Parsing (Local / Snatchy)
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

        GeneratedQuery {
            sql,
            explanation,
            executive_report,
            cannot_generate,
            visual_suggestions,
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

        let report_re = regex::Regex::new(r"(?i)REPORT:").unwrap();
        let explanation_re = regex::Regex::new(r"(?i)EXPLANATION:").unwrap();
        let visual_re = regex::Regex::new(r"(?i)VISUAL:").unwrap();

        let parts: Vec<&str> = report_re.split(text).collect();
        let (main_part, executive_report) = if parts.len() > 1 {
            (parts[0], parts[1..].join(" ").trim().to_string())
        } else {
            (text, String::new())
        };

        let parts: Vec<&str> = explanation_re.split(main_part).collect();
        let (sql_and_visuals, explanation) = if parts.len() > 1 {
            (parts[0], parts[1..].join(" ").trim().to_string())
        } else {
            (main_part, "Query generated from schema context".to_string())
        };

        let parts: Vec<&str> = visual_re.split(sql_and_visuals).collect();
        let (sql_part, visual_parts) = if parts.len() > 1 {
            (parts[0], parts[1..].join(" ").trim().to_string())
        } else {
            (sql_and_visuals, String::new())
        };

        // Clean SQL
        let sql_fence_re = regex::Regex::new(r"(?i)```sql\n?|```").unwrap();
        let mut sql = sql_fence_re.replace_all(sql_part, "").trim().to_string();
        
        let start_re = regex::Regex::new(r"(?i)\b(SELECT|WITH|PRAGMA)\b").unwrap();
        if let Some(m) = start_re.find(&sql) {
            sql = sql[m.start()..].trim().to_string();
        }

        let visual_suggestions = Self::parse_visual_text(&visual_parts);

        GeneratedQuery {
            sql,
            explanation,
            executive_report,
            cannot_generate: false,
            visual_suggestions,
        }
    }

    pub fn extract_theme(content: &str) -> Option<serde_json::Value> {
        let text = content.trim();
        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                if start < end {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) {
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
                }
            }
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
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&layout_str) {
                if val.is_array() {
                    layout = Some(val);
                }
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
