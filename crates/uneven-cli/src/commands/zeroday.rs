use anyhow::Result;
use console::style;
use indicatif::ProgressBar;
use std::fs;
use std::path::Path;
use crate::commands::license;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::pentester::{
    self, Severity, SecurityFinding,
};
use uneven_core::infrastructure::pentester::zeroday::run_zeroday_scan;

pub async fn run(dir: Option<String>, report: Option<String>) -> Result<()> {
    // 1. License Check (Requires Team Plan)
    license::require_team()?;

    let root = loader::find_project_root(None);
    let config = loader::load_config(&root)?;

    // 2. Resolve directories to scan
    // CLI option takes precedence, then config.zeroday.dirs, then default to "./src"
    let dirs: Vec<String> = if let Some(d) = dir {
        vec![d]
    } else if let Some(ref zd) = config.zeroday {
        if let Some(ref d) = zd.dirs {
            d.clone()
        } else {
            vec!["./".to_string()]
        }
    } else {
        vec!["./".to_string()]
    };

    // 3. Resolve low cpu mode
    let low_cpu_mode = if let Some(ref zd) = config.zeroday {
        zd.low_cpu_mode.unwrap_or(false)
    } else {
        false
    };

    // 4. Run native Zero-Day Scan
    let pb = ProgressBar::new_spinner();
    pb.set_message("Running Zero-Day static analysis...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    // Convert Vec<String> to Vec<&str>
    let dir_refs: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();
    let findings = run_zeroday_scan(&dir_refs, low_cpu_mode).await?;

    pb.finish_with_message("Zero-Day static analysis completed");

    let mut all_findings = findings;

    // 5. LLM Review phase at the end
    // Use LLMSecurityReviewer if there are findings and brain config is available
    if !all_findings.is_empty() {
        let zeroday_brain = config.get_brain_for_command("zeroday");
        let reviewer = pentester::reviewer::LLMSecurityReviewer::new(&zeroday_brain);
        let _ = reviewer.review_findings(&mut all_findings).await;
    }

    // 6. Print findings summary and individual details to console
    let critical = all_findings.iter().filter(|f| f.severity == Severity::Critical).count();
    let high = all_findings.iter().filter(|f| f.severity == Severity::High).count();
    let medium = all_findings.iter().filter(|f| f.severity == Severity::Medium).count();
    let low = all_findings.iter().filter(|f| f.severity == Severity::Low).count();

    println!();
    println!("{}", style(format!("  Findings: {} total", all_findings.len())).bold());
    if critical > 0 {
        println!("{}", style(format!("    ● critical : {}", critical)).red().bold());
    }
    if high > 0 {
        println!("{}", style(format!("    ● high     : {}", high)).color256(208).bold());
    }
    if medium > 0 {
        println!("{}", style(format!("    ● medium   : {}", medium)).yellow().bold());
    }
    if low > 0 {
        println!("{}", style(format!("    ● low      : {}", low)).dim());
    }
    println!();

    if all_findings.is_empty() {
        println!("{}", style("  ✓ No custom zero-day vulnerabilities or high-risk flows detected.").green());
    } else {
        for f in &all_findings {
            let loc = if let Some(ref file) = f.file {
                if let Some(line) = f.line {
                    format!("{}:{}", file, line)
                } else {
                    file.clone()
                }
            } else {
                "N/A".to_string()
            };
            
            let sev_style = match f.severity {
                Severity::Critical => style(format!("{:?}", f.severity)).red().bold(),
                Severity::High => style(format!("{:?}", f.severity)).color256(208).bold(),
                Severity::Medium => style(format!("{:?}", f.severity)).yellow().bold(),
                Severity::Low | Severity::Info => style(format!("{:?}", f.severity)).dim(),
            };

            println!(
                "  [{}] {} in {}\n    Description: {}\n    Recommendation: {}",
                sev_style,
                style(&f.finding_type).bold(),
                style(loc).cyan(),
                f.description,
                f.recommendation
            );
            if let Some(ref note) = f.llm_note {
                println!("    {} {}", style("AI Review Note:").bold().blue(), note);
            }
            println!();
        }
    }

    // 7. Generate report (Markdown/HTML) if requested
    if let Some(ref fmt) = report {
        let reports_dir = root.join(".uneven").join("reports").join("security");
        fs::create_dir_all(&reports_dir)?;

        let (gen_md, gen_html) = match fmt.as_str() {
            "md" => (true, false),
            "html" => (false, true),
            _ => (true, true),
        };

        if gen_md {
            let md_report = build_markdown_report(&all_findings, &root.to_string_lossy(), &dirs.join(", "));
            let md_path = reports_dir.join("zeroday-report.md");
            fs::write(&md_path, md_report)?;
            println!("{}", style(format!("  📄 Markdown report: {}", md_path.to_string_lossy())).cyan());
        }

        if gen_html {
            let html_report = build_html_report(&all_findings, &root.to_string_lossy(), &dirs.join(", "));
            let html_path = reports_dir.join("zeroday-report.html");
            fs::write(&html_path, html_report)?;
            println!("{}", style(format!("  🌐 HTML report:     {}", html_path.to_string_lossy())).cyan());
        }
    } else {
        println!("{}", style("  Tip: run with -r or --report <format> (md|html|both) to generate a detailed report").dim());
    }

    Ok(())
}

fn build_markdown_report(findings: &[SecurityFinding], project_root: &str, target: &str) -> String {
    let project_name = Path::new(project_root).file_name().and_then(|s| s.to_str()).unwrap_or("project");
    let date = chrono::Utc::now().to_rfc3339();
    let mut md = format!(
        "# Zero-Day Vulnerability Scan Report: {}\n\n- **Scanned Directories**: {}\n- **Generated At**: {}\n\n---\n\n",
        project_name, target, date
    );

    if findings.is_empty() {
        md.push_str("## ✓ No vulnerabilities or threats found.\n");
    } else {
        md.push_str("## Findings Summary\n\n");
        let critical = findings.iter().filter(|f| f.severity == Severity::Critical).count();
        let high = findings.iter().filter(|f| f.severity == Severity::High).count();
        let medium = findings.iter().filter(|f| f.severity == Severity::Medium).count();
        let low = findings.iter().filter(|f| f.severity == Severity::Low).count();
        md.push_str(&format!(
            "- **Critical**: {}\n- **High**: {}\n- **Medium**: {}\n- **Low**: {}\n\n---\n\n## Details\n\n",
            critical, high, medium, low
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
    let title = format!("Zero-Day Security Audit - {}", project_name);

    let mut content = format!(
        r#"<header>
    <div>
      <h1>Zero-Day Vulnerability Scan Report: {}</h1>
      <p>Scanned Directories: {}</p>
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

    let critical = findings.iter().filter(|f| f.severity == Severity::Critical).count();
    let high = findings.iter().filter(|f| f.severity == Severity::High).count();
    let medium = findings.iter().filter(|f| f.severity == Severity::Medium).count();
    let low = findings.iter().filter(|f| f.severity == Severity::Low).count();
    content.push_str(&format!(
        r#"<div class="stat"><div class="value">{}</div><div class="label">Critical</div></div>
        <div class="stat"><div class="value">{}</div><div class="label">High</div></div>
        <div class="stat"><div class="value">{}</div><div class="label">Medium</div></div>
        <div class="stat"><div class="value">{}</div><div class="label">Low</div></div>"#,
        critical, high, medium, low
    ));

    content.push_str(
        r#"      </div>
    </div>
    <div class="card">
      <h2>Security Findings Registry</h2>
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
