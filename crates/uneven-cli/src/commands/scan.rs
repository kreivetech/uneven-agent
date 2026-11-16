use anyhow::Result;
use console::{style, Color};
use indicatif::ProgressBar;
use serde::Serialize;
use std::fs;
use std::path::Path;

use uneven_core::application::{DepVulnerability, ScanResult, ScanService};
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::pentester::Severity;

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

    let pb_clone = pb.clone();
    let on_progress = move |msg: &str| {
        if let Some(ref bar) = pb_clone {
            bar.set_message(msg.to_string());
        }
    };

    // Execute decoupled scan service from uneven-core
    let scan_result = ScanService::run_scan(&root, &dir, &config, Some(on_progress)).await?;

    if let Some(ref bar) = pb {
        bar.finish_and_clear();
    }

    if json_output {
        output_json(&scan_result)?;
        return Ok(());
    }

    print_terminal_results(&scan_result, &dir);

    // Save report if requested
    if report {
        save_reports(&scan_result, &root, &dir)?;
    }

    let has_blockers = scan_result
        .findings
        .iter()
        .any(|f| f.severity == Severity::Critical || f.severity == Severity::High);

    if has_blockers {
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        std::process::exit(1);
    }

    Ok(())
}

fn output_json(scan_result: &ScanResult) -> Result<()> {
    let findings_json: Vec<SecurityFindingJson> = scan_result
        .findings
        .iter()
        .map(|f| SecurityFindingJson {
            severity: format!("{:?}", f.severity).to_lowercase(),
            finding_type: f.finding_type.clone(),
            file: f.file.clone(),
            line: f.line,
            description: f.description.clone(),
            recommendation: f.recommendation.clone(),
            cvss: f.cvss,
            llm_verdict: f.llm_verdict.clone(),
            llm_note: f.llm_note.clone(),
        })
        .collect();

    let result = ScanResultJson {
        findings: findings_json,
        files_scanned: scan_result.files_scanned,
        dependencies_scanned: scan_result.dependencies_scanned,
        duration_ms: scan_result.duration_ms,
        risk_level: format!("{:?}", scan_result.risk_level).to_lowercase(),
    };

    println!("{}", serde_json::to_string_pretty(&result)?);
    {
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }

    let has_blockers = scan_result
        .findings
        .iter()
        .any(|f| f.severity == Severity::Critical || f.severity == Severity::High);

    if has_blockers {
        std::process::exit(1);
    } else {
        std::process::exit(0);
    }
}

fn print_terminal_results(scan_result: &ScanResult, dir: &str) {
    println!();
    println!("{}", style("  🛡️ Uneven Snatchy Security & Malware Scanner").bold());
    println!("{}", style("  ".to_string() + &"─".repeat(54)).dim());
    println!(
        "{}",
        style(format!(
            "  Scanned : {} source files, {} dependencies (in {})",
            scan_result.files_scanned, scan_result.dependencies_scanned, dir
        ))
        .dim()
    );
    println!("{}", style(format!("  Duration: {}ms", scan_result.duration_ms)).dim());
    println!();

    if scan_result.findings.is_empty() {
        println!("{}", style("  ✓ No malicious patterns or security flaws detected").green());
        println!("{}", style("  Your project workspace looks clean.\n").dim());
    } else {
        let risk_label = format!("{:?}", scan_result.risk_level).to_uppercase();
        let risk_style = match scan_result.risk_level {
            Severity::Critical => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).bg(Color::Red).fg(Color::White).bold(),
            Severity::High => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).red().bold(),
            Severity::Medium => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).yellow().bold(),
            Severity::Low => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).blue(),
            _ => style(format!("  ⚠  RISK LEVEL: {}", risk_label)).dim(),
        };
        println!("{}", risk_style);
        println!();

        let mut summary_parts = Vec::new();
        if scan_result.summary.critical > 0 {
            summary_parts.push(style(format!("{} critical", scan_result.summary.critical)).bg(Color::Red).fg(Color::White).bold().to_string());
        }
        if scan_result.summary.high > 0 {
            summary_parts.push(style(format!("{} high", scan_result.summary.high)).red().bold().to_string());
        }
        if scan_result.summary.medium > 0 {
            summary_parts.push(style(format!("{} medium", scan_result.summary.medium)).yellow().bold().to_string());
        }
        if scan_result.summary.low > 0 {
            summary_parts.push(style(format!("{} low", scan_result.summary.low)).blue().to_string());
        }
        println!("  Findings: {}", summary_parts.join(&style(" · ").dim().to_string()));
        println!();

        for group in &scan_result.grouped_findings {
            println!("{}", style(format!("  {}  {} ({})", group.icon, group.title, group.findings.len())).bold());

            for finding in &group.findings {
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
                        format!("{}…", match_c[..80].replace('\n', " "))
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

    print_dep_audit(
        &scan_result.vulnerabilities,
        scan_result.audit_skipped,
        scan_result.audit_error.as_deref(),
    );
}

fn print_dep_audit(vulns: &[DepVulnerability], skipped: bool, error: Option<&str>) {
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

    let mut critical = 0;
    let mut high = 0;
    let mut moderate = 0;
    let mut low = 0;
    let mut info = 0;

    for v in vulns {
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
        if let Some(title) = v.get("title").and_then(|t| t.as_str())
            && !title.is_empty()
        {
            return title.to_string();
        }
    }
    format!("Vulnerability in {}", vuln.name)
}

fn save_reports(scan_result: &ScanResult, root: &Path, dir: &str) -> Result<()> {
    let reports_dir = root.join(".uneven").join("reports").join("security");
    fs::create_dir_all(&reports_dir)?;

    let md_report = scan_result.to_markdown_report(root, dir);
    let md_path = reports_dir.join("scan-report.md");
    fs::write(&md_path, md_report)?;
    println!("{}", style(format!("  📄 Markdown : {}", md_path.to_string_lossy())).cyan());

    let html_report = scan_result.to_html_report(root, dir);
    let html_path = reports_dir.join("scan-report.html");
    fs::write(&html_path, html_report)?;
    println!("{}", style(format!("  🌐 HTML     : {}", html_path.to_string_lossy())).cyan());
    println!();
    Ok(())
}
