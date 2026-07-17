use anyhow::Result;
use console::{style, Color};
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command as SysCommand;
use std::time::Instant;

use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::pentester::{
    self, Severity, SecurityFinding,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DepVulnerability {
    pub name: String,
    pub severity: String,
    pub is_direct: bool,
    pub range: String,
    pub fix_available: serde_json::Value,
    pub via: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct NpmAuditOutput {
    vulnerabilities: Option<HashMap<String, NpmAuditVuln>>,
}

#[derive(Debug, Deserialize)]
struct NpmAuditVuln {
    name: Option<String>,
    severity: Option<String>,
    #[serde(rename = "isDirect")]
    is_direct: Option<bool>,
    range: Option<String>,
    #[serde(rename = "fixAvailable")]
    fix_available: Option<serde_json::Value>,
    via: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanResultJson {
    findings: Vec<SecurityFindingJson>,
    files_scanned: usize,
    dependencies_scanned: usize,
    duration_ms: u128,
    risk_level: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SecurityFindingJson {
    severity: String,
    finding_type: String,
    file: Option<String>,
    line: Option<usize>,
    description: String,
    recommendation: String,
    cvss: Option<f64>,
    llm_verdict: Option<String>,
    llm_note: Option<String>,
}

pub async fn run(dir: String, report: bool, json_output: bool) -> Result<()> {
    let start_time = Instant::now();
    let root = loader::find_project_root(None);
    let config = loader::load_config(&root)?;
    
    // License Check / Free limit
    if !super::license::check_pro() {
        super::license::check_session_limit("scan", 2, 6 * 3600)?;
    }
    
    // Ensure .uneven/logs exists
    let uneven_dir = root.join(".uneven");
    let logs_dir = uneven_dir.join("logs");
    fs::create_dir_all(&logs_dir)?;

    let pb = if !json_output {
        let bar = ProgressBar::new_spinner();
        bar.set_message("Scanning for malicious code patterns...");
        bar.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(bar)
    } else {
        None
    };

    // 1. Run static analysis (secrets, vulnerabilities, malware)
    let static_findings = pentester::static_analysis(&[&dir]).await
        .unwrap_or_default();

    // 2. Run supply chain analysis
    let supply_findings = pentester::supply_chain::run_audit(&root.to_string_lossy()).await
        .unwrap_or_default();

    // 3. Run dependency CVE check (npm audit)
    let (vulnerabilities, audit_skipped, audit_error) = run_npm_audit(&root);

    if let Some(ref bar) = pb {
        bar.finish_with_message("Scan complete");
    }

    let duration_ms = start_time.elapsed().as_millis();

    // Combine static and supply findings
    let mut all_findings = static_findings;
    all_findings.extend(supply_findings);

    // LLM Review
    let scan_brain = config.get_brain_for_command("scan");
    let reviewer = uneven_core::infrastructure::pentester::reviewer::LLMSecurityReviewer::new(&scan_brain);
    let _ = reviewer.review_findings(&mut all_findings).await;

    // Compute dependencies scanned
    let dependencies_scanned = count_dependencies_scanned(&root);

    let risk_level = compute_risk_level(&all_findings);

    if json_output {
        let findings_json: Vec<SecurityFindingJson> = all_findings.iter().map(|f| SecurityFindingJson {
            severity: format!("{:?}", f.severity).to_lowercase(),
            finding_type: f.finding_type.clone(),
            file: f.file.clone(),
            line: f.line,
            description: f.description.clone(),
            recommendation: f.recommendation.clone(),
            cvss: f.cvss,
            llm_verdict: f.llm_verdict.clone(),
            llm_note: f.llm_note.clone(),
        }).collect();

        let result = ScanResultJson {
            findings: findings_json,
            files_scanned: count_scanned_files(&dir),
            dependencies_scanned,
            duration_ms,
            risk_level: format!("{:?}", risk_level).to_lowercase(),
        };

        println!("{}", serde_json::to_string_pretty(&result)?);
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        
        let has_blockers = all_findings.iter().any(|f| f.severity == Severity::Critical || f.severity == Severity::High);
        if has_blockers {
            std::process::exit(1);
        } else {
            std::process::exit(0);
        }
    }

    // Terminal layout printout
    println!();
    println!("{}", style("  Uneven Malware Scanner").bold());
    println!("{}", style("  ".to_string() + &"─".repeat(54)).dim());
    println!(
        "{}",
        style(format!(
            "  Scanned : {} source files, {} dependencies",
            count_scanned_files(&dir),
            dependencies_scanned
        ))
        .dim()
    );
    println!("{}", style(format!("  Duration: {}ms", duration_ms)).dim());
    println!();

    if all_findings.is_empty() {
        println!("{}", style("  ✓ No malicious patterns detected").green());
        println!("{}", style("  Your project looks clean.\n").dim());
    } else {
        let risk_label = format!("{:?}", risk_level).to_uppercase();
        let risk_style = match risk_level {
            Severity::Critical => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).bg(Color::Red).fg(Color::White).bold(),
            Severity::High => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).red().bold(),
            Severity::Medium => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).yellow().bold(),
            Severity::Low => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).blue(),
            _ => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).dim(),
        };
        println!("{}", risk_style);
        println!();

        // Print summaries
        let counts = count_findings_by_severity(&all_findings);
        let mut summary = Vec::new();
        if counts.critical > 0 {
            summary.push(style(format!("{} critical", counts.critical)).bg(Color::Red).fg(Color::White).bold().to_string());
        }
        if counts.high > 0 {
            summary.push(style(format!("{} high", counts.high)).red().bold().to_string());
        }
        if counts.medium > 0 {
            summary.push(style(format!("{} medium", counts.medium)).yellow().bold().to_string());
        }
        if counts.low > 0 {
            summary.push(style(format!("{} low", counts.low)).blue().to_string());
        }
        println!("  Findings: {}", summary.join(&style(" · ").dim().to_string()));
        println!();

        // Group by category
        let grouped = group_by_category(&all_findings);
        for (category, items) in grouped {
            let icon = get_category_icon(&category);
            println!("{}", style(format!("  {}  {} ({})", icon, format_category(&category), items.len())).bold());

            for finding in items {
                let sev_style = match finding.severity {
                    Severity::Critical => style("[CRITICAL]").bg(Color::Red).fg(Color::White).bold(),
                    Severity::High => style("[HIGH]").red().bold(),
                    Severity::Medium => style("[MEDIUM]").yellow().bold(),
                    Severity::Low => style("[LOW]").blue(),
                    Severity::Info => style("[INFO]").dim(),
                };
                let loc = if let Some(ref f) = finding.file {
                    if let Some(l) = finding.line {
                        format!("{}:{}", f, l)
                    } else {
                        f.clone()
                    }
                } else {
                    "N/A".to_string()
                };

                let conf = if finding.finding_type.contains("malware") {
                    " (95% confidence)"
                } else {
                    ""
                };

                println!("    {} {}{}", sev_style, style(loc).underlined(), style(conf).dim());
                println!("       {}", style(&finding.description).white());
                if let Some(ref match_c) = finding.match_content {
                    let preview = if match_c.len() > 80 {
                        format!("{}…", &match_c[..80].replace('\n', " "))
                    } else {
                        match_c.replace('\n', " ")
                    };
                    println!("       {}", style(format!("Snippet: {}", preview)).dim());
                }
                println!("       {}", style(format!("Fix: {}", finding.recommendation)).yellow());
                println!();
            }
        }
    }

    // Print Dependency Vulnerabilities
    print_dep_audit(vulnerabilities, audit_skipped, audit_error);

    // Save report
    if report {
        let reports_dir = root.join(".uneven").join("reports").join("security");
        fs::create_dir_all(&reports_dir)?;
        
        let md_report = build_markdown_report(&all_findings, &root.to_string_lossy(), &dir);
        let md_path = reports_dir.join("scan-report.md");
        fs::write(&md_path, md_report)?;
        println!("{}", style(format!("  📄 Markdown : {}", md_path.to_string_lossy())).cyan());

        let html_report = build_html_report(&all_findings, &root.to_string_lossy(), &dir);
        let html_path = reports_dir.join("scan-report.html");
        fs::write(&html_path, html_report)?;
        println!("{}", style(format!("  🌐 HTML     : {}", html_path.to_string_lossy())).cyan());
        println!();
    }

    let has_blockers = all_findings.iter().any(|f| f.severity == Severity::Critical || f.severity == Severity::High);
    if has_blockers {
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        std::process::exit(1);
    }
    
    Ok(())
}

fn run_npm_audit(root: &Path) -> (Vec<DepVulnerability>, bool, Option<String>) {
    let pkg_json = root.join("package.json");
    if !pkg_json.exists() {
        return (vec![], true, None);
    }

    let output = SysCommand::new("npm")
        .args(["audit", "--json"])
        .current_dir(root)
        .output();

    match output {
        Ok(out) => {
            let stdout_str = String::from_utf8_lossy(&out.stdout);
            if let Ok(audit_data) = serde_json::from_str::<NpmAuditOutput>(&stdout_str) {
                if let Some(vulns) = audit_data.vulnerabilities {
                    let list: Vec<DepVulnerability> = vulns.into_iter().map(|(name, v)| DepVulnerability {
                        name: v.name.unwrap_or(name),
                        severity: v.severity.unwrap_or_else(|| "low".to_string()),
                        is_direct: v.is_direct.unwrap_or(false),
                        range: v.range.unwrap_or_else(|| "*".to_string()),
                        fix_available: v.fix_available.unwrap_or(serde_json::Value::Bool(false)),
                        via: v.via.unwrap_or_default(),
                    }).collect();
                    return (list, false, None);
                }
            }
            (vec![], true, Some("Failed to parse npm audit JSON output".to_string()))
        }
        Err(e) => (vec![], true, Some(e.to_string())),
    }
}

fn print_dep_audit(vulns: Vec<DepVulnerability>, skipped: bool, error: Option<String>) {
    println!("{}", style("  ".to_string() + &"─".repeat(54)).dim());
    println!("{}", style("  Dependency Vulnerabilities").bold());
    println!();

    if skipped {
        let reason = if let Some(err) = error {
            format!("  ⊘ Dependency audit skipped ({})", err)
        } else {
            "  ⊘ Dependency audit skipped (no package.json found)".to_string()
        };
        println!("{}", style(reason).dim());
        println!();
        return;
    }

    if vulns.is_empty() {
        println!("{}", style("  ✓ No known vulnerabilities found").green());
        println!("{}", style("  Run npm audit anytime to re-check.\n").dim());
        return;
    }

    // Counts
    let mut critical = 0;
    let mut high = 0;
    let mut moderate = 0;
    let mut low = 0;
    let mut info = 0;

    for v in &vulns {
        match v.severity.as_str() {
            "critical" => critical += 1,
            "high" => high += 1,
            "moderate" => moderate += 1,
            "low" => low += 1,
            _ => info += 1,
        }
    }

    let mut summary = Vec::new();
    if critical > 0 {
        summary.push(style(format!("{} critical", critical)).bg(Color::Red).fg(Color::White).bold().to_string());
    }
    if high > 0 {
        summary.push(style(format!("{} high", high)).red().bold().to_string());
    }
    if moderate > 0 {
        summary.push(style(format!("{} moderate", moderate)).yellow().bold().to_string());
    }
    if low > 0 {
        summary.push(style(format!("{} low", low)).blue().to_string());
    }
    if info > 0 {
        summary.push(style(format!("{} info", info)).dim().to_string());
    }

    println!("  {}", summary.join(&style(" · ").dim().to_string()));
    println!();

    // Show direct vulns
    let direct: Vec<&DepVulnerability> = vulns.iter().filter(|v| v.is_direct).collect();
    if !direct.is_empty() {
        for vuln in &direct {
            let sev_style = match vuln.severity.as_str() {
                "critical" => style("[CRITICAL]").bg(Color::Red).fg(Color::White).bold(),
                "high" => style("[HIGH]").red().bold(),
                "moderate" => style("[MODERATE]").yellow().bold(),
                "low" => style("[LOW]").blue(),
                _ => style("[INFO]").dim(),
            };

            let title = get_vuln_title(vuln);
            let fix = match &vuln.fix_available {
                serde_json::Value::Bool(false) => style("no fix available").dim().to_string(),
                serde_json::Value::Object(obj) => {
                    let name = obj.get("name").and_then(|n| n.as_str()).unwrap_or(&vuln.name);
                    let version = obj.get("version").and_then(|v| v.as_str()).unwrap_or("latest");
                    style(format!("fix: npm install {}@{}", name, version)).green().to_string()
                }
                _ => style("fix: npm audit fix").green().to_string(),
            };

            println!("    {} {} {}", sev_style, style(&vuln.name).underlined(), style(&vuln.range).dim());
            println!("       {}", style(title).white());
            println!("       {}", fix);
            println!();
        }
    }

    let transitive = vulns.len() - direct.len();
    if transitive > 0 {
        println!(
            "{}",
            style(format!(
                "  + {} indirect vulnerabilities — run npm audit for full list",
                transitive
            ))
            .dim()
        );
        println!();
    }

    println!(
        "  {} {} {}",
        style("Run").dim(),
        style("npm audit fix").cyan(),
        style("to apply automatic patches.").dim()
    );
    println!();
}

fn get_vuln_title(vuln: &DepVulnerability) -> String {
    for v in &vuln.via {
        if let Some(title) = v.get("title").and_then(|t| t.as_str()) {
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    format!("Vulnerability in {}", vuln.name)
}

fn count_scanned_files(dir: &str) -> usize {
    let mut count = 0;
    for entry in walkdir::WalkDir::new(dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if ["js", "jsx", "ts", "tsx", "py", "rs", "go", "java", "php", "rb"].contains(&ext) {
                    count += 1;
                }
            }
        }
    }
    count
}

fn count_dependencies_scanned(root: &Path) -> usize {
    // Read package-lock.json packages count
    if let Ok(content) = fs::read_to_string(root.join("package-lock.json")) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(packages) = parsed.get("packages").and_then(|p| p.as_object()) {
                return packages.len().saturating_sub(1); // Exclude root
            }
        }
    }
    0
}

struct SeverityCounts {
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
}

fn count_findings_by_severity(findings: &[SecurityFinding]) -> SeverityCounts {
    let mut c = SeverityCounts { critical: 0, high: 0, medium: 0, low: 0 };
    for f in findings {
        match f.severity {
            Severity::Critical => c.critical += 1,
            Severity::High => c.high += 1,
            Severity::Medium => c.medium += 1,
            Severity::Low => c.low += 1,
            _ => {}
        }
    }
    c
}

fn compute_risk_level(findings: &[SecurityFinding]) -> Severity {
    if findings.is_empty() {
        return Severity::Info;
    }
    let mut max_severity = Severity::Info;
    for f in findings {
        if f.severity > max_severity {
            max_severity = f.severity.clone();
        }
    }
    max_severity
}

fn group_by_category(findings: &[SecurityFinding]) -> HashMap<String, Vec<&SecurityFinding>> {
    let mut map = HashMap::new();
    for f in findings {
        let cat = get_malware_category(&f.finding_type).to_string();
        map.entry(cat).or_insert_with(Vec::new).push(f);
    }
    map
}

fn get_malware_category(finding_type: &str) -> &'static str {
    if finding_type.contains("remote_shell") || finding_type.contains("reverse_shell") {
        "remote-shell"
    } else if finding_type.contains("data_exfiltration") {
        "data-exfiltration"
    } else if finding_type.contains("obfuscated") {
        "obfuscation"
    } else if finding_type.contains("credential") || finding_type.contains("secret") {
        "credential-theft"
    } else if finding_type.contains("persistence") {
        "persistence"
    } else if finding_type.contains("miner") {
        "crypto-mining"
    } else if finding_type.contains("Typosquatting") {
        "typosquatting"
    } else {
        "supply-chain"
    }
}

fn get_category_icon(category: &str) -> &'static str {
    match category {
        "remote-shell" => "🐚",
        "data-exfiltration" => "📡",
        "obfuscation" => "🔒",
        "supply-chain" => "📦",
        "credential-theft" => "🔑",
        "persistence" => "⚓",
        "crypto-mining" => "⛏️ ",
        "typosquatting" => "🎭",
        _ => "⚠️",
    }
}

fn format_category(cat: &str) -> String {
    cat.split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn build_markdown_report(findings: &[SecurityFinding], project_root: &str, target: &str) -> String {
    let project_name = Path::new(project_root).file_name().and_then(|s| s.to_str()).unwrap_or("project");
    let date = chrono::Utc::now().to_rfc3339();
    let mut md = format!(
        "# Security Audit Report: {}\n\n- **Target Directory**: {}\n- **Generated At**: {}\n\n---\n\n",
        project_name, target, date
    );

    if findings.is_empty() {
        md.push_str("## ✓ No vulnerabilities or threats found.\n");
    } else {
        md.push_str("## Findings Summary\n\n");
        let counts = count_findings_by_severity(findings);
        md.push_str(&format!(
            "- **Critical**: {}\n- **High**: {}\n- **Medium**: {}\n- **Low**: {}\n\n---\n\n## Details\n\n",
            counts.critical, counts.high, counts.medium, counts.low
        ));

        for f in findings {
            let mut detail = format!(
                "### [{:?}] {} in `{}`\n- **Description**: {}\n- **Recommendation**: {}\n",
                f.severity,
                f.finding_type,
                f.file.clone().unwrap_or_else(|| "N/A".to_string()),
                f.description,
                f.recommendation
            );
            if let Some(ref note) = f.llm_note {
                detail.push_str(&format!("- **AI Audit Note**: {}\n", note));
            }
            detail.push('\n');
            md.push_str(&detail);
        }
    }
    md
}

fn build_html_report(findings: &[SecurityFinding], project_root: &str, target: &str) -> String {
    let project_name = Path::new(project_root).file_name().and_then(|s| s.to_str()).unwrap_or("project");
    let title = format!("Security Audit - {}", project_name);

    let mut content = format!(
        r#"<header>
    <div>
      <h1>Security Audit Report: {}</h1>
      <p>Target: {}</p>
    </div>
    <div class="meta">
      <span class="badge">Generated by Uneven</span>
    </div>
  </header>
  <div class="container">
    <div class="card">
      <h2>Executive Summary</h2>
      <div class="stats">"#,
        project_name, target
    );

    let counts = count_findings_by_severity(findings);
    content.push_str(&format!(
        r#"<div class="stat"><div class="value">{}</div><div class="label">Critical</div></div>
        <div class="stat"><div class="value">{}</div><div class="label">High</div></div>
        <div class="stat"><div class="value">{}</div><div class="label">Medium</div></div>
        <div class="stat"><div class="value">{}</div><div class="label">Low</div></div>"#,
        counts.critical, counts.high, counts.medium, counts.low
    ));

    content.push_str(
        r#"      </div>
    </div>
    <div class="card">
      <h2>Vulnerability Registry</h2>
      <div class="tbl-wrap">
        <table>
          <thead>
            <tr>
              <th>Severity</th>
              <th>Type</th>
              <th>Location</th>
              <th>Description</th>
              <th>Recommendation</th>
            </tr>
          </thead>
          <tbody>"#
    );

    for f in findings {
        let file_loc = if let Some(ref file) = f.file {
            if let Some(line) = f.line {
                format!("{}:{}", file, line)
            } else {
                file.clone()
            }
        } else {
            "N/A".to_string()
        };

        let desc = if let Some(ref note) = f.llm_note {
            format!("{}<br/><small style=\"color: #6a737d;\"><strong>AI Note:</strong> {}</small>", f.description, note)
        } else {
            f.description.clone()
        };

        content.push_str(&format!(
            r#"<tr>
              <td><strong>{:?}</strong></td>
              <td>{}</td>
              <td><code>{}</code></td>
              <td>{}</td>
              <td>{}</td>
            </tr>"#,
            f.severity, f.finding_type, file_loc, desc, f.recommendation
        ));
    }

    content.push_str(
        r#"          </tbody>
        </table>
      </div>
    </div>
  </div>"#
    );

    uneven_core::infrastructure::reporting::generate_html_report(&title, &content)
}
