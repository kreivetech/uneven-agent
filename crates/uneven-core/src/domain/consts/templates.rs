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

    // Initialize first tab
    switchTab('summary-view');

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

pub const CHART_JS_CDN: &str = "https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js";

pub fn build_report_viewer_script(html: &str, title: &str) -> String {
    let escaped_html = html
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");

    format!(
        r#"'use strict';
const http = require('http');
const {{ exec }} = require('child_process');
const os = require('os');

const TITLE = {};
const HTML = `{}`;
const TIMEOUT_MS = 30 * 60 * 1000; // 30 mins

let inactivityTimer;
function resetTimer(server) {{
  clearTimeout(inactivityTimer);
  inactivityTimer = setTimeout(() => {{
    server.close(() => process.exit(0));
  }}, TIMEOUT_MS);
}}

const server = http.createServer((req, res) => {{
  resetTimer(server);
  if (req.url === '/close') {{
    res.writeHead(200, {{ 'Content-Type': 'text/plain' }});
    res.end('Dashboard closed.');
    setTimeout(() => {{ server.close(() => process.exit(0)); }}, 300);
    return;
  }}
  if (req.url === '/health') {{
    res.writeHead(200, {{ 'Content-Type': 'text/plain' }});
    res.end('ok');
    return;
  }}
  res.writeHead(200, {{
    'Content-Type': 'text/html; charset=utf-8',
    'Content-Security-Policy': "default-src 'self' 'unsafe-inline' 'unsafe-eval'; img-src 'self' data:; connect-src 'none'",
    'X-Frame-Options': 'DENY',
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'no-store',
  }});
  res.end(HTML);
}});

server.listen(0, '127.0.0.1', () => {{
  const {{ port }} = server.address();
  const url = `http://127.0.0.1:${{port}}`;

  console.log('\n  Uneven Dashboard Report\n  ' + '─'.repeat(40));
  console.log('  Title    : ' + TITLE);
  console.log('  URL      : ' + url + '\n');

  const platform = os.platform();
  const cmd = platform === 'win32' ? `start "" "${{url}}"` : platform === 'darwin' ? `open "${{url}}"` : `xdg-open "${{url}}"`;
  exec(cmd);
  resetTimer(server);
}});

process.on('SIGINT', () => server.close(() => process.exit(0)));
"#,
        serde_json::to_string(title).unwrap(),
        escaped_html
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_security_html_report(
    project_name: &str,

    target: &str,
    date: &str,
    findings_html: &str,
    summary_critical: u32,
    summary_high: u32,
    summary_medium: u32,
    summary_low: u32,
    summary_info: u32,
    reremediation_html: &str,
    risk_level_label: &str,
    risk_level_color: &str,
) -> String {
    let esc = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    };

    let remediation_section = if !reremediation_html.is_empty() {
        format!(
            "<section><h2>Remediation Plan</h2><ol>{}</ol></section>",
            reremediation_html
        )
    } else {
        "".to_string()
    };

    let findings_section = if findings_html.is_empty() {
        "<div class=\"empty\">✅ No security issues found.</div>".to_string()
    } else {
        format!(
            "<table><thead><tr><th>#</th><th>Severity</th><th>Type</th><th>Description</th><th>Location</th><th>CVSS</th><th>Remediation</th></tr></thead><tbody>{}</tbody></table>",
            findings_html
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Security Report — {0}</title>
  <style>
    *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; font-size: 14px; color: #24292e; background: #f6f8fa; }}
    .container {{ max-width: 1100px; margin: 0 auto; padding: 32px 16px; }}
    header {{ background: #24292e; color: #fff; padding: 24px 32px; border-radius: 8px; margin-bottom: 24px; }}
    header h1 {{ font-size: 22px; font-weight: 700; }}
    header p {{ color: #8b949e; margin-top: 4px; font-size: 13px; }}
    .risk-banner {{ display: inline-block; margin-top: 12px; padding: 4px 14px; border-radius: 4px; font-weight: 700; font-size: 13px; color: #fff; background: {1}; }}
    .summary-grid {{ display: grid; grid-template-columns: repeat(5, 1fr); gap: 12px; margin-bottom: 24px; }}
    .summary-card {{ background: #fff; border: 1px solid #e1e4e8; border-radius: 8px; padding: 16px; text-align: center; }}
    .summary-card .count {{ font-size: 32px; font-weight: 700; }}
    .summary-card .label {{ font-size: 12px; color: #6a737d; margin-top: 4px; }}
    .summary-card.critical .count {{ color: #d73a49; }}
    .summary-card.high .count {{ color: #e36209; }}
    .summary-card.medium .count {{ color: #b08800; }}
    .summary-card.low .count {{ color: #0075ca; }}
    .summary-card.info .count {{ color: #6f42c1; }}
    section {{ background: #fff; border: 1px solid #e1e4e8; border-radius: 8px; padding: 24px; margin-bottom: 20px; }}
    section h2 {{ font-size: 18px; font-weight: 600; margin-bottom: 16px; padding-bottom: 8px; border-bottom: 1px solid #e1e4e8; }}
    table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
    th {{ text-align: left; padding: 8px 12px; background: #f6f8fa; border: 1px solid #e1e4e8; font-weight: 600; }}
    td {{ padding: 8px 12px; border: 1px solid #e1e4e8; vertical-align: top; }}
    tr:nth-child(even) td {{ background: #fafbfc; }}
    code {{ font-family: SFMono-Regular, Consolas, monospace; font-size: 12px; background: #f6f8fa; padding: 2px 4px; border-radius: 3px; border: 1px solid #e1e4e8; }}
    ol {{ padding-left: 20px; }}
    ol li {{ margin: 8px 0; }}
    .empty {{ text-align: center; color: #28a745; padding: 24px; font-size: 16px; }}
    footer {{ text-align: center; color: #6a737d; font-size: 12px; margin-top: 24px; }}
  </style>
</head>
<body>
  <div class="container">
    <header>
      <h1>Security Report — {0}</h1>
      <p>Target: {2} &nbsp;·&nbsp; Generated: {3} &nbsp;·&nbsp; Tool: Uneven Security Analyzer</p>
      <div class="risk-banner">{4}</div>
    </header>

    <div class="summary-grid">
      <div class="summary-card critical"><div class="count">{5}</div><div class="label">Critical</div></div>
      <div class="summary-card high"><div class="count">{6}</div><div class="label">High</div></div>
      <div class="summary-card medium"><div class="count">{7}</div><div class="label">Medium</div></div>
      <div class="summary-card low"><div class="count">{8}</div><div class="label">Low</div></div>
      <div class="summary-card info"><div class="count">{9}</div><div class="label">Info</div></div>
    </div>

    <section>
      <h2>Findings</h2>
      {10}
    </section>

    {11}
    <footer>Generated by <strong>Uneven</strong> — Autonomous Local Development Agent</footer>
  </div>
</body>
</html>"#,
        esc(project_name),
        risk_level_color,
        esc(target),
        date,
        esc(risk_level_label),
        summary_critical,
        summary_high,
        summary_medium,
        summary_low,
        summary_info,
        findings_section,
        remediation_section
    )
}
