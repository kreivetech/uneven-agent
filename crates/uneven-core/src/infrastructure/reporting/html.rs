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

    // Tab 1..N: Fixed-Viewport Analytics Pages (max 4 charts per tab)
    let overview_pages = build_overview_pages(datasets, &primary_dataset_name, visual_suggestions, theme);
    for (idx, (tab_title, page_html)) in overview_pages.into_iter().enumerate() {
        let tab_id = if idx == 0 {
            "tab-overview".to_string()
        } else {
            format!("tab-page-{}", idx + 1)
        };
        let is_first = idx == 0;
        let btn_class = if is_first {
            "tab-btn active px-4 py-2 rounded-xl text-xs font-semibold flex items-center gap-2 transition-all cursor-pointer bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 shrink-0"
        } else {
            "tab-btn px-4 py-2 rounded-xl text-xs font-semibold flex items-center gap-2 transition-all cursor-pointer text-neutral-400 hover:text-white border border-transparent shrink-0"
        };
        let icon = if is_first { "lucide:layout-dashboard" } else { "lucide:bar-chart-2" };
        tab_buttons.push(format!(
            r#"<button onclick="switchTab('{}')" data-tab-btn="{}" class="{}"><iconify-icon icon="{}" class="text-sm"></iconify-icon>{}</button>"#,
            tab_id, tab_id, btn_class, icon, esc(&tab_title)
        ));
        let hidden_cls = if is_first { "" } else { " hidden" };
        tab_contents.push(format!(
            r#"<section id="{}" class="tab-pane canvas-tab{}">{}</section>"#,
            tab_id, hidden_cls, page_html
        ));
    }

    // Last Tab: Data Explorer (Multi-table support, scrollable)
    tab_buttons.push(format!(
        r#"<button onclick="switchTab('tab-explorer')" data-tab-btn="tab-explorer" class="tab-btn px-4 py-2 rounded-xl text-xs font-semibold flex items-center gap-2 transition-all cursor-pointer text-neutral-400 hover:text-white border border-transparent shrink-0"><iconify-icon icon="lucide:database" class="text-sm"></iconify-icon>Data Explorer ({} tables)</button>"#,
        datasets.len()
    ));

    let explorer_html = render_multi_table_explorer(datasets);
    tab_contents.push(format!(
        r#"<section id="tab-explorer" class="tab-pane scroll-tab hidden flex flex-col gap-6">{}</section>"#,
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
    <main class="dashboard-main">
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

      setTimeout(() => window.dispatchEvent(new Event('resize')), 30);
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
    };
    let Some(obj) = visual_suggestions.and_then(|v| v.as_object()) else {
        return empty;
    };
    ChartOverride {
        chart_type: obj.get("chartType").or_else(|| obj.get("chart_type")).and_then(|v| v.as_str()).map(String::from),
        title: obj.get("title").and_then(|v| v.as_str()).map(String::from),
        subtitle: obj.get("subtitle").and_then(|v| v.as_str()).map(String::from),
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

const MAX_KPIS_PER_TAB: usize = 5;
const MAX_CHARTS_PER_TAB: usize = 3;

struct CanvasChartSpec {
    dataset: String,
    x_col: String,
    y_cols: Vec<String>,
    chart_type: String,
    title: String,
    subtitle: String,
    colors: Option<Vec<String>>,
}

fn is_radial_chart(chart_type: &str) -> bool {
    matches!(chart_type, "donut" | "pie" | "radialbar")
}

fn layout_charts_for_page(
    chunk: &[CanvasChartSpec],
    datasets: &HashMap<String, Vec<serde_json::Value>>,
    primary_dataset_name: &str,
    theme: &str,
) -> String {
    let render_spec = |spec: &CanvasChartSpec, slot_class: &str| {
        let rows = datasets
            .get(&spec.dataset)
            .or_else(|| datasets.get(primary_dataset_name))
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        render_chart(
            rows,
            &spec.x_col,
            &spec.y_cols,
            &spec.chart_type,
            &spec.title,
            &spec.subtitle,
            slot_class,
            theme,
            spec.colors.as_deref(),
        )
    };

    match chunk.len() {
        0 => String::new(),
        1 => format!(
            r#"<div class="chart-canvas layout-1">{}</div>"#,
            render_spec(&chunk[0], "")
        ),
        2 => {
            let p0 = is_radial_chart(&chunk[0].chart_type);
            let p1 = is_radial_chart(&chunk[1].chart_type);
            let cls = if p1 && !p0 {
                "layout-2-pie-right"
            } else if p0 && !p1 {
                "layout-2-pie-left"
            } else {
                "layout-2"
            };
            format!(
                r#"<div class="chart-canvas {}">{}{}</div>"#,
                cls,
                render_spec(&chunk[0], ""),
                render_spec(&chunk[1], "")
            )
        }
        _ => {
            if let Some(pie_idx) = chunk.iter().position(|c| is_radial_chart(&c.chart_type)) {
                let mut cards = Vec::with_capacity(3);
                for (idx, spec) in chunk.iter().enumerate() {
                    if idx != pie_idx {
                        cards.push(render_spec(spec, ""));
                    }
                }
                cards.push(render_spec(&chunk[pie_idx], "slot-tall"));
                format!(
                    r#"<div class="chart-canvas layout-3-l">{}</div>"#,
                    cards.join("\n")
                )
            } else {
                format!(
                    r#"<div class="chart-canvas layout-3-stack">{}{}{}</div>"#,
                    render_spec(&chunk[0], ""),
                    render_spec(&chunk[1], ""),
                    render_spec(&chunk[2], "slot-wide")
                )
            }
        }
    }
}

fn collect_spec_items(
    items: &[serde_json::Value],
    datasets: &HashMap<String, Vec<serde_json::Value>>,
    default_dataset: &str,
) -> (Vec<String>, Vec<CanvasChartSpec>) {
    let mut kpis = Vec::new();
    let mut charts = Vec::new();

    for item in items {
        let Some(root_obj) = item.as_object() else { continue };
        let comp_type = root_obj.get("type").and_then(|v| v.as_str()).unwrap_or("chart").to_lowercase();
        let cfg = root_obj.get("config").and_then(|v| v.as_object()).unwrap_or(root_obj);

        if comp_type == "explanation" || comp_type == "table" {
            continue;
        }

        let ds_name = cfg.get("dataset")
            .or_else(|| root_obj.get("dataset"))
            .and_then(|v| v.as_str())
            .unwrap_or(default_dataset);
        let Some(rows) = datasets.get(ds_name).or_else(|| datasets.get(default_dataset)) else { continue };
        if rows.is_empty() {
            continue;
        }

        if comp_type == "kpi" || comp_type == "card" {
            if kpis.len() >= MAX_KPIS_PER_TAB {
                continue;
            }
            let Some(metric) = cfg.get("metric").or_else(|| cfg.get("metricCol")).and_then(|v| v.as_str()) else { continue };
            let label = cfg.get("label")
                .or_else(|| cfg.get("title"))
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| humanize_identifier(metric));
            let sum = sum_numeric_column(rows, metric);
            let formatted = format_report_value(&serde_json::json!(sum), metric);
            let icon = cfg.get("icon").and_then(|v| v.as_str()).unwrap_or("lucide:trending-up");
            let icon_name = if icon.contains(':') { icon.to_string() } else { format!("lucide:{}", icon) };
            kpis.push(render_kpi_card(&label, &formatted, "", &icon_name, "#10b981", 3));
            continue;
        }

        let (columns, numeric_cols, text_cols, date_cols) = infer_columns(rows);
        let Some(fallback_x) = date_cols.first().or_else(|| text_cols.first()).or_else(|| columns.first()).cloned() else { continue };
        let x_col = cfg.get("x")
            .or_else(|| cfg.get("labelCol"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&fallback_x)
            .to_string();

        let fallback_y: Vec<String> = numeric_cols.first().cloned().into_iter().collect();
        let y_cols = match cfg.get("valueCols") {
            Some(serde_json::Value::Array(arr)) => {
                let cols: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                if cols.is_empty() { extract_y_cols(cfg, &fallback_y) } else { cols }
            }
            _ => extract_y_cols(cfg, &fallback_y),
        };
        if y_cols.is_empty() {
            continue;
        }

        let chart_type = cfg.get("chartType")
            .or_else(|| cfg.get("chart_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("bar")
            .to_lowercase();
        let title = cfg.get("title")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("{} Overview", humanize_identifier(&y_cols[0])));
        let subtitle = cfg.get("subtitle")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| "Configured visualization".to_string());
        let colors: Option<Vec<String>> = cfg.get("colors")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|c| c.as_str().map(String::from)).collect())
            .filter(|v: &Vec<String>| !v.is_empty());

        charts.push(CanvasChartSpec {
            dataset: ds_name.to_string(),
            x_col,
            y_cols,
            chart_type,
            title,
            subtitle,
            colors,
        });
    }

    (kpis, charts)
}

fn paginate_charts_into_tabs(
    kpis: &[String],
    charts: &[CanvasChartSpec],
    datasets: &HashMap<String, Vec<serde_json::Value>>,
    primary_dataset_name: &str,
    theme: &str,
) -> Vec<(String, String)> {
    let kpi_count = kpis.len().min(MAX_KPIS_PER_TAB);
    let kpi_strip = if kpi_count > 0 {
        format!(
            r#"<div class="kpi-strip" style="grid-template-columns: repeat({}, minmax(0, 1fr));">{}</div>"#,
            kpi_count,
            kpis.iter().take(kpi_count).cloned().collect::<Vec<_>>().join("\n")
        )
    } else {
        String::new()
    };

    if charts.is_empty() {
        return vec![("Executive Overview".to_string(), kpi_strip)];
    }

    let mut pages = Vec::new();
    for (page_idx, chunk) in charts.chunks(MAX_CHARTS_PER_TAB).enumerate() {
        let canvas_html = layout_charts_for_page(chunk, datasets, primary_dataset_name, theme);
        let (tab_title, page_body) = if page_idx == 0 {
            ("Executive Overview".to_string(), format!("{}{}", kpi_strip, canvas_html))
        } else {
            (format!("Analytics Page {}", page_idx + 1), canvas_html)
        };
        pages.push((tab_title, page_body));
    }
    pages
}

fn build_overview_pages(
    datasets: &HashMap<String, Vec<serde_json::Value>>,
    primary_dataset_name: &str,
    visual_suggestions: Option<&serde_json::Value>,
    theme: &str,
) -> Vec<(String, String)> {
    let rows = datasets.get(primary_dataset_name).map(|v| v.as_slice()).unwrap_or(&[]);
    if rows.is_empty() {
        return vec![("Executive Overview".to_string(), "<div class='p-8 text-center text-neutral-400'>No records returned.</div>".to_string())];
    }

    if let Some(items) = parse_layout_items(visual_suggestions) {
        let (kpis, charts) = collect_spec_items(items, datasets, primary_dataset_name);
        if !kpis.is_empty() || !charts.is_empty() {
            return paginate_charts_into_tabs(&kpis, &charts, datasets, primary_dataset_name, theme);
        }
    }

    let (columns, numeric_cols, text_cols, date_cols) = infer_columns(rows);

    let mut kpis_html = Vec::new();
    kpis_html.push(render_kpi_card(
        "Total Volume",
        &rows.len().to_string(),
        "",
        "lucide:layers",
        "#10b981",
        3,
    ));

    for (idx, col) in numeric_cols.iter().take(4).enumerate() {
        let sum = sum_numeric_column(rows, col);
        let colors = ["#06b6d4", "#8b5cf6", "#f59e0b", "#ec4899"];
        let color = colors[idx % colors.len()];
        let icons = ["lucide:trending-up", "lucide:dollar-sign", "lucide:activity", "lucide:hash"];
        let icon = icons[idx % icons.len()];
        let formatted = format_report_value(&serde_json::json!(sum), col);

        kpis_html.push(render_kpi_card(
            &humanize_identifier(col),
            &formatted,
            "",
            icon,
            color,
            3,
        ));
    }

    let mut charts: Vec<CanvasChartSpec> = Vec::new();
    if !numeric_cols.is_empty() {
        let primary_metric = &numeric_cols[0];
        let override_spec = extract_chart_override(visual_suggestions);

        if let Some(date_col) = date_cols.first() {
            let trend_metrics: Vec<String> = numeric_cols.iter().take(2).cloned().collect();
            let trend_type = override_spec.chart_type.unwrap_or_else(|| if trend_metrics.len() > 1 { "line".to_string() } else { "area".to_string() });
            charts.push(CanvasChartSpec {
                dataset: primary_dataset_name.to_string(),
                x_col: date_col.clone(),
                y_cols: trend_metrics,
                chart_type: trend_type,
                title: override_spec.title.unwrap_or_else(|| format!("{} Trend", humanize_identifier(primary_metric))),
                subtitle: override_spec.subtitle.unwrap_or_else(|| "Time-series progression".to_string()),
                colors: None,
            });

            if let Some(cat_col) = text_cols.first() {
                charts.push(CanvasChartSpec {
                    dataset: primary_dataset_name.to_string(),
                    x_col: cat_col.clone(),
                    y_cols: vec![primary_metric.clone()],
                    chart_type: "donut".to_string(),
                    title: "Proportional Share".to_string(),
                    subtitle: "Relative contribution ratio (Top 6)".to_string(),
                    colors: None,
                });
            }
        } else if let Some(label_col) = text_cols.first().or_else(|| columns.first()) {
            let primary_type = override_spec.chart_type.unwrap_or_else(|| "bar".to_string());
            charts.push(CanvasChartSpec {
                dataset: primary_dataset_name.to_string(),
                x_col: label_col.clone(),
                y_cols: vec![primary_metric.clone()],
                chart_type: primary_type,
                title: override_spec.title.unwrap_or_else(|| format!("{} Breakdown", humanize_identifier(primary_metric))),
                subtitle: override_spec.subtitle.unwrap_or_else(|| "Ranked distribution across primary dimensions".to_string()),
                colors: None,
            });

            if numeric_cols.len() >= 2 {
                let comp_metrics: Vec<String> = numeric_cols.iter().take(2).cloned().collect();
                charts.push(CanvasChartSpec {
                    dataset: primary_dataset_name.to_string(),
                    x_col: label_col.clone(),
                    y_cols: comp_metrics.clone(),
                    chart_type: "combo".to_string(),
                    title: format!("{} vs {}", humanize_identifier(&comp_metrics[0]), humanize_identifier(&comp_metrics[1])),
                    subtitle: "Multi-metric comparative view".to_string(),
                    colors: None,
                });
            }

            charts.push(CanvasChartSpec {
                dataset: primary_dataset_name.to_string(),
                x_col: label_col.clone(),
                y_cols: vec![primary_metric.clone()],
                chart_type: "donut".to_string(),
                title: "Proportional Share".to_string(),
                subtitle: "Relative contribution ratio (Top 6)".to_string(),
                colors: None,
            });
        }
    }

    for (ds_name, ds_rows) in datasets {
        if ds_name == primary_dataset_name || ds_rows.is_empty() {
            continue;
        }
        let (ds_cols, ds_num, ds_text, ds_date) = infer_columns(ds_rows);
        if let Some(metric) = ds_num.first() {
            if let Some(x_col) = ds_date.first().or_else(|| ds_text.first()).or_else(|| ds_cols.first()) {
                let ctype = if !ds_date.is_empty() { "area" } else { "bar" };
                charts.push(CanvasChartSpec {
                    dataset: ds_name.clone(),
                    x_col: x_col.clone(),
                    y_cols: vec![metric.clone()],
                    chart_type: ctype.to_string(),
                    title: format!("{}: {}", humanize_identifier(ds_name), humanize_identifier(metric)),
                    subtitle: "Secondary dataset overview".to_string(),
                    colors: None,
                });
            }
        }
    }

    paginate_charts_into_tabs(&kpis_html, &charts, datasets, primary_dataset_name, theme)
}

fn render_kpi_card(
    title: &str,
    value: &str,
    _subtitle: &str,
    icon: &str,
    accent_color: &str,
    _span: usize,
) -> String {
    format!(
        r#"<div class="bento-card kpi-card relative overflow-hidden group">
  <div class="flex items-center justify-between mb-1.5">
    <span class="text-[11px] font-semibold text-neutral-400 font-sans tracking-wide truncate">{}</span>
    <div class="w-6 h-6 rounded-lg flex items-center justify-center text-xs shadow-sm shrink-0" style="background-color: {}15; color: {}; border: 1px solid {}30;">
      <iconify-icon icon="{}"></iconify-icon>
    </div>
  </div>
  <div class="text-xl font-bold text-neutral-100 tracking-tight font-sans truncate">{}</div>
</div>"#,
        esc(title),
        accent_color,
        accent_color,
        accent_color,
        icon,
        esc(value)
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
    slot_class: &str,
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

    // Keep bar charts bounded to Top 8 (+ Others for single series) so cards fit inside 100vh
    let (capped_labels, capped_rows): (Vec<String>, Vec<usize>) = if chart_type == "bar" && raw_labels.len() > 8 && value_cols.len() == 1 {
        let raw_vals = extract_numeric_series(rows, primary_col);
        let (top_l, _top_v) = aggregate_top_n_pie(raw_labels.clone(), raw_vals, 8);
        (top_l, Vec::new())
    } else {
        let indices: Vec<usize> = (0..raw_labels.len().min(12)).collect();
        (indices.iter().map(|&i| raw_labels[i].clone()).collect(), indices)
    };

    let is_horizontal = !is_pie && !is_combo && should_use_horizontal_bars(chart_type, &capped_labels);

    let (labels, series_json, xaxis_json) = if is_pie {
        let raw_values = extract_numeric_series(rows, primary_col);
        let (pie_labels, pie_values) = aggregate_top_n_pie(raw_labels, raw_values, 6);
        (pie_labels, serde_json::json!(pie_values), serde_json::json!({}))
    } else if chart_type == "bar" && raw_labels.len() > 8 && value_cols.len() == 1 {
        let raw_vals = extract_numeric_series(rows, primary_col);
        let (bar_labels, bar_values) = aggregate_top_n_pie(raw_labels, raw_vals, 8);
        let rotate_angle = if !is_horizontal && bar_labels.len() > 6 { -30 } else { 0 };
        let xaxis = serde_json::json!({
            "categories": bar_labels,
            "axisBorder": { "show": false },
            "axisTicks": { "show": false },
            "labels": {
                "trim": true,
                "rotate": rotate_angle,
                "style": { "colors": axis_color, "fontSize": "11px" }
            }
        });
        let series = serde_json::json!([{ "name": humanize_identifier(primary_col), "data": bar_values }]);
        (bar_labels, series, xaxis)
    } else {
        let series_items: Vec<serde_json::Value> = value_cols
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let full_data = extract_numeric_series(rows, col);
                let data: Vec<f64> = if capped_rows.is_empty() {
                    full_data
                } else {
                    capped_rows.iter().map(|&i| full_data[i]).collect()
                };
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
        let rotate_angle = if !is_horizontal && capped_labels.len() > 6 { -30 } else { 0 };
        let xaxis = serde_json::json!({
            "categories": capped_labels,
            "axisBorder": { "show": false },
            "axisTicks": { "show": false },
            "labels": {
                "trim": true,
                "rotate": rotate_angle,
                "style": { "colors": axis_color, "fontSize": "11px" }
            }
        });
        (capped_labels, serde_json::json!(series_items), xaxis)
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
    let yaxis_json = if is_combo && value_cols.len() >= 2 {
        serde_json::json!([
            {
                "title": { "text": humanize_identifier(&value_cols[0]), "style": { "color": axis_color, "fontSize": "10px" } },
                "labels": { "style": { "colors": axis_color, "fontSize": "10px" } }
            },
            {
                "opposite": true,
                "title": { "text": humanize_identifier(&value_cols[1]), "style": { "color": axis_color, "fontSize": "10px" } },
                "labels": { "style": { "colors": axis_color, "fontSize": "10px" } }
            }
        ])
    } else {
        serde_json::json!({
            "labels": { "style": { "colors": axis_color, "fontSize": "10px" } }
        })
    };

    let card_classes = if slot_class.is_empty() {
        "bento-card".to_string()
    } else {
        format!("bento-card {}", slot_class)
    };

    format!(
        r#"<div class="{}">
  <div class="flex items-center justify-between mb-2 shrink-0">
    <div class="min-w-0">
      <h3 class="text-sm font-semibold text-neutral-200 tracking-tight font-sans truncate">{}</h3>
      <p class="text-[11px] text-neutral-400 font-mono truncate">{}</p>
    </div>
    <div class="w-7 h-7 rounded-lg bg-[#14161f] border border-[#232734] flex items-center justify-center text-neutral-400 text-xs shadow-sm shrink-0">
      <iconify-icon icon="lucide:bar-chart-2"></iconify-icon>
    </div>
  </div>
  <div id="{}" class="chart-container"></div>
  <script>
    new ApexCharts(document.getElementById('{}'), {{
      chart: {{
        type: '{}',
        width: '100%',
        height: '100%',
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
      yaxis: {},
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
        breakpoint: 1023,
        options: {{
          chart: {{ height: 280 }},
          legend: {{ position: 'bottom' }}
        }}
      }}]
    }}).render();
  </script>
</div>"#,
        card_classes,
        esc(title),
        esc(subtitle),
        chart_id,
        chart_id,
        apex_type,
        axis_color,
        if is_dark { "dark" } else { "light" },
        colors,
        series_json,
        labels,
        xaxis_json,
        yaxis_json,
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
        assert!(html.contains("class=\"chart-canvas layout-3-l\""));
        assert!(html.contains("slot-tall"));
        assert!(html.contains("width: '100%'"));
        assert!(html.contains("height: '100%'"));
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
                { "type": "kpi", "dataset": "trend", "metric": "revenue", "label": "Revenue" },
                { "type": "kpi", "dataset": "trend", "metric": "orders", "label": "Orders" },
                { "type": "chart", "dataset": "trend", "chartType": "combo", "x": "month", "y": ["revenue", "orders"], "title": "Combo View" },
                { "type": "chart", "dataset": "trend", "chartType": "area", "x": "month", "y": "revenue", "title": "Area View" },
                { "type": "chart", "dataset": "trend", "chartType": "donut", "x": "month", "y": "revenue", "title": "Donut View" },
                { "type": "chart", "dataset": "trend", "chartType": "line", "x": "month", "y": "orders", "title": "Page 2 Line" }
            ]
        });

        let html = generate_multi_dashboard(&datasets, "Multi-Chart Spec", "dark", Some(&spec));
        assert!(html.contains("Combo View"));
        assert!(html.contains("Area View"));
        assert!(html.contains("Donut View"));
        assert!(html.contains("layout-3-l"));
        assert!(html.contains("Analytics Page 2"));
        assert!(html.contains("Page 2 Line"));
        assert!(html.contains("\"type\":\"column\""));
        assert!(html.contains("\"opposite\":true"));
        assert!(html.contains("toggleTheme()"));
    }
}
