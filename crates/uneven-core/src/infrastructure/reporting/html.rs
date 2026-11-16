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
    let primary_rows = datasets.get(&primary_dataset_name).map(|v| v.as_slice()).unwrap_or(&[]);

    // Tab 1: Executive Overview
    tab_buttons.push(format!(
        r#"<button onclick="switchTab('tab-overview')" data-tab-btn="tab-overview" class="tab-btn active px-4 py-2 rounded-xl text-xs font-semibold flex items-center gap-2 transition-all cursor-pointer bg-emerald-500/10 text-emerald-400 border border-emerald-500/20"><iconify-icon icon="lucide:layout-dashboard" class="text-sm"></iconify-icon>Executive Overview</button>"#
    ));

    let overview_html = render_overview_tab(primary_rows, visual_suggestions, theme);
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

fn render_overview_tab(
    rows: &[serde_json::Value],
    _visual_suggestions: Option<&serde_json::Value>,
    theme: &str,
) -> String {
    if rows.is_empty() {
        return "<div class='p-8 text-center text-neutral-400'>No records returned.</div>".to_string();
    }

    let first = &rows[0];
    let columns: Vec<String> = match first {
        serde_json::Value::Object(map) => map.keys().cloned().collect(),
        _ => Vec::new(),
    };

    let numeric_cols: Vec<String> = columns
        .iter()
        .filter(|c| is_numeric(first.get(*c).unwrap_or(&serde_json::Value::Null)) && !c.to_lowercase().ends_with("id"))
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
        let sum: f64 = rows
            .iter()
            .filter_map(|r| r.get(col).and_then(|v| {
                if v.is_number() { v.as_f64() }
                else if v.is_string() { v.as_str().and_then(|s| s.parse::<f64>().ok()) }
                else { None }
            }))
            .sum();

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

    // 2. Charts Row (Bento Horizontal Layout)
    let mut charts_html = Vec::new();
    if !numeric_cols.is_empty() {
        let primary_metric = &numeric_cols[0];
        let label_col = text_cols.first().unwrap_or(&columns[0]);

        // Main Chart (col-8)
        charts_html.push(render_chart(
            rows,
            label_col,
            primary_metric,
            "bar",
            &format!("{} Breakdown", humanize_identifier(primary_metric)),
            "Ranked distribution across primary dimensions",
            8,
            280,
            theme,
        ));

        // Secondary Distribution Donut Chart (col-4)
        charts_html.push(render_chart(
            rows,
            label_col,
            primary_metric,
            "donut",
            "Proportional Share",
            "Relative contribution ratio",
            4,
            280,
            theme,
        ));
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

fn render_chart(
    rows: &[serde_json::Value],
    label_col: &str,
    value_col: &str,
    chart_type: &str,
    title: &str,
    subtitle: &str,
    span: usize,
    height: usize,
    theme: &str,
) -> String {
    let chart_id = format!("chart_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[0..8].to_string());
    
    let labels: Vec<String> = rows.iter()
        .map(|r| r.get(label_col).map(|v| v.as_str().unwrap_or(&v.to_string()).to_string()).unwrap_or_default())
        .collect();

    let values: Vec<f64> = rows.iter()
        .filter_map(|r| r.get(value_col).and_then(|v| {
            if v.is_number() { v.as_f64() }
            else if v.is_string() { v.as_str().and_then(|s| s.parse::<f64>().ok()) }
            else { None }
        }))
        .collect();

    let is_pie = chart_type == "donut" || chart_type == "pie";
    let is_dark = theme != "light";

    let (series_json, xaxis_json) = if is_pie {
        (serde_json::json!(values), serde_json::json!({}))
    } else {
        (
            serde_json::json!([{ "name": humanize_identifier(value_col), "data": values }]),
            serde_json::json!({
                "categories": labels,
                "axisBorder": { "show": false },
                "axisTicks": { "show": false },
                "labels": { "style": { "colors": if is_dark { "#9ca3af" } else { "#475569" }, "fontSize": "11px" } }
            })
        )
    };

    let colors = vec!["#10b981", "#06b6d4", "#8b5cf6", "#f59e0b", "#ec4899"];

    format!(
        r#"<div class="bento-card col-{}">
  <div class="flex items-center justify-between mb-4">
    <div>
      <h3 class="text-sm font-semibold text-neutral-200 tracking-tight font-sans">{}</h3>
      <p class="text-[11px] text-neutral-400 font-mono mt-0.5">{}</p>
    </div>
    <div class="w-7 h-7 rounded-lg bg-[#14161f] border border-[#232734] flex items-center justify-center text-neutral-400 text-xs shadow-sm">
      <iconify-icon icon="lucide:bar-chart-2"></iconify-icon>
    </div>
  </div>
  <div id="{}" style="min-height: {}px;"></div>
  <script>
    new ApexCharts(document.getElementById('{}'), {{
      chart: {{
        type: '{}',
        height: {},
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
      stroke: {{ curve: 'smooth', width: 2 }},
      plotOptions: {{
        bar: {{ borderRadius: 6, columnWidth: '45%' }}
      }},
      grid: {{
        borderColor: '{}',
        strokeDashArray: 4
      }}
    }}).render();
  </script>
</div>"#,
        span,
        esc(title),
        esc(subtitle),
        chart_id,
        height,
        chart_id,
        chart_type,
        height,
        if is_dark { "#9ca3af" } else { "#475569" },
        if is_dark { "dark" } else { "light" },
        colors,
        series_json,
        labels,
        xaxis_json,
        if is_dark { "#1e222e" } else { "#e2e8f0" }
    )
}

fn render_multi_table_explorer(datasets: &HashMap<String, Vec<serde_json::Value>>) -> String {
    let mut tables_html = Vec::new();

    for (dataset_name, rows) in datasets {
        let table_id = format!("tbl_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[0..8].to_string());
        
        if rows.is_empty() {
            continue;
        }

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

        let tbody_tr = rows.iter().take(50).map(|r| {
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

        tables_html.push(format!(
            r#"<div class="bento-card col-12 space-y-4">
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
            esc(dataset_name),
            rows.len(),
            table_id,
            table_id,
            thead_th,
            tbody_tr
        ));
    }

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
}
