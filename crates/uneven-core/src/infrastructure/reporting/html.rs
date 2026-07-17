pub const DASHBOARD_CSS: &str = r#"
  @import url('https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=Outfit:wght@400;500;600;700&display=swap');
  
  :root {
    --bg-main: #0f172a;
    --card-bg: rgba(15, 23, 42, 0.7);
    --primary: #3b82f6;
    --primary-glow: rgba(59, 130, 246, 0.5);
    --accent: #8b5cf6;
    --text-main: #f1f5f9;
    --text-muted: #64748b;
    --border: #1e293b;
  }

  body.light-mode {
    --bg-main: #f8fafc;
    --card-bg: rgba(255, 255, 255, 0.8);
    --primary: #2563eb;
    --primary-glow: rgba(37, 99, 235, 0.3);
    --accent: #7c3aed;
    --text-main: #0f172a;
    --text-muted: #475569;
    --border: #e2e8f0;
  }

  body {
    font-family: 'Inter', sans-serif;
    background-color: var(--bg-main);
    color: var(--text-main);
    background-image: 
      radial-gradient(at 0% 0%, var(--primary-glow) 0px, transparent 50%),
      radial-gradient(at 100% 0%, var(--primary-glow) 0px, transparent 50%);
  }

  .glass {
    background: var(--card-bg);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .premium-shadow {
    box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06);
  }

  h1, h2, h3 { font-family: 'Outfit', sans-serif; }

  /* Custom Scrollbar */
  ::-webkit-scrollbar { width: 6px; height: 6px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: #334155; border-radius: 10px; }
  ::-webkit-scrollbar-thumb:hover { background: #475569; }

  /* Print Styles */
  @media print {
    @page { margin: 0.5cm; }
    body { background: #fff !important; color: #000 !important; overflow: visible !important; height: auto !important; }
    .fixed, nav, button, .no-print, footer { display: none !important; }
    .lg\:pl-16, .lg\:pl-20 { padding-left: 0 !important; }
    main { overflow: visible !important; padding: 0 !important; }
    .bg-slate-900\/50, .bg-slate-900\/20, .bg-slate-800 { background: #fff !important; border: 1px solid #eee !important; box-shadow: none !important; }
    .text-white, .text-slate-300, .text-slate-400, .text-slate-500 { color: #000 !important; }
    .border-slate-800, .border-slate-700 { border-color: #eee !important; }
    section { break-inside: avoid; page-break-inside: avoid; display: block !important; width: 100% !important; margin-bottom: 1rem !important; }
    .grid { display: flex !important; flex-wrap: wrap !important; gap: 10px !important; }
    .grid > div { width: calc(50% - 10px) !important; min-width: 0 !important; margin-bottom: 10px !important; padding: 10px !important; }
    .grid-cols-4 > div { width: calc(25% - 10px) !important; }
    
    /* Table Print Fixes */
    table { width: 100% !important; border-collapse: collapse !important; }
    th, td { padding: 4px !important; font-size: 8px !important; border: 1px solid #eee !important; }
    .overflow-x-auto, .overflow-hidden { overflow: visible !important; border: none !important; }
    
    /* Header Print */
    header { padding: 0 0 10px 0 !important; border-bottom: 2px solid #000 !important; margin-bottom: 20px !important; }
    h1 { font-size: 18px !important; }
  }

  /* Table Custom Styles */
  .tbl-wrap table thead th {
    font-family: 'Outfit', sans-serif;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
"#;

pub const DASHBOARD_SCRIPTS: &str = r#"
  (function() {
    // Lucide Icons Initialization
    if (window.lucide) {
      lucide.createIcons();
    }

    // Tab Switching Logic
    window.switchTab = function(tabId) {
      document.querySelectorAll('section[data-tab]').forEach(section => {
        section.classList.add('hidden');
      });
      const activeSection = document.getElementById(tabId);
      if (activeSection) {
        activeSection.classList.remove('hidden');
      }

      // Update Sidebar UI
      document.querySelectorAll('nav a').forEach(a => {
        a.classList.remove('text-blue-500', 'bg-blue-500/10');
        a.classList.add('text-slate-500');
      });
      const activeLink = document.querySelector('nav a[data-target="' + tabId + '"]');
      if (activeLink) {
        activeLink.classList.remove('text-slate-500');
        activeLink.classList.add('text-blue-500', 'bg-blue-500/10');
      }
    };

    // Download Executive Report as PDF
    window.downloadExecutiveReport = function() {
      const { jsPDF } = window.jspdf;
      const doc = new jsPDF();
      
      const report = window.EXECUTIVE_REPORT || "No report generated.";
      const title = document.querySelector('h1')?.textContent || "Analysis Report";
      const date = new Date().toLocaleDateString();
      
      // PDF Styling
      doc.setFont("helvetica", "bold");
      doc.setFontSize(20);
      doc.text("EXECUTIVE ANALYSIS REPORT", 105, 20, { align: "center" });
      
      doc.setFontSize(10);
      doc.setFont("helvetica", "normal");
      doc.text("Generated by Uneven AI Engine v1.12.0", 105, 28, { align: "center" });
      
      doc.setDrawColor(200, 200, 200);
      doc.line(20, 35, 190, 35);
      
      doc.setFontSize(12);
      doc.setFont("helvetica", "bold");
      doc.text("Title: " + title, 20, 45);
      doc.text("Date: " + date, 20, 52);
      
      doc.line(20, 58, 190, 58);
      
      // Report Content
      doc.setFont("helvetica", "normal");
      doc.setFontSize(11);
      
      const splitText = doc.splitTextToSize(report, 170);
      doc.text(splitText, 20, 70);
      
      // Footer
      const pageCount = doc.internal.getNumberOfPages();
      for(let i = 1; i <= pageCount; i++) {
        doc.setPage(i);
        doc.setFontSize(8);
        doc.text("Confidential - Internal Use Only", 105, 285, { align: "center" });
        doc.text("Page " + i + " of " + pageCount, 190, 285, { align: "right" });
      }
      
      doc.save("executive-report-" + new Date().getTime() + ".pdf");
    };

    // Initialize first tab (starts at Executive View, i.e., view-1)
    switchTab('view-1');

    // Table sort logic
    const tables = document.querySelectorAll('table[data-sortable]')
    tables.forEach(table => {
      let sortCol = -1, sortDir = 1
      const tbody = table.querySelector('tbody')
      const headers = table.querySelectorAll('thead th')

      headers.forEach((th, i) => {
        th.addEventListener('click', () => {
          if (sortCol === i) sortDir *= -1
          else { sortCol = i; sortDir = 1 }
          const rows = [...tbody.querySelectorAll('tr')]
          rows.sort((a, b) => {
            const av = a.cells[i]?.getAttribute('data-value') || a.cells[i]?.textContent || ''
            const bv = b.cells[i]?.getAttribute('data-value') || b.cells[i]?.textContent || ''
            const an = parseFloat(av.replace(/[^0-9.-]/g, '')), bn = parseFloat(bv.replace(/[^0-9.-]/g, ''))
            const cmp = !isNaN(an) && !isNaN(bn) ? an - bn : av.localeCompare(bv)
            return cmp * sortDir
          })
          rows.forEach(r => tbody.appendChild(r))
          headers.forEach(h => h.classList.remove('text-blue-600'))
          th.classList.add('text-blue-600')
        })
      })
    })

    // Search filter logic
    const searchInputs = document.querySelectorAll('input[data-table]')
    searchInputs.forEach(input => {
      const tableId = input.getAttribute('data-table')
      const table = document.getElementById(tableId)
      if (!table) return
      input.addEventListener('input', () => {
        const q = input.value.toLowerCase()
        table.querySelectorAll('tbody tr').forEach(row => {
          row.style.display = row.textContent.toLowerCase().includes(q) ? '' : 'none'
        })
      })
    })
  })()
"#;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

fn is_numeric(v: &serde_json::Value) -> bool {
    v.is_number() || (v.is_string() && v.as_str().and_then(|s| s.parse::<f64>().ok()).is_some())
}

fn is_date_like(col: &str, v: &serde_json::Value) -> bool {
    let name_hint = col.to_lowercase().ends_with("date")
        || col.to_lowercase().ends_with("time")
        || col.to_lowercase().ends_with("at")
        || col.to_lowercase().ends_with("day")
        || col.to_lowercase().contains("created")
        || col.to_lowercase().contains("updated");
    let val_hint = v.is_string() && v.as_str().map(|s| s.len() >= 10 && s[0..4].chars().all(|c| c.is_ascii_digit())).unwrap_or(false);
    name_hint || val_hint
}

fn humanize(s: &str) -> String {
    let clean = s.trim_start_matches("u.").trim_start_matches("o.").trim_start_matches("i.");
    let mut out = String::new();
    let mut prev_char: Option<char> = None;
    for c in clean.chars() {
        if c == '_' {
            out.push(' ');
        } else if c.is_uppercase() {
            if let Some(prev) = prev_char {
                if !prev.is_whitespace() && !prev.is_uppercase() {
                    out.push(' ');
                }
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

fn is_money_col(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("price") || n.contains("total") || n.contains("revenue") || n.contains("salary")
        || n.contains("amount") || n.contains("tax") || n.contains("cost") || n.contains("fee")
        || n.contains("sum") || n.contains("balance") || n.contains("vlr") || n.contains("valor")
        || n.contains("pagamento")
}

fn format_value(v: &serde_json::Value, col: &str) -> String {
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
        return format!("{:.2}", n);
    }

    if v.is_boolean() {
        return v.as_bool().unwrap().to_string();
    }

    v.as_str().unwrap_or(&v.to_string()).to_string()
}

fn render_premium_header(title: &str, date: &str) -> String {
    format!(
        r#"
  <header class="px-6 lg:px-12 py-6">
    <div class="max-w-[1600px] mx-auto flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
      <div>
        <div class="flex items-center space-x-2 mb-1">
          <span class="px-2 py-0.5 bg-blue-500/10 text-blue-400 text-[9px] font-black uppercase tracking-[0.2em] rounded-md border border-blue-500/20">Autonomous BI</span>
        </div>
        <h1 class="text-2xl font-black text-[var(--text-main)] tracking-tighter leading-none mb-1">{}</h1>
        <p class="text-[var(--text-muted)] text-[10px] font-semibold flex items-center">
          <i data-lucide="calendar" class="w-3 h-3 mr-1.5"></i> {}
        </p>
      </div>
      <div class="flex items-center space-x-3 no-print">
      </div>
    </div>
  </header>"#,
        esc(title), esc(date)
    )
}

fn render_single_stat_card(
    label: &str,
    value: &str,
    subtitle: &str,
    icon: &str,
    color: &str,
    span: i64,
) -> String {
    let is_hex = color.starts_with('#');
    let icon_color_style = if is_hex {
        format!("style=\"color: {}\"", color)
    } else {
        format!("class=\"text-{}-400\"", color)
    };
    let bg_simple_style = if is_hex {
        format!("style=\"background-color: {}1a; color: {}\"", color, color)
    } else {
        format!("class=\"p-1.5 bg-{}-500/10 rounded-lg text-{}-400 border border-{}-500/20\"", color, color, color)
    };

    format!(
        r#"
    <div class="bg-[var(--card-bg)] border border-[var(--border)] p-4 rounded-xl relative group overflow-hidden col-span-6 sm:col-span-4 lg:col-span-{} backdrop-blur-xl">
      <div class="absolute -right-2 -bottom-2 opacity-5 group-hover:scale-110 transition-transform duration-500">
        <i data-lucide="{}" class="w-12 h-12" {}></i>
      </div>
      <div class="flex items-center justify-between mb-3">
        <div class="p-1.5 rounded-lg border" {}><i data-lucide="{}" class="w-3.5 h-3.5"></i></div>
        <span class="text-[8px] font-black text-[var(--text-muted)] uppercase tracking-[0.2em]">{}</span>
      </div>
      <div class="text-xl font-black text-[var(--text-main)] tracking-tighter">{}</div>
      <div class="text-[9px] font-bold text-[var(--text-muted)] mt-0.5 uppercase tracking-widest">{}</div>
    </div>"#,
        span, icon, icon_color_style, bg_simple_style, icon, esc(label), value, esc(subtitle)
    )
}

fn render_explanation(explanation_text: &str, span: i64) -> String {
    format!(
        r#"
  <div class="bg-blue-600/5 border border-blue-500/10 rounded-xl p-4 flex items-center space-x-4 mb-4 col-span-12 lg:col-span-{}">
    <div class="bg-blue-600/20 p-2 rounded-lg text-blue-400">
      <i data-lucide="sparkles" class="w-4 h-4"></i>
    </div>
    <div class="flex-1">
      <p class="text-white text-xs font-semibold leading-snug opacity-90 italic">"{}"</p>
    </div>
  </div>"#,
        span, esc(explanation_text)
    )
}

fn render_stat_cards(
    results: &[serde_json::Value],
    columns: &[String],
    visual_suggestions: Option<&serde_json::Value>,
    span: i64,
) -> String {
    if results.is_empty() {
        return String::new();
    }
    let first_row = &results[0];
    
    let ai_focus: Vec<String> = visual_suggestions
        .and_then(|s| s.get("summaryFields").and_then(|v| v.as_array()))
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let focus_cols: Vec<String> = if !ai_focus.is_empty() {
        ai_focus.into_iter().filter(|c| columns.contains(c)).collect()
    } else {
        columns
            .iter()
            .filter(|c| is_numeric(first_row.get(*c).unwrap_or(&serde_json::Value::Null)) && !c.to_lowercase().ends_with("id"))
            .take(3)
            .cloned()
            .collect()
    };

    let suggested_colors = visual_suggestions
        .and_then(|s| s.get("colors").and_then(|v| v.as_array()))
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_else(|| {
            vec![
                "#3b82f6".to_string(),
                "#6366f1".to_string(),
                "#10b981".to_string(),
                "#f59e0b".to_string(),
                "#ef4444".to_string(),
            ]
        });

    let mut cards = Vec::new();
    cards.push(render_single_stat_card("Volume", &results.len().to_string(), "Total records", "database", &suggested_colors[0], span));

    let icons = ["trending-up", "zap", "bar-chart"];
    for (idx, col) in focus_cols.iter().enumerate() {
        let values: Vec<f64> = results
            .iter()
            .filter_map(|r| r.get(col).and_then(|v| {
                if v.is_number() {
                    v.as_f64()
                } else if v.is_string() {
                    v.as_str().and_then(|s| s.parse::<f64>().ok())
                } else {
                    None
                }
            }))
            .collect();
        
        if values.is_empty() {
            continue;
        }
        let sum: f64 = values.iter().sum();
        let color = &suggested_colors[(idx + 1) % suggested_colors.len()];
        let icon = icons[idx % icons.len()];
        let formatted = format_value(&serde_json::Value::from(sum), col);

        cards.push(render_single_stat_card("Indicator", &formatted, &humanize(col), icon, color, span));
    }

    cards.join("\n")
}

fn render_table_component(results: &[serde_json::Value], columns: &[String], config: &serde_json::Value) -> String {
    let title = config.get("title").and_then(|v| v.as_str()).unwrap_or("Data Explorer");
    let subtitle = config.get("subtitle").and_then(|v| v.as_str()).unwrap_or("Detailed record-level inspection");
    let span = config.get("colSpan").and_then(|v| v.as_i64()).unwrap_or(12);

    let table_id = format!("tbl-{}", &uuid::Uuid::new_v4().to_string()[0..8]);
    let headers = columns.iter().map(|c| {
        format!(
            "<th class=\"px-4 py-3 text-left text-[10px] font-black text-[var(--text-muted)] uppercase tracking-[0.1em] border-b border-[var(--border)] cursor-pointer hover:text-blue-400 transition-colors\">{}</th>",
            esc(&humanize(c))
        )
    }).collect::<Vec<String>>().join("");

    let body_rows = results.iter().map(|row| {
        let cells = columns.iter().map(|c| {
            let val = row.get(c).unwrap_or(&serde_json::Value::Null);
            let formatted = format_value(val, c);
            format!(
                "<td class=\"px-4 py-3 whitespace-nowrap text-xs text-[var(--text-main)] font-medium border-b border-[var(--border)]/50 group-last:border-none\" data-value=\"{}\">{}</td>",
                esc(&val.to_string()),
                esc(&formatted)
            )
        }).collect::<Vec<String>>().join("");
        format!("<tr class=\"group hover:bg-blue-500/5 transition-all duration-300\">{}</tr>", cells)
    }).collect::<Vec<String>>().join("\n");

    format!(
        r#"
  <section class="bg-[var(--card-bg)] border border-[var(--border)] rounded-[1.5rem] p-6 backdrop-blur-xl col-span-12 lg:col-span-{}">
    <div class="flex items-center justify-between mb-6">
      <div class="flex items-center space-x-3">
        <div class="p-2 bg-[var(--bg-main)] rounded-xl border border-[var(--border)] text-[var(--text-muted)]">
          <i data-lucide="table" class="w-5 h-5"></i>
        </div>
        <div>
          <h2 class="text-lg font-bold text-[var(--text-main)] tracking-tight">{}</h2>
          <p class="text-[var(--text-muted)] text-xs font-medium">{}</p>
        </div>
      </div>
    </div>
    <div class="mb-6">
      <div class="relative group">
        <div class="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none">
          <i data-lucide="search" class="w-4 h-4 text-[var(--text-muted)] group-focus-within:text-blue-500 transition-colors"></i>
        </div>
        <input type="text" data-table="{}" class="block w-full pl-11 pr-4 py-3 border border-[var(--border)] rounded-xl leading-5 bg-[var(--bg-main)] text-[var(--text-main)] placeholder-[var(--text-muted)] focus:outline-none focus:ring-4 focus:ring-blue-500/10 focus:border-blue-500 text-xs transition-all" placeholder="Search data points...">
      </div>
    </div>
    <div class="overflow-hidden border border-[var(--border)] rounded-xl bg-[var(--bg-main)]/50">
      <div class="overflow-x-auto">
        <table id="{}" data-sortable class="min-w-full divide-y divide-[var(--border)]">
          <thead class="bg-[var(--bg-main)]"><tr>{}</tr></thead>
          <tbody class="divide-y divide-[var(--border)]/50">{}</tbody>
        </table>
      </div>
    </div>
  </section>"#,
        span, esc(title), esc(subtitle), table_id, table_id, headers, body_rows
    )
}

fn render_apex_chart_component(
    results: &[serde_json::Value],
    columns: &[String],
    config: &serde_json::Value,
    theme: &str,
    visual_suggestions: Option<&serde_json::Value>,
) -> String {
    let chart_type = config.get("chartType").and_then(|v| v.as_str()).unwrap_or("bar");
    let title = config.get("title").and_then(|v| v.as_str()).unwrap_or("Insight");
    let subtitle = config.get("subtitle").and_then(|v| v.as_str()).unwrap_or("Data visualization");
    let span = config.get("colSpan").and_then(|v| v.as_i64()).unwrap_or(6);
    let height = config.get("height").and_then(|v| v.as_i64()).unwrap_or(220);

    let chart_id = format!("apex-{}", &uuid::Uuid::new_v4().to_string()[0..6]);
    let first_row = &results[0];

    let value_cols: Vec<String> = config
        .get("valueCols")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_else(|| {
            columns
                .iter()
                .filter(|c| is_numeric(first_row.get(*c).unwrap_or(&serde_json::Value::Null)) && !c.to_lowercase().ends_with("id"))
                .take(3)
                .cloned()
                .collect()
        });

    let date_cols: Vec<String> = columns
        .iter()
        .filter(|c| is_date_like(c, first_row.get(*c).unwrap_or(&serde_json::Value::Null)))
        .cloned()
        .collect();

    let text_cols: Vec<String> = columns
        .iter()
        .filter(|c| {
            let v = first_row.get(*c).unwrap_or(&serde_json::Value::Null);
            !is_numeric(v) && !c.to_lowercase().ends_with("id")
        })
        .cloned()
        .collect();

    let label_col = config
        .get("labelCol")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .or_else(|| date_cols.first().cloned())
        .or_else(|| text_cols.first().cloned())
        .unwrap_or_else(|| columns[0].clone());

    let labels: Vec<String> = results
        .iter()
        .map(|r| r.get(&label_col).map(|v| v.as_str().unwrap_or(&v.to_string()).to_string()).unwrap_or_default())
        .collect();

    let is_pie = chart_type == "donut" || chart_type == "pie" || chart_type == "radialBar";

    let suggested_colors = config
        .get("colors")
        .and_then(|v| v.as_array())
        .or_else(|| visual_suggestions.and_then(|s| s.get("colors").and_then(|v| v.as_array())))
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_else(|| {
            vec![
                "#3b82f6".to_string(),
                "#6366f1".to_string(),
                "#10b981".to_string(),
                "#f59e0b".to_string(),
                "#ef4444".to_string(),
            ]
        });

    let series_json = if is_pie {
        if !value_cols.is_empty() {
            let data: Vec<f64> = results
                .iter()
                .filter_map(|r| r.get(&value_cols[0]).and_then(|v| {
                    if v.is_number() {
                        v.as_f64()
                    } else {
                        v.as_str().and_then(|s| s.parse::<f64>().ok())
                    }
                }))
                .collect();
            serde_json::json!(data)
        } else {
            serde_json::json!(Vec::<f64>::new())
        }
    } else {
        let mut list = Vec::new();
        for col in &value_cols {
            let data: Vec<f64> = results
                .iter()
                .filter_map(|r| r.get(col).and_then(|v| {
                    if v.is_number() {
                        v.as_f64()
                    } else {
                        v.as_str().and_then(|s| s.parse::<f64>().ok())
                    }
                }))
                .collect();
            list.push(serde_json::json!({
                "name": humanize(col),
                "data": data
            }));
        }
        serde_json::json!(list)
    };

    let mut plot_options = serde_json::json!({});
    let fill = serde_json::json!({ "type": "solid", "opacity": 1 });
    let xaxis = serde_json::json!({
        "categories": labels,
        "axisBorder": { "show": false },
        "axisTicks": { "show": false },
        "labels": { "style": { "fontSize": "10px" } }
    });

    if chart_type == "radialBar" {
        if let serde_json::Value::Array(ref arr) = series_json {
            let raw_val = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            let _display_val = raw_val.clamp(0.0, 100.0);
            plot_options = serde_json::json!({
                "radialBar": {
                    "startAngle": -135,
                    "endAngle": 135,
                    "hollow": { "size": "70%" },
                    "track": {
                        "background": if theme == "light" { "#e2e8f0" } else { "#1e293b" },
                        "strokeWidth": "100%"
                    },
                    "dataLabels": {
                        "name": { "show": true, "color": if theme == "dark" { "#94a3b8" } else { "#475569" }, "fontSize": "11px", "offsetY": -10 },
                        "value": {
                            "show": true,
                            "color": if theme == "dark" { "#fff" } else { "#0f172a" },
                            "fontSize": "22px",
                            "fontWeight": 700,
                            "offsetY": 5
                        }
                    }
                }
            });
        }
    } else if chart_type == "treemap" {
        plot_options = serde_json::json!({
            "treemap": {
                "enableShades": true,
                "shadeIntensity": 0.5,
                "distributed": true,
                "colorScale": { "ranges": [{ "from": 0, "to": 1000000, "color": "#3b82f6" }] }
            }
        });
    }

    let apex_config = serde_json::json!({
        "series": series_json,
        "colors": suggested_colors,
        "stroke": {
            "curve": "smooth",
            "width": if chart_type == "area" || chart_type == "line" { 3 } else { 0 }
        },
        "theme": { "mode": if theme == "light" { "light" } else { "dark" } },
        "chart": {
            "type": chart_type,
            "height": height,
            "toolbar": { "show": false },
            "background": "transparent",
            "foreColor": if theme == "light" { "#475569" } else { "#94a3b8" },
            "fontFamily": "Inter, sans-serif",
            "sparkline": { "enabled": false },
            "dropShadow": { "enabled": false }
        },
        "plotOptions": plot_options,
        "fill": fill,
        "xaxis": xaxis,
        "grid": {
            "borderColor": if theme == "light" { "#e2e8f0" } else { "#1e293b" },
            "strokeDashArray": 4,
            "padding": { "top": 0, "right": 0, "bottom": 0, "left": 10 }
        },
        "legend": {
            "position": "bottom",
            "horizontalAlign": "center",
            "fontWeight": 600,
            "fontSize": "11px",
            "markers": { "radius": 12 }
        }
    });

    format!(
        r##"
  <div class="bg-[var(--card-bg)] border border-[var(--border)] rounded-xl p-4 backdrop-blur-xl col-span-12 lg:col-span-{}">
    <div class="flex items-center justify-between mb-3">
      <div>
        <h3 class="text-sm font-bold text-[var(--text-main)] tracking-tight">{}</h3>
        <p class="text-[var(--text-muted)] text-[10px] font-medium mt-0">{}</p>
      </div>
      <div class="p-1.5 bg-[var(--bg-main)] rounded-lg border border-[var(--border)] text-[var(--text-muted)]">
        <i data-lucide="{}" class="w-3.5 h-3.5"></i>
      </div>
    </div>
    <div id="{}"></div>
    <script>
      new ApexCharts(document.querySelector("#{}"), {}).render();
    </script>
  </div>"##,
        span,
        esc(title),
        esc(subtitle),
        if chart_type == "area" { "trending-up" } else { "bar-chart" },
        chart_id,
        chart_id,
        apex_config
    )
}

fn render_component(
    results: &[serde_json::Value],
    columns: &[String],
    comp: &serde_json::Value,
    theme: &str,
    visual_suggestions: Option<&serde_json::Value>,
) -> String {
    let t = comp.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let config = comp.get("config").unwrap_or(&serde_json::Value::Null);

    match t {
        "kpi-grid" => render_stat_cards(results, columns, visual_suggestions, 3),
        "card" => {
            let col = config.get("metricCol").and_then(|v| v.as_str()).unwrap_or("");
            let values: Vec<f64> = results
                .iter()
                .filter_map(|r| r.get(col).and_then(|v| {
                    if v.is_number() {
                        v.as_f64()
                    } else {
                        v.as_str().and_then(|s| s.parse::<f64>().ok())
                    }
                }))
                .collect();
            let sum: f64 = values.iter().sum();
            let icon = config.get("icon").and_then(|v| v.as_str()).unwrap_or("activity");
            let col_span = config.get("colSpan").and_then(|v| v.as_i64()).unwrap_or(3);
            let formatted = format_value(&serde_json::Value::from(sum), col);
            render_single_stat_card("Indicator", &formatted, &humanize(col), icon, "indigo", col_span)
        }
        "chart" => render_apex_chart_component(results, columns, config, theme, visual_suggestions),
        "table" => render_table_component(results, columns, config),
        "explanation" => {
            let text = visual_suggestions.and_then(|s| s.get("explanation").and_then(|v| v.as_str())).unwrap_or("");
            let col_span = config.get("colSpan").and_then(|v| v.as_i64()).unwrap_or(12);
            render_explanation(text, col_span)
        }
        _ => String::new(),
    }
}

pub fn generate_html_report(title: &str, content_html: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{}</title>
  <style>{}</style>
</head>
<body>
  {}
  <footer>Generated by <strong>Uneven</strong> — Autonomous Local Development Agent</footer>
  <script>{}</script>
</body>
</html>"#,
        title, DASHBOARD_CSS, content_html, DASHBOARD_SCRIPTS
    )
}

pub fn generate_dashboard(
    results: &[serde_json::Value],
    title: &str,
    theme: &str,
    visual_suggestions: Option<&serde_json::Value>,
) -> String {
    if results.is_empty() {
        return generate_html_report(title, "<h1>No data returned</h1>");
    }

    let first_row = &results[0];
    let columns: Vec<String> = match first_row {
        serde_json::Value::Object(map) => map.keys().cloned().collect(),
        _ => Vec::new(),
    };

    let mut layout = Vec::new();
    if let Some(sug) = visual_suggestions {
        if let Some(l) = sug.get("layout") {
            if let Some(arr) = l.as_array() {
                layout = arr.clone();
            }
        }
    }
    
    if layout.is_empty() {
        if let Some(sug) = visual_suggestions {
            if sug.get("explanation").is_some() {
                layout.push(serde_json::json!({
                    "type": "explanation",
                    "config": { "title": "AI Insights", "colSpan": 12 }
                }));
            }
        }
        
        layout.push(serde_json::json!({
            "type": "kpi-grid",
            "config": { "title": "Strategic Metrics" }
        }));
        
        let has_numeric = columns.iter().any(|c| first_row.get(c).map(is_numeric).unwrap_or(false));
        if has_numeric {
            let has_date = columns.iter().any(|c| first_row.get(c).map(|v| is_date_like(c, v)).unwrap_or(false));
            if has_date {
                layout.push(serde_json::json!({
                    "type": "chart",
                    "config": { "chartType": "area", "title": "Trend Analysis", "subtitle": "Performance over time", "colSpan": 6 }
                }));
            } else {
                layout.push(serde_json::json!({
                    "type": "chart",
                    "config": { "chartType": "bar", "title": "Distribution", "subtitle": "Primary metric ranking", "colSpan": 6 }
                }));
            }
            
            let has_text = columns.iter().any(|c| {
                let v = first_row.get(c).unwrap_or(&serde_json::Value::Null);
                !is_numeric(v) && !is_date_like(c, v) && !c.to_lowercase().ends_with("id")
            });
            if has_text {
                layout.push(serde_json::json!({
                    "type": "chart",
                    "config": { "chartType": "donut", "title": "Composition", "subtitle": "Proportional breakdown", "colSpan": 6 }
                }));
            }
        }
        
        layout.push(serde_json::json!({
            "type": "table",
            "config": { "title": "Data Explorer", "subtitle": "Detailed record-level inspection" }
        }));
    }

    let mut dynamic_tabs = Vec::new();
    let mut current_tab_components = Vec::new();
    let mut current_tab_height = 0;
    let mut current_x = 0;
    let mut current_row_max_height = 0;
    
    let mut components = Vec::new();
    let mut tables = Vec::new();
    
    for comp in layout {
        if let Some(t) = comp.get("type").and_then(|v| v.as_str()) {
            if t == "table" {
                tables.push(comp);
            } else {
                components.push(comp);
            }
        }
    }
    
    let mut tab_id = 1;
    for comp in components {
        let config = comp.get("config");
        let span = config.and_then(|c| c.get("colSpan").and_then(|v| v.as_i64())).unwrap_or(6) as i32;
        let h = config.and_then(|c| c.get("height").and_then(|v| v.as_i64())).unwrap_or(220) as i32;
        let t = comp.get("type").and_then(|v| v.as_str()).unwrap_or("");
        
        let actual_h = if t == "card" || t == "kpi-grid" {
            130
        } else {
            h
        };
        
        if current_x + span > 12 {
            current_tab_height += current_row_max_height + 16;
            current_x = 0;
            current_row_max_height = 0;
        }
        
        if current_tab_height + actual_h > 550 && !current_tab_components.is_empty() {
            dynamic_tabs.push(serde_json::json!({
                "id": format!("view-{}", tab_id),
                "name": if tab_id == 1 { "Executive View".to_string() } else { format!("Analytics {}", tab_id - 1) },
                "components": current_tab_components
            }));
            tab_id += 1;
            current_tab_components = Vec::new();
            current_tab_height = 0;
            current_x = 0;
            current_row_max_height = 0;
        }
        
        current_tab_components.push(comp);
        current_x += span;
        current_row_max_height = std::cmp::max(current_row_max_height, actual_h);
    }
    
    if !current_tab_components.is_empty() {
        dynamic_tabs.push(serde_json::json!({
            "id": format!("view-{}", tab_id),
            "name": if tab_id == 1 { "Executive View".to_string() } else { format!("Analytics {}", tab_id - 1) },
            "components": current_tab_components
        }));
    }
    
    if !tables.is_empty() {
        dynamic_tabs.push(serde_json::json!({
            "id": "data-explorer",
            "name": "Data Explorer",
            "components": tables.iter().map(|t| {
                let mut tc = t.clone();
                if let Some(c) = tc.get_mut("config") {
                    if let Some(map) = c.as_object_mut() {
                        map.insert("colSpan".to_string(), serde_json::json!(12));
                    }
                }
                tc
            }).collect::<Vec<serde_json::Value>>()
        }));
    }

    let date = chrono::Local::now().format("%Y-%m-%d %H:%M LOCAL").to_string();

    let custom_colors_style = if let Some(colors) = visual_suggestions.and_then(|s| s.get("colors").and_then(|v| v.as_array())) {
        if !colors.is_empty() {
            let primary = colors[0].as_str().unwrap_or("#3b82f6");
            let accent = colors.get(1).and_then(|v| v.as_str()).unwrap_or("#8b5cf6");
            let bg = colors.get(2).and_then(|v| v.as_str()).unwrap_or("#0f172a");
            let text = colors.get(3).and_then(|v| v.as_str()).unwrap_or("#f1f5f9");
            format!(
                r#"
  <style>
    :root {{
      --primary: {};
      --primary-glow: {}80;
      --accent: {};
      --bg-main: {};
      --text-main: {};
    }}
  </style>"#,
                primary, primary, accent, bg, text
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let sidebar_nav_links = dynamic_tabs.iter().enumerate().map(|(idx, tab)| {
        let id = tab.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let name = tab.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let icon = if id == "data-explorer" {
            "database"
        } else if id == "view-1" {
            "layout-grid"
        } else {
            "activity"
        };
        let active_classes = if idx == 0 { "text-blue-500 bg-blue-500/10" } else { "text-slate-500" };
        format!(
            r#"<a href="javascript:void(0)" onclick="switchTab('{}')" data-target="{}" title="{}" class="p-2.5 rounded-xl transition-all {}"><i data-lucide="{}" class="w-5 h-5"></i></a>"#,
            id, id, esc(name), active_classes, icon
        )
    }).collect::<Vec<String>>().join("\n");

    let tab_sections = dynamic_tabs.iter().map(|tab| {
        let id = tab.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let comps = tab.get("components").and_then(|v| v.as_array()).unwrap();
        let rendered_comps = comps.iter().map(|c| render_component(results, &columns, c, theme, visual_suggestions)).collect::<Vec<String>>().join("\n");
        format!(
            r#"<section id="{}" data-tab class="hidden grid grid-cols-12 gap-4">{}</section>"#,
            id, rendered_comps
        )
    }).collect::<Vec<String>>().join("\n");

    let executive_report_text = visual_suggestions
        .and_then(|s| s.get("executiveReport").or_else(|| s.get("report")).and_then(|v| v.as_str()))
        .unwrap_or("No report available.");

    format!(
        r#"<!DOCTYPE html>
<html lang="en" class="h-full">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{}</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <script src="https://cdn.jsdelivr.net/npm/apexcharts"></script>
  <script src="https://unpkg.com/lucide@latest"></script>
  <script src="https://cdnjs.cloudflare.com/ajax/libs/jspdf/2.5.1/jspdf.umd.min.js"></script>
  <style>{}</style>
  {}
</head>
<body class="{} bg-[var(--bg-main)] text-[var(--text-main)] min-h-screen h-screen font-['Inter'] selection:bg-blue-500/30 overflow-hidden flex flex-col">
  
  <div class="fixed left-0 top-0 h-full w-16 bg-slate-900 border-r border-slate-800 flex flex-col items-center py-6 z-50 hidden lg:flex no-print">
    <div onclick="switchTab('view-1')" class="w-10 h-10 bg-blue-600 rounded-xl flex items-center justify-center text-white font-black text-xl shadow-lg shadow-blue-500/20 mb-8 cursor-pointer hover:scale-105 transition-transform">U</div>
    <nav class="flex flex-col space-y-4">
      {}
    </nav>
  </div>

  <div class="lg:pl-16 flex-1 flex flex-col overflow-hidden">
    {}
    
    <main class="max-w-[1600px] w-full mx-auto px-6 lg:px-12 flex-1 overflow-y-auto custom-scrollbar pb-6">
      
      {}

    </main>
    
    <footer class="px-6 lg:px-12 py-3 border-t border-[var(--border)] flex justify-between items-center opacity-40 no-print">
      <div class="text-[8px] font-bold tracking-widest uppercase text-[var(--text-muted)]">Uneven Engine v1.12.0</div>
      <div class="text-[8px] font-bold text-[var(--text-muted)]">Confidential BI Report</div>
    </footer>
  </div>

  <script>
    window.EXECUTIVE_REPORT = {:?};
    {}
  </script>
</body>
</html>"#,
        esc(title),
        DASHBOARD_CSS,
        custom_colors_style,
        if theme != "dark" { format!("{}-mode", theme) } else { String::new() },
        sidebar_nav_links,
        render_premium_header(title, &date),
        tab_sections,
        executive_report_text,
        DASHBOARD_SCRIPTS
    )
}
