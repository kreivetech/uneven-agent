use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NamedQuery {
    pub name: String,
    pub sql: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DashboardTheme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DashboardComponentSpec {
    #[serde(rename = "kpi")]
    Kpi {
        #[serde(default)]
        dataset: Option<String>,
        metric: String,
        label: Option<String>,
        #[serde(default)]
        format: Option<String>, // "currency", "integer", "percent"
        #[serde(default = "default_kpi_span")]
        span: i64,
        #[serde(default)]
        icon: Option<String>,
    },
    #[serde(rename = "chart")]
    Chart {
        #[serde(default)]
        dataset: Option<String>,
        #[serde(rename = "chartType", default = "default_chart_type")]
        chart_type: String, // "bar", "line", "area", "donut", "pie", "combo"
        title: Option<String>,
        subtitle: Option<String>,
        #[serde(default)]
        x: Option<String>,
        #[serde(default)]
        y: Option<serde_json::Value>, // String or Vec<String>
        #[serde(default = "default_chart_span")]
        span: i64,
        #[serde(default = "default_chart_height")]
        height: i64,
        #[serde(default)]
        colors: Option<Vec<String>>,
    },
    #[serde(rename = "table")]
    Table {
        #[serde(default)]
        dataset: Option<String>,
        title: Option<String>,
        subtitle: Option<String>,
        #[serde(default = "default_table_span")]
        span: i64,
        #[serde(default = "default_pagination")]
        pagination: usize,
    },
    #[serde(rename = "explanation")]
    Explanation {
        text: String,
        #[serde(default = "default_table_span")]
        span: i64,
    },
}

fn default_kpi_span() -> i64 { 3 }
fn default_chart_span() -> i64 { 6 }
fn default_chart_height() -> i64 { 360 }
fn default_table_span() -> i64 { 12 }
fn default_pagination() -> usize { 10 }
fn default_chart_type() -> String { "bar".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSpec {
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub theme: DashboardTheme,
    #[serde(default)]
    pub palette: Option<Vec<String>>,
    #[serde(default)]
    pub layout: Vec<DashboardComponentSpec>,
}
