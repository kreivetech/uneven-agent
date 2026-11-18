use anyhow::Result;
use console::style;
use indicatif::ProgressBar;
use uneven_core::application::{ZerodayResult, ZerodayService};
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::pentester::Severity;

pub async fn run(dir: Option<String>, report: Option<String>, deep: bool) -> Result<()> {
    let root = loader::find_project_root(None);
    let config = loader::load_config(&root)?;

    // 2. Setup Progress Spinner
    let pb = ProgressBar::new_spinner();
    let initial_msg = if deep {
        "Starting autonomous Think Zero-Day audit and multi-file taint tracing..."
    } else {
        "Running Zero-Day static analysis and taint tracking..."
    };
    pb.set_message(initial_msg);
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let pb_clone = pb.clone();
    let on_progress = move |msg: &str| {
        pb_clone.set_message(msg.to_string());
    };

    let dir_override = dir.map(|d| vec![d]);
    let generate_reports = report.is_some();

    // 3. Execute decoupled ZerodayService
    let result: ZerodayResult = ZerodayService::run_zeroday_scan(
        &root,
        dir_override,
        &config,
        generate_reports,
        deep,
        Some(on_progress),
    )
    .await?;

    pb.finish_and_clear();

    // 4. Print summary to console
    println!();
    println!("{}", style(format!("  Findings: {} total ({}ms)", result.findings.len(), result.duration_ms)).bold());
    if result.summary.critical > 0 {
        println!("{}", style(format!("    ● critical : {}", result.summary.critical)).red().bold());
    }
    if result.summary.high > 0 {
        println!("{}", style(format!("    ● high     : {}", result.summary.high)).color256(208).bold());
    }
    if result.summary.medium > 0 {
        println!("{}", style(format!("    ● medium   : {}", result.summary.medium)).yellow().bold());
    }
    if result.summary.low > 0 {
        println!("{}", style(format!("    ● low      : {}", result.summary.low)).dim());
    }
    println!();

    if result.findings.is_empty() {
        println!("{}", style("  ✓ No custom zero-day vulnerabilities or high-risk flows detected.").green());
    } else {
        for f in &result.findings {
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
            if let Some(ref trail) = f.match_content {
                println!("    {} {}", style("Taint Trail:").bold().yellow(), style(trail).cyan());
            }
            if let Some(ref note) = f.llm_note {
                println!("    {} {}", style("AI Review Note:").bold().blue(), note);
            }
            println!();
        }
    }

    if let Some(ref md) = result.markdown_report_path {
        println!("{}", style(format!("  📄 Markdown report: {}", md)).cyan());
    }
    if let Some(ref html) = result.html_report_path {
        println!("{}", style(format!("  🌐 HTML report:     {}", html)).cyan());
    }

    if report.is_none() {
        println!("{}", style("  Tip: run with -r or --report <format> (md|html|both) to generate a detailed report").dim());
    }

    Ok(())
}
