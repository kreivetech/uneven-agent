use std::collections::HashMap;
use crate::infrastructure::utils::formatter::{
    escape_html as esc, format_report_value, humanize_identifier, is_date_like, is_numeric,
};

pub const DASHBOARD_CSS: &str = include_str!("assets/dashboard.css");
pub const ICONIFY_JS: &str = include_str!("assets/iconify-icon.min.js");

pub fn generate_multi_dashboard(
    datasets: &HashMap<String, Vec<serde_json::Value>>,
    title: &str,
    theme: &str,
    visual_suggestions: Option<&serde_json::Value>,
) -> String {
    if datasets.is_empty() {
        return generate_html_report(title, "<div class='p-8 text-center text-neutral-400'>No datasets returned from query.</div>");
    }

    let date = chrono::Local::now().format("%b %d, %Y - %H:%M").to_string();
    let theme_class = if theme == "light" { "light-mode" } else { "" };

    // 1. Semantic Tabs Preparation
    let mut tab_buttons = Vec::new();
    let mut tab_contents = Vec::new();

    let primary_dataset_name = datasets.keys().next().cloned().unwrap_or_else(|| "main".to_string());

    // Tab 1: Executive Overview
    tab_buttons.push(format!(
        r#"<button onclick="switchTab('tab-overview')" data-tab-btn="tab-overview" class="tab-btn active px-4 py-2 rounded-xl text-xs font-semibold flex items-center gap-2 transition-all cursor-pointer bg-emerald-500/10 text-emerald-400 border border-emerald-500/20"><iconify-icon icon="lucide:layout-dashboard" class="text-sm"></iconify-icon>Executive Overview</button>"#
    ));

    let overview_html = render_overview_tab(datasets, &primary_dataset_name, visual_suggestions, theme);
    tab_contents.push(format!(
        r#"<section id="tab-overview" class="tab-pane flex flex-col gap-6">{}</section>"#,
        overview_html
    ));

    // Tab 2: Data Explorer (Multi-table support)
    tab_buttons.push(format!(
        r#"<button onclick="switchTab('tab-explorer')" data-tab-btn="tab-explorer" class="tab-btn px-4 py-2 rounded-xl text-xs font-semibold flex items-center gap-2 transition-all cursor-pointer text-neutral-400 hover:text-white border border-transparent"><iconify-icon icon="lucide:database" class="text-sm"></iconify-icon>Data Explorer ({} tables)</button>"#,
        datasets.len()
    ));

    let explorer_html = render_multi_table_explorer(datasets);
    tab_contents.push(format!(
        r#"<section id="tab-explorer" class="tab-pane hidden flex flex-col gap-6">{}</section>"#,
        explorer_html
    ));

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{}</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <script src="https://cdn.jsdelivr.net/npm/apexcharts"></script>
  <script>{}</script>
  <style>{}</style>
</head>
<body class="{}">
  <div class="dashboard-shell">
    <!-- Top Executive Header -->
    <header class="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-[var(--border)]">
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-xl bg-emerald-950/80 border border-emerald-600/40 flex items-center justify-center text-emerald-400 shadow-sm shrink-0">
          <iconify-icon icon="lucide:bar-chart-3" class="text-xl"></iconify-icon>
        </div>
        <div>
          <div class="flex items-center gap-2">
            <h1 class="text-lg font-bold text-neutral-100 tracking-tight">{}</h1>
            <span class="px-2 py-0.5 rounded text-[10px] font-mono font-medium bg-emerald-950/80 border border-emerald-600/40 text-emerald-400 flex items-center gap-1">
              <iconify-icon icon="lucide:shield-check" class="text-xs"></iconify-icon>
              Safe Blind Analytics
            </span>
          </div>
          <p class="text-xs text-neutral-400 font-mono flex items-center gap-1 mt-0.5">
            <iconify-icon icon="lucide:calendar" class="text-xs"></iconify-icon>
            Generated on {}
          </p>
        </div>
      </div>

      <div class="flex items-center gap-2 no-print">
        <button onclick="toggleTheme()" class="px-3 py-1.5 rounded-xl bg-[#14161f] hover:bg-[#1a1e28] border border-[#232734] text-neutral-300 hover:text-white text-xs font-mono transition-all flex items-center gap-1.5 cursor-pointer shadow-sm active:scale-95">
          <iconify-icon id="theme-toggle-icon" icon="lucide:sun-moon" class="text-xs"></iconify-icon>
          Theme
        </button>
        <button onclick="window.print()" class="px-3 py-1.5 rounded-xl bg-[#14161f] hover:bg-[#1a1e28] border border-[#232734] text-neutral-300 hover:text-white text-xs font-mono transition-all flex items-center gap-1.5 cursor-pointer shadow-sm active:scale-95">
          <iconify-icon icon="lucide:printer" class="text-xs"></iconify-icon>
          Print / PDF
        </button>
      </div>
    </header>

    <!-- Navigation Tabs Bar -->
    <nav class="flex items-center gap-2 no-print overflow-x-auto pb-1">
      {}
    </nav>

    <!-- Tab Sections Container -->
    <main class="flex-1 flex flex-col gap-6">
      {}
    </main>
  </div>

  <script>
    function switchTab(tabId) {{
      document.querySelectorAll('.tab-pane').forEach(el => el.classList.add('hidden'));
      document.querySelectorAll('.tab-btn').forEach(btn => {{
        btn.classList.remove('bg-emerald-500/10', 'text-emerald-400', 'border-emerald-500/20');
        btn.classList.add('text-neutral-400', 'border-transparent');
      }});

      const targetPane = document.getElementById(tabId);
      if (targetPane) targetPane.classList.remove('hidden');

      const targetBtn = document.querySelector(`[data-tab-btn="${{tabId}}"]`);
      if (targetBtn) {{
        targetBtn.classList.remove('text-neutral-400', 'border-transparent');
        targetBtn.classList.add('bg-emerald-500/10', 'text-emerald-400', 'border-emerald-500/20');
      }}
    }}

    // Table search filter
    function filterTable(tableId, query) {{
      const q = query.toLowerCase();
      const rows = document.querySelectorAll(`#${{tableId}} tbody tr`);
      rows.forEach(r => {{
        r.style.display = r.textContent.toLowerCase().includes(q) ? '' : 'none';
      }});
    }}

    function toggleTheme() {{
      document.body.classList.toggle('light-mode');
    }}
  </script>
</body>
</html>"#,
        esc(title),
        ICONIFY_JS,
        DASHBOARD_CSS,
        theme_class,
        esc(title),
        date,
        tab_buttons.join("\n"),
        tab_contents.join("\n")
    )
}

pub fn generate_dashboard(
    results: &[serde_json::Value],
    title: &str,
    theme: &str,
    visual_suggestions: Option<&serde_json::Value>,
) -> String {
    let mut datasets = HashMap::new();
    datasets.insert("dataset_main".to_string(), results.to_vec());
    generate_multi_dashboard(&datasets, title, theme, visual_suggestions)
}

pub fn generate_html_report(title: &str, content_html: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>{}</title>
  <style>{}</style>
</head>
<body class="p-8">
  {}
</body>
</html>"#,
        esc(title), DASHBOARD_CSS, content_html
    )
}

struct ChartOverride {
    chart_type: Option<String>,
    title: Option<String>,
    subtitle: Option<String>,
    span: Option<usize>,
    height: Option<usize>,
}

fn parse_layout_items(visual_suggestions: Option<&serde_json::Value>) -> Option<&Vec<serde_json::Value>> {
    let val = visual_suggestions?;
    if let Some(arr) = val.as_array() {
        return (!arr.is_empty()).then_some(arr);
    }
    if let Some(arr) = val.get("layout").and_then(|v| v.as_array()) {
        return (!arr.is_empty()).then_some(arr);
    }
    if let Some(arr) = val.get("charts").and_then(|v| v.as_array()) {
        return (!arr.is_empty()).then_some(arr);
    }
    None
}

fn extract_chart_override(visual_suggestions: Option<&serde_json::Value>) -> ChartOverride {
    let empty = ChartOverride {
        chart_type: None,
        title: None,
        subtitle: None,
        span: None,
        height: None,
    };
    let Some(obj) = visual_suggestions.and_then(|v| v.as_object()) else {
        return empty;
    };
    ChartOverride {
        chart_type: obj.get("chartType").or_else(|| obj.get("chart_type")).and_then(|v| v.as_str()).map(String::from),
        title: obj.get("title").and_then(|v| v.as_str()).map(String::from),
        subtitle: obj.get("subtitle").and_then(|v| v.as_str()).map(String::from),
        span: obj.get("span").and_then(|v| v.as_u64()).map(|n| (n as usize).clamp(3, 12)),
        height: obj.get("height").and_then(|v| v.as_u64()).map(|n| (n as usize).clamp(280, 720)),
    }
}

fn extract_y_cols(obj: &serde_json::Map<String, serde_json::Value>, fallback: &[String]) -> Vec<String> {
    match obj.get("y") {
        Some(serde_json::Value::String(s)) if !s.is_empty() => vec![s.clone()],
        Some(serde_json::Value::Array(arr)) => {
            let cols: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
            if cols.is_empty() { fallback.to_vec() } else { cols }
        }
        _ => fallback.to_vec(),
    }
}

fn infer_columns(rows: &[serde_json::Value]) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let Some(first) = rows.first() else {
        return (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    };
    let columns: Vec<String> = match first {
        serde_json::Value::Object(map) => map.keys().cloned().collect(),
        _ => Vec::new(),
    };
    let numeric_cols: Vec<String> = columns
        .iter()
        .filter(|c| is_numeric(first.get(*c).unwrap_or(&serde_json::Value::Null)) && !c.to_lowercase().ends_with("id"))
        .cloned()
        .collect();
    let date_cols: Vec<String> = columns
        .iter()
        .filter(|c| is_date_like(c, first.get(*c).unwrap_or(&serde_json::Value::Null)))
        .cloned()
        .collect();
    let text_cols: Vec<String> = columns
        .iter()
        .filter(|c| {
            let v = first.get(*c).unwrap_or(&serde_json::Value::Null);
            !is_numeric(v) && !is_date_like(c, v) && !c.to_lowercase().ends_with("id")
        })
        .cloned()
        .collect();
    (columns, numeric_cols, text_cols, date_cols)
}

fn sum_numeric_column(rows: &[serde_json::Value], col: &str) -> f64 {
    rows.iter()
        .filter_map(|r| r.get(col).and_then(|v| {
            if v.is_number() { v.as_f64() }
            else if v.is_string() { v.as_str().and_then(|s| s.parse::<f64>().ok()) }
            else { None }
        }))
        .sum()
}

fn should_use_horizontal_bars(chart_type: &str, labels: &[String]) -> bool {
    chart_type == "bar"
        && (labels.len() > 10
            || (labels.len() > 6 && labels.iter().any(|l| l.chars().count() > 18)))
}

fn compute_chart_height(
    chart_type: &str,
    row_count: usize,
    is_horizontal: bool,
    override_height: Option<usize>,
) -> usize {
    if let Some(h) = override_height {
        return h;
    }
    if is_horizontal && chart_type == "bar" {
        return (row_count * 32 + 96).clamp(360, 640);
    }
    360
}

fn aggregate_top_n_pie(labels: Vec<String>, values: Vec<f64>, limit: usize) -> (Vec<String>, Vec<f64>) {
    let mut pairs: Vec<(String, f64)> = labels.into_iter().zip(values).collect();
    if pairs.len() <= limit {
        return pairs.into_iter().unzip();
    }
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut top: Vec<(String, f64)> = pairs.drain(..limit).collect();
    let others_sum: f64 = pairs.into_iter().map(|(_, v)| v).sum();
    if others_sum > 0.0 {
        top.push(("Others".to_string(), others_sum));
    }
    top.into_iter().unzip()
}

fn render_explanation_card(text: &str, span: usize) -> String {
    format!(
        r#"<div class="bento-card col-{}">
  <div class="flex items-center gap-2 mb-2">
    <div class="w-7 h-7 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400 text-xs shrink-0">
      <iconify-icon icon="lucide:sparkles"></iconify-icon>
    </div>
    <h3 class="text-sm font-semibold text-neutral-200 tracking-tight font-sans">Analysis Insights</h3>
  </div>
  <p class="text-xs text-neutral-300 leading-relaxed whitespace-pre-line">{}</p>
</div>"#,
        span,
        esc(text)
    )
}

fn render_spec_component(
    item: &serde_json::Value,
    datasets: &HashMap<String, Vec<serde_json::Value>>,
    default_dataset: &str,
    theme: &str,
) -> Option<String> {
    let obj = item.as_object()?;
    let comp_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("chart").to_lowercase();
    let span = obj.get("span").and_then(|v| v.as_u64()).map(|n| (n as usize).clamp(3, 12));

    if comp_type == "explanation" {
        let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
        return Some(render_explanation_card(text, span.unwrap_or(12)));
    }

    let ds_name = obj.get("dataset").and_then(|v| v.as_str()).unwrap_or(default_dataset);
    let rows = datasets.get(ds_name).or_else(|| datasets.get(default_dataset))?;
    if rows.is_empty() {
        return None;
    }

    if comp_type == "kpi" {
        let metric = obj.get("metric").and_then(|v| v.as_str())?;
        let label = obj.get("label").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| humanize_identifier(metric));
        let sum = sum_numeric_column(rows, metric);
        let formatted = format_report_value(&serde_json::json!(sum), metric);
        let icon = obj.get("icon").and_then(|v| v.as_str()).unwrap_or("lucide:trending-up");
        return Some(render_kpi_card(&label, &formatted, "Configured Metric", icon, "#10b981", span.unwrap_or(3)));
    }

    if comp_type == "table" {
        let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or(ds_name);
        let limit = obj.get("pagination").and_then(|v| v.as_u64()).unwrap_or(15) as usize;
        return Some(render_dataset_table_card(title, rows, span.unwrap_or(12), limit));
    }

    let (columns, numeric_cols, text_cols, date_cols) = infer_columns(rows);
    let fallback_x = date_cols.first().or_else(|| text_cols.first()).or_else(|| columns.first())?.clone();
    let x_col = obj.get("x").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or(&fallback_x);
    let fallback_y: Vec<String> = numeric_cols.first().cloned().into_iter().collect();
    let y_cols = extract_y_cols(obj, &fallback_y);
    if y_cols.is_empty() {
        return None;
    }

    let chart_type = obj.get("chartType").or_else(|| obj.get("chart_type")).and_then(|v| v.as_str()).unwrap_or("bar");
    let default_title = format!("{} Overview", humanize_identifier(&y_cols[0]));
    let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or(&default_title);
    let subtitle = obj.get("subtitle").and_then(|v| v.as_str()).unwrap_or("Configured visualization");
    let height = obj.get("height").and_then(|v| v.as_u64()).map(|n| (n as usize).clamp(280, 720)).unwrap_or(0);
    let colors: Option<Vec<String>> = obj.get("colors").and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|c| c.as_str().map(String::from)).collect())
        .filter(|v: &Vec<String>| !v.is_empty());

    Some(render_chart(
        rows,
        x_col,
        &y_cols,
        chart_type,
        title,
        subtitle,
        span.unwrap_or(6),
        height,
        theme,
        colors.as_deref(),
    ))
}

fn render_overview_tab(
    datasets: &HashMap<String, Vec<serde_json::Value>>,
    primary_dataset_name: &str,
    visual_suggestions: Option<&serde_json::Value>,
    theme: &str,
) -> String {
    let rows = datasets.get(primary_dataset_name).map(|v| v.as_slice()).unwrap_or(&[]);
    if rows.is_empty() {
        return "<div class='p-8 text-center text-neutral-400'>No records returned.</div>".to_string();
    }

    // Explicit multi-component DashboardSpec / layout mode
    if let Some(items) = parse_layout_items(visual_suggestions) {
        let rendered: Vec<String> = items
            .iter()
            .filter_map(|item| render_spec_component(item, datasets, primary_dataset_name, theme))
            .collect();
        if !rendered.is_empty() {
            return format!(r#"<div class="bento-grid">{}</div>"#, rendered.join("\n"));
        }
    }

    let (columns, numeric_cols, text_cols, date_cols) = infer_columns(rows);

    // 1. KPI Cards Row
    let mut kpis_html = Vec::new();
    kpis_html.push(render_kpi_card(
        "Total Volume",
        &rows.len().to_string(),
        "Aggregated Records",
        "lucide:layers",
        "#10b981",
        3,
    ));

    for (idx, col) in numeric_cols.iter().take(3).enumerate() {
        let sum = sum_numeric_column(rows, col);
        let colors = ["#06b6d4", "#8b5cf6", "#f59e0b"];
        let color = colors[idx % colors.len()];
        let icons = ["lucide:trending-up", "lucide:dollar-sign", "lucide:activity"];
        let icon = icons[idx % icons.len()];
        let formatted = format_report_value(&serde_json::json!(sum), col);

        kpis_html.push(render_kpi_card(
            &humanize_identifier(col),
            &formatted,
            "Calculated Metric",
            icon,
            color,
            3,
        ));
    }

    // 2. Smart Automatic Multi-Chart Bento Layout
    let mut charts_html = Vec::new();
    if !numeric_cols.is_empty() {
        let primary_metric = &numeric_cols[0];
        let override_spec = extract_chart_override(visual_suggestions);

        if let Some(date_col) = date_cols.first() {
            let trend_metrics: Vec<String> = numeric_cols.iter().take(2).cloned().collect();
            let trend_type = override_spec.chart_type.as_deref().unwrap_or(if trend_metrics.len() > 1 { "line" } else { "area" });
            let trend_span = override_spec.span.unwrap_or(if text_cols.is_empty() { 12 } else { 8 });
            charts_html.push(render_chart(
                rows,
                date_col,
                &trend_metrics,
                trend_type,
                override_spec.title.as_deref().unwrap_or(&format!("{} Trend", humanize_identifier(primary_metric))),
                override_spec.subtitle.as_deref().unwrap_or("Time-series progression"),
                trend_span,
                override_spec.height.unwrap_or(0),
                theme,
                None,
            ));

            if let Some(cat_col) = text_cols.first() {
                charts_html.push(render_chart(
                    rows,
                    cat_col,
                    &[primary_metric.clone()],
                    "donut",
                    "Proportional Share",
                    "Relative contribution ratio (Top 6)",
                    4,
                    360,
                    theme,
                    None,
                ));
            }
        } else if let Some(label_col) = text_cols.first().or_else(|| columns.first()) {
            let primary_type = override_spec.chart_type.as_deref().unwrap_or("bar");
            charts_html.push(render_chart(
                rows,
                label_col,
                &[primary_metric.clone()],
                primary_type,
                override_spec.title.as_deref().unwrap_or(&format!("{} Breakdown", humanize_identifier(primary_metric))),
                override_spec.subtitle.as_deref().unwrap_or("Ranked distribution across primary dimensions"),
                override_spec.span.unwrap_or(8),
                override_spec.height.unwrap_or(0),
                theme,
                None,
            ));

            charts_html.push(render_chart(
                rows,
                label_col,
                &[primary_metric.clone()],
                "donut",
                "Proportional Share",
                "Relative contribution ratio (Top 6)",
                4,
                360,
                theme,
                None,
            ));

            if numeric_cols.len() >= 2 {
                let comp_metrics: Vec<String> = numeric_cols.iter().take(2).cloned().collect();
                charts_html.push(render_chart(
                    rows,
                    label_col,
                    &comp_metrics,
                    "combo",
                    &format!("{} vs {}", humanize_identifier(&comp_metrics[0]), humanize_identifier(&comp_metrics[1])),
                    "Multi-metric comparative view",
                    if numeric_cols.len() >= 3 { 6 } else { 12 },
                    360,
                    theme,
                    None,
                ));
            }

            if numeric_cols.len() >= 3 {
                let third_metric = &numeric_cols[2];
                charts_html.push(render_chart(
                    rows,
                    label_col,
                    &[third_metric.clone()],
                    "line",
                    &format!("{} Distribution", humanize_identifier(third_metric)),
                    "Secondary metric trend",
                    6,
                    360,
                    theme,
                    None,
                ));
            }
        }
    }

    // Include secondary dataset summary chart when multi-dataset is present
    for (ds_name, ds_rows) in datasets {
        if ds_name == primary_dataset_name || ds_rows.is_empty() {
            continue;
        }
        let (ds_cols, ds_num, ds_text, ds_date) = infer_columns(ds_rows);
        if let Some(metric) = ds_num.first() {
            if let Some(x_col) = ds_date.first().or_else(|| ds_text.first()).or_else(|| ds_cols.first()) {
                let ctype = if !ds_date.is_empty() { "area" } else { "bar" };
                charts_html.push(render_chart(
                    ds_rows,
                    x_col,
                    &[metric.clone()],
                    ctype,
                    &format!("{}: {}", humanize_identifier(ds_name), humanize_identifier(metric)),
                    "Secondary dataset overview",
                    6,
                    360,
                    theme,
                    None,
                ));
            }
        }
    }

    format!(
        r#"<div class="bento-grid">
  {}
  {}
</div>"#,
        kpis_html.join("\n"),
        charts_html.join("\n")
    )
}

fn render_kpi_card(
    title: &str,
    value: &str,
    subtitle: &str,
    icon: &str,
    accent_color: &str,
    span: usize,
) -> String {
    format!(
        r#"<div class="bento-card col-{} relative overflow-hidden group">
  <div class="flex items-center justify-between mb-3">
    <span class="text-xs font-semibold text-neutral-400 font-sans tracking-wide">{}</span>
    <div class="w-8 h-8 rounded-lg flex items-center justify-center text-sm shadow-sm" style="background-color: {}15; color: {}; border: 1px solid {}30;">
      <iconify-icon icon="{}"></iconify-icon>
    </div>
  </div>
  <div class="text-2xl font-bold text-neutral-100 tracking-tight font-sans">{}</div>
  <div class="text-[11px] text-neutral-500 font-mono mt-1">{}</div>
</div>"#,
        span,
        esc(title),
        accent_color,
        accent_color,
        accent_color,
        icon,
        esc(value),
        esc(subtitle)
    )
}

fn extract_numeric_series(rows: &[serde_json::Value], col: &str) -> Vec<f64> {
    rows.iter()
        .map(|r| {
            r.get(col)
                .and_then(|v| {
                    if v.is_number() {
                        v.as_f64()
                    } else if v.is_string() {
                        v.as_str().and_then(|s| s.parse::<f64>().ok())
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0)
        })
        .collect()
}

fn render_chart(
    rows: &[serde_json::Value],
    label_col: &str,
    value_cols: &[String],
    chart_type: &str,
    title: &str,
    subtitle: &str,
    span: usize,
    height: usize,
    theme: &str,
    custom_colors: Option<&[String]>,
) -> String {
    let chart_id = format!("chart_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[0..8].to_string());
    let primary_col = value_cols.first().map(|s| s.as_str()).unwrap_or("value");

    let raw_labels: Vec<String> = rows.iter()
        .map(|r| r.get(label_col).map(|v| v.as_str().unwrap_or(&v.to_string()).to_string()).unwrap_or_default())
        .collect();

    let is_pie = chart_type == "donut" || chart_type == "pie";
    let is_combo = chart_type == "combo";
    let apex_type = if is_combo { "line" } else { chart_type };
    let is_dark = theme != "light";
    let axis_color = if is_dark { "#9ca3af" } else { "#475569" };
    let is_horizontal = !is_pie && !is_combo && should_use_horizontal_bars(chart_type, &raw_labels);
    let effective_height = compute_chart_height(
        chart_type,
        raw_labels.len(),
        is_horizontal,
        if height > 0 { Some(height) } else { None },
    );

    let (labels, series_json, xaxis_json) = if is_pie {
        let raw_values = extract_numeric_series(rows, primary_col);
        let (pie_labels, pie_values) = aggregate_top_n_pie(raw_labels, raw_values, 6);
        (pie_labels, serde_json::json!(pie_values), serde_json::json!({}))
    } else {
        let series_items: Vec<serde_json::Value> = value_cols
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let data = extract_numeric_series(rows, col);
                if is_combo {
                    serde_json::json!({
                        "name": humanize_identifier(col),
                        "type": if idx == 0 { "column" } else { "line" },
                        "data": data
                    })
                } else {
                    serde_json::json!({
                        "name": humanize_identifier(col),
                        "data": data
                    })
                }
            })
            .collect();
        let rotate_angle = if !is_horizontal && raw_labels.len() > 6 { -35 } else { 0 };
        let xaxis = serde_json::json!({
            "categories": raw_labels,
            "axisBorder": { "show": false },
            "axisTicks": { "show": false },
            "labels": {
                "trim": true,
                "rotate": rotate_angle,
                "style": { "colors": axis_color, "fontSize": "11px" }
            }
        });
        (raw_labels, serde_json::json!(series_items), xaxis)
    };

    let default_colors = vec![
        "#10b981".to_string(),
        "#06b6d4".to_string(),
        "#8b5cf6".to_string(),
        "#f59e0b".to_string(),
        "#ec4899".to_string(),
        "#3b82f6".to_string(),
        "#64748b".to_string(),
    ];
    let colors = custom_colors.unwrap_or(&default_colors);
    let fill_json = if chart_type == "area" {
        serde_json::json!({
            "type": "gradient",
            "gradient": { "shadeIntensity": 1, "opacityFrom": 0.42, "opacityTo": 0.05, "stops": [0, 95, 100] }
        })
    } else {
        serde_json::json!({ "opacity": 1 })
    };
    let stroke_json = if is_combo {
        serde_json::json!({ "curve": "smooth", "width": [0, 3] })
    } else {
        serde_json::json!({ "curve": "smooth", "width": 2 })
    };

    format!(
        r#"<div class="bento-card col-{}">
  <div class="flex items-center justify-between mb-4">
    <div class="min-w-0">
      <h3 class="text-sm font-semibold text-neutral-200 tracking-tight font-sans truncate">{}</h3>
      <p class="text-[11px] text-neutral-400 font-mono mt-0.5 truncate">{}</p>
    </div>
    <div class="w-7 h-7 rounded-lg bg-[#14161f] border border-[#232734] flex items-center justify-center text-neutral-400 text-xs shadow-sm shrink-0">
      <iconify-icon icon="lucide:bar-chart-2"></iconify-icon>
    </div>
  </div>
  <div id="{}" class="chart-container" style="min-height: {}px;"></div>
  <script>
    new ApexCharts(document.getElementById('{}'), {{
      chart: {{
        type: '{}',
        width: '100%',
        height: {},
        parentHeightOffset: 0,
        redrawOnParentResize: true,
        redrawOnWindowResize: true,
        toolbar: {{ show: false }},
        background: 'transparent',
        foreColor: '{}',
        fontFamily: 'Plus Jakarta Sans, sans-serif'
      }},
      theme: {{ mode: '{}' }},
      colors: {:?},
      series: {},
      labels: {:?},
      xaxis: {},
      fill: {},
      dataLabels: {{ enabled: false }},
      stroke: {},
      legend: {{
        position: 'bottom',
        fontSize: '11px',
        labels: {{ colors: '{}' }}
      }},
      plotOptions: {{
        bar: {{ horizontal: {}, borderRadius: 6, columnWidth: '48%', barHeight: '65%' }},
        pie: {{ donut: {{ size: '68%' }} }}
      }},
      grid: {{
        borderColor: '{}',
        strokeDashArray: 4
      }},
      responsive: [{{
        breakpoint: 768,
        options: {{
          chart: {{ height: 320 }},
          legend: {{ position: 'bottom' }}
        }}
      }}]
    }}).render();
  </script>
</div>"#,
        span,
        esc(title),
        esc(subtitle),
        chart_id,
        effective_height,
        chart_id,
        apex_type,
        effective_height,
        axis_color,
        if is_dark { "dark" } else { "light" },
        colors,
        series_json,
        labels,
        xaxis_json,
        fill_json,
        stroke_json,
        axis_color,
        is_horizontal,
        if is_dark { "#1e222e" } else { "#e2e8f0" }
    )
}

fn render_dataset_table_card(
    dataset_name: &str,
    rows: &[serde_json::Value],
    span: usize,
    row_limit: usize,
) -> String {
    let table_id = format!("tbl_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[0..8].to_string());
    let first = &rows[0];
    let columns: Vec<String> = match first {
        serde_json::Value::Object(map) => map.keys().cloned().collect(),
        _ => Vec::new(),
    };

    let thead_th = columns.iter().map(|c| {
        format!(
            r#"<th class="px-4 py-3 text-left text-[11px] font-semibold text-neutral-400 uppercase tracking-wider font-mono">{}</th>"#,
            esc(&humanize_identifier(c))
        )
    }).collect::<Vec<_>>().join("");

    let tbody_tr = rows.iter().take(row_limit).map(|r| {
        let tds = columns.iter().map(|c| {
            let val = r.get(c).unwrap_or(&serde_json::Value::Null);
            let formatted = format_report_value(val, c);
            format!(
                r#"<td class="px-4 py-2.5 text-xs text-neutral-200 font-sans border-b border-[#1a1e28] whitespace-nowrap">{}</td>"#,
                esc(&formatted)
            )
        }).collect::<Vec<_>>().join("");
        format!(r#"<tr class="hover:bg-[#151822] transition-colors">{}</tr>"#, tds)
    }).collect::<Vec<_>>().join("\n");

    format!(
        r#"<div class="bento-card col-{} space-y-4">
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 pb-3 border-b border-[var(--border)]">
    <div>
      <h3 class="text-sm font-semibold text-neutral-200 tracking-tight font-sans flex items-center gap-2">
        <iconify-icon icon="lucide:table" class="text-emerald-400"></iconify-icon>
        Dataset: {}
      </h3>
      <p class="text-[11px] text-neutral-400 font-mono mt-0.5">{} records returned</p>
    </div>
    <div class="flex items-center gap-2">
      <div class="relative">
        <iconify-icon icon="lucide:search" class="absolute left-3 top-2.5 text-neutral-500 text-xs"></iconify-icon>
        <input type="text" oninput="filterTable('{}', this.value)" placeholder="Search records..." class="bg-[#14161f] border border-[#232734] focus:border-emerald-500 rounded-lg pl-8 pr-3 py-1.5 text-xs text-neutral-200 font-mono outline-none">
      </div>
    </div>
  </div>

  <div class="overflow-x-auto rounded-xl border border-[var(--border)] bg-[#0c0d12]">
    <table id="{}" class="w-full text-left border-collapse">
      <thead class="bg-[#12141c] border-b border-[var(--border)]">
        <tr>{}</tr>
      </thead>
      <tbody>
        {}
      </tbody>
    </table>
  </div>
</div>"#,
        span,
        esc(dataset_name),
        rows.len(),
        table_id,
        table_id,
        thead_th,
        tbody_tr
    )
}

fn render_multi_table_explorer(datasets: &HashMap<String, Vec<serde_json::Value>>) -> String {
    let tables_html: Vec<String> = datasets
        .iter()
        .filter(|(_, rows)| !rows.is_empty())
        .map(|(dataset_name, rows)| render_dataset_table_card(dataset_name, rows, 12, 50))
        .collect();

    format!(r#"<div class="bento-grid">{}</div>"#, tables_html.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_multi_dashboard() {
        let mut datasets = HashMap::new();
        datasets.insert("sales".to_string(), vec![
            serde_json::json!({ "region": "North America", "revenue": 450000.0, "orders": 1240 }),
            serde_json::json!({ "region": "Europe", "revenue": 280000.0, "orders": 810 }),
        ]);

        let html = generate_multi_dashboard(&datasets, "Executive Sales Review", "dark", None);
        assert!(!html.is_empty());
        assert!(html.contains("Executive Sales Review"));
        assert!(html.contains("iconify-icon"));
        assert!(html.contains("North America"));
        assert!(html.contains("Safe Blind Analytics"));
    }

    #[test]
    fn test_isolated_dashboard_generator_chart_sizing() {
        let rows: Vec<serde_json::Value> = (1..=14)
            .map(|i| {
                serde_json::json!({
                    "category": format!("Segment Category {}", i),
                    "revenue": (15 - i) as f64 * 12500.0,
                    "orders": i * 42
                })
            })
            .collect();

        let suggestions = serde_json::json!({
            "chartType": "bar",
            "title": "Revenue by Segment",
            "subtitle": "14 categories — auto horizontal & dynamic height",
            "span": 8
        });

        let html = generate_dashboard(&rows, "Isolated Sizing Test", "dark", Some(&suggestions));
        assert!(html.contains("Revenue by Segment"));
        assert!(html.contains("class=\"chart-container\""));
        assert!(html.contains("width: '100%'"));
        assert!(html.contains("horizontal: true"));
        assert!(html.contains("height: 544"));
        assert!(html.contains("height: 360"));
        assert!(html.contains("\"Others\""));

        if let Ok(out_path) = std::env::var("DASHBOARD_PREVIEW_OUT") {
            std::fs::write(out_path, &html).expect("failed to write dashboard preview");
        }
    }

    #[test]
    fn test_multi_chart_layout_and_combo_series() {
        let mut datasets = HashMap::new();
        datasets.insert(
            "trend".to_string(),
            vec![
                serde_json::json!({ "month": "2026-01", "revenue": 1000.0, "orders": 10 }),
                serde_json::json!({ "month": "2026-02", "revenue": 1500.0, "orders": 18 }),
            ],
        );
        let spec = serde_json::json!({
            "layout": [
                { "type": "explanation", "text": "Growth accelerated in February.", "span": 12 },
                { "type": "chart", "dataset": "trend", "chartType": "combo", "x": "month", "y": ["revenue", "orders"], "title": "Combo View", "span": 6 },
                { "type": "chart", "dataset": "trend", "chartType": "area", "x": "month", "y": "revenue", "title": "Area View", "span": 6 }
            ]
        });

        let html = generate_multi_dashboard(&datasets, "Multi-Chart Spec", "dark", Some(&spec));
        assert!(html.contains("Analysis Insights"));
        assert!(html.contains("Growth accelerated in February."));
        assert!(html.contains("Combo View"));
        assert!(html.contains("Area View"));
        assert!(html.contains("\"type\":\"column\""));
        assert!(html.contains("toggleTheme()"));
    }
}
