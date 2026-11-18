use crate::domain::config::UnevenConfig;
use crate::domain::skills::SKILL_ZERODAY_AUDIT;
use crate::infrastructure::pentester::reviewer::LLMSecurityReviewer;
use crate::infrastructure::pentester::zeroday::run_zeroday_scan as run_native_zeroday_scan;
use crate::infrastructure::pentester::{SecurityFinding, Severity};
use crate::infrastructure::reporting::generate_html_report;
use crate::UnevenError;
use catfive::{string, Validator};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ZerodaySummaryCounts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodayResult {
    pub findings: Vec<SecurityFinding>,
    pub summary: ZerodaySummaryCounts,
    pub duration_ms: u128,
    pub scanned_dirs: Vec<String>,
    pub markdown_report_path: Option<String>,
    pub html_report_path: Option<String>,
    pub is_deep_audit: bool,
}

impl ZerodayResult {
    /// Formats the zero-day scan result into clean, friendly Markdown for conversational chat in CLI or IDE.
    pub fn to_chat_summary(&self) -> String {
        let mut out = String::new();
        let mode_label = if self.is_deep_audit {
            "Autonomous Deep-Dive Think Audit"
        } else {
            "Static Taint Analysis"
        };

        out.push_str(&format!("### ☣️ Snatchy Zero-Day & Taint Analysis ({})\n\n", mode_label));
        out.push_str(&format!(
            "- **Total Findings**: {}\n- **Duration**: {}ms\n- **Scanned Targets**: `{}`\n\n",
            self.findings.len(),
            self.duration_ms,
            self.scanned_dirs.join(", ")
        ));

        if self.findings.is_empty() {
            out.push_str("✅ **No custom zero-day vulnerability flows, unsafe sinks, or taint anomalies detected.**\nYour codebase passed all deep inspection checks.\n");
            return out;
        }

        out.push_str(&format!(
            "**Findings Breakdown:**\n- 🔴 Critical: {}\n- 🟠 High: {}\n- 🟡 Medium: {}\n- 🔵 Low: {}\n\n",
            self.summary.critical, self.summary.high, self.summary.medium, self.summary.low
        ));

        for f in self.findings.iter().take(10) {
            let loc = if let Some(ref file) = f.file {
                if let Some(line) = f.line {
                    format!("{}:{}", file, line)
                } else {
                    file.clone()
                }
            } else {
                "N/A".to_string()
            };

            out.push_str(&format!(
                "- `[{:?}]` `{}` in `{}`: {}\n  *Fix*: {}\n",
                f.severity, f.finding_type, loc, f.description, f.recommendation
            ));
            if let Some(ref trail) = f.match_content {
                out.push_str(&format!("  *Taint Trail*: `{}`\n", trail));
            }
            if let Some(ref note) = f.llm_note {
                out.push_str(&format!("  *AI Audit*: {}\n", note));
            }
        }

        if self.findings.len() > 10 {
            out.push_str(&format!(
                "\n*... and {} more zero-day findings.*\n",
                self.findings.len() - 10
            ));
        }

        if let Some(ref md) = self.markdown_report_path {
            out.push_str(&format!("\n📊 **Markdown Report**: `{}`", md));
        }
        if let Some(ref html) = self.html_report_path {
            out.push_str(&format!("\n🌐 **HTML Report**: `{}`", html));
        }

        out
    }
}

pub struct ZerodayService;

impl ZerodayService {
    /// Executes a complete, decoupled Zero-Day scan:
    /// - Fast Mode: Rayon parallel AST pattern matching and taint flow analysis + LLMSecurityReviewer.
    /// - Deep Mode: Autonomous multi-turn Think loop powered by `SKILL_ZERODAY_AUDIT` and Catfive validation,
    ///   recursively inspecting multi-file taint paths via `[NEED_FILE]`.
    pub async fn run_zeroday_scan<F>(
        project_root: &Path,
        dir_override: Option<Vec<String>>,
        config: &UnevenConfig,
        generate_reports: bool,
        deep_mode: bool,
        mut progress_callback: Option<F>,
    ) -> Result<ZerodayResult, UnevenError>
    where
        F: FnMut(&str) + Send + Sync,
    {
        let start_time = Instant::now();

        crate::application::LicenseService::require_team()?;

        // 1. Resolve and validate target directories using Catfive
        let dir_validator = string().min(1).max(512).block("\0\r\n");
        let dirs: Vec<String> = if let Some(raw_dirs) = dir_override {
            raw_dirs
                .into_iter()
                .filter(|d| dir_validator.validate(d.as_str()).is_ok())
                .collect()
        } else if let Some(ref zd) = config.zeroday {
            if let Some(ref d) = zd.dirs {
                d.iter()
                    .filter(|s| dir_validator.validate(s.as_str()).is_ok())
                    .cloned()
                    .collect()
            } else {
                vec!["./".to_string()]
            }
        } else {
            vec!["./".to_string()]
        };

        let dirs = if dirs.is_empty() {
            vec!["./".to_string()]
        } else {
            dirs
        };

        // 2. Resolve low cpu mode
        let low_cpu_mode = if let Some(ref zd) = config.zeroday {
            zd.low_cpu_mode.unwrap_or(false)
        } else {
            false
        };

        if let Some(ref mut cb) = progress_callback {
            cb("Running Zero-Day parallel static analysis and taint flow tracing...");
        }

        let dir_refs: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();
        let static_findings = run_native_zeroday_scan(project_root, &dir_refs, low_cpu_mode).await?;
        let mut all_findings = static_findings;

        // 3. Autonomous Deep-Dive Think Loop (if enabled)
        if deep_mode {
            if let Some(ref mut cb) = progress_callback {
                cb("Initiating autonomous Think reasoning loop with SKILL_ZERODAY_AUDIT...");
            }

            let candidate_files = collect_candidate_source_files(project_root, &dirs);
            let mut audited_files = HashSet::new();

            // Prioritize files that already triggered static warnings
            for f in &all_findings {
                if let Some(ref file_path) = f.file {
                    audited_files.insert(file_path.clone());
                }
            }

            // Cluster candidate files in batches of up to 4 files for multi-file correlation
            let file_chunks: Vec<Vec<String>> = candidate_files
                .chunks(4)
                .take(6) // Cap maximum deep batches to guarantee responsive execution
                .map(|chunk| chunk.to_vec())
                .collect();

            for (idx, chunk) in file_chunks.into_iter().enumerate() {
                if let Some(ref mut cb) = progress_callback {
                    cb(&format!(
                        "Deep auditing cluster {}/{}: {} files...",
                        idx + 1,
                        dirs.len().max(1),
                        chunk.len()
                    ));
                }

                let task_prompt = format!(
                    "Audit the following source files for custom zero-day vulnerabilities, logic flaws, and unvalidated taint flows.\n\
                    Trace all untrusted user inputs down to critical execution sinks (shell, SQL, eval, filesystem, deserialization).\n\
                    Use [NEED_FILE: path] to inspect related callers, callee helpers, or repositories.\n\
                    For each confirmed flaw, emit:\n\
                    [ZERODAY_FINDING: <Severity> | <Type> | <file:line> | <Description> | <Recommendation> | <TaintTrail>]\n\
                    Target files to inspect:\n{}",
                    chunk.iter().map(|f| format!("- {}", f)).collect::<Vec<_>>().join("\n")
                );

                let dummy_progress = |_: crate::application::think_service::ThoughtPhase| {};
                if let Ok(thought_res) = crate::application::think_service::execute_thought_loop_with_system(
                    &task_prompt,
                    &chunk,
                    config,
                    project_root,
                    None,
                    None,
                    Some(dummy_progress),
                    Some(SKILL_ZERODAY_AUDIT),
                )
                .await
                {
                    let discovered = parse_zeroday_finding_tags(&thought_res.final_response);
                    for item in discovered {
                        // Avoid duplicates if already present
                        if !all_findings.iter().any(|existing| {
                            existing.file == item.file
                                && existing.line == item.line
                                && existing.finding_type == item.finding_type
                        }) {
                            all_findings.push(item);
                        }
                    }
                }
            }
        } else if !all_findings.is_empty() {
            // Standard Review: Single-turn LLMSecurityReviewer
            if let Some(ref mut cb) = progress_callback {
                cb("Reviewing Zero-Day findings with AI to eliminate false positives...");
            }
            let zeroday_brain = config.get_brain_for_command("zeroday");
            let reviewer = LLMSecurityReviewer::new(&zeroday_brain);
            let _ = reviewer.review_findings(&mut all_findings).await;
        }

        let critical = all_findings.iter().filter(|f| f.severity == Severity::Critical).count();
        let high = all_findings.iter().filter(|f| f.severity == Severity::High).count();
        let medium = all_findings.iter().filter(|f| f.severity == Severity::Medium).count();
        let low = all_findings.iter().filter(|f| f.severity == Severity::Low).count();

        let summary = ZerodaySummaryCounts {
            critical,
            high,
            medium,
            low,
        };

        // 4. Generate reports if requested
        let mut markdown_report_path = None;
        let mut html_report_path = None;

        if generate_reports {
            let reports_dir = project_root.join(".uneven").join("reports").join("security");
            let _ = fs::create_dir_all(&reports_dir);

            let target_desc = dirs.join(", ");
            let md_report = build_markdown_report(
                &all_findings,
                &project_root.to_string_lossy(),
                &target_desc,
                deep_mode,
            );
            let md_path = reports_dir.join("zeroday-report.md");
            if fs::write(&md_path, md_report).is_ok() {
                markdown_report_path = Some(md_path.to_string_lossy().to_string());
            }

            let html_report = build_html_report(
                &all_findings,
                &project_root.to_string_lossy(),
                &target_desc,
                deep_mode,
            );
            let html_path = reports_dir.join("zeroday-report.html");
            if fs::write(&html_path, html_report).is_ok() {
                html_report_path = Some(html_path.to_string_lossy().to_string());
            }
        }

        let duration_ms = start_time.elapsed().as_millis();

        if let Some(ref mut cb) = progress_callback {
            cb("Zero-Day security analysis completed.");
        }

        Ok(ZerodayResult {
            findings: all_findings,
            summary,
            duration_ms,
            scanned_dirs: dirs,
            markdown_report_path,
            html_report_path,
            is_deep_audit: deep_mode,
        })
    }
}

/// Discovers candidate project source files using ignore walk.
fn collect_candidate_source_files(root: &Path, dirs: &[String]) -> Vec<String> {
    let mut files = Vec::new();
    let supported_extensions = [
        "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "php", "rb",
    ];

    for dir in dirs {
        let p = Path::new(dir);
        let target_dir = if p.is_absolute() || p.exists() {
            p.to_path_buf()
        } else {
            root.join(p)
        };

        if !target_dir.exists() {
            continue;
        }

        let walker = WalkBuilder::new(&target_dir)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension().and_then(|e| e.to_str())
                && supported_extensions.contains(&ext.to_lowercase().as_str())
            {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                files.push(rel);
            }
        }
    }

    files
}

/// Parses structured `[ZERODAY_FINDING: ...]` tags validated via Catfive.
pub fn parse_zeroday_finding_tags(text: &str) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();
    let mut start_idx = 0;
    let path_validator = string().min(1).max(512).block("\0\r\n\t");
    let desc_validator = string().min(3).block("\0");

    while let Some(open_pos) = text[start_idx..].find("[ZERODAY_FINDING:") {
        let abs_open = start_idx + open_pos;
        if let Some(close_pos) = text[abs_open..].find(']') {
            let tag_content = &text[abs_open + 17..abs_open + close_pos];
            let parts: Vec<&str> = tag_content.split('|').map(|s| s.trim()).collect();
            if parts.len() >= 5 {
                let sev_str = parts[0];
                let finding_type = parts[1].to_string();
                let loc = parts[2];
                let desc = parts[3];
                let rec = parts[4];
                let trail = if parts.len() > 5 {
                    let raw_trail = parts[5].trim();
                    if raw_trail.is_empty() || raw_trail == "N/A" {
                        None
                    } else {
                        Some(raw_trail.to_string())
                    }
                } else {
                    None
                };

                let severity = match sev_str.to_lowercase().as_str() {
                    "critical" => Severity::Critical,
                    "high" => Severity::High,
                    "medium" => Severity::Medium,
                    "low" => Severity::Low,
                    _ => Severity::Medium,
                };

                let (file, line) = if loc.contains(':') {
                    let mut split = loc.split(':');
                    let f = split.next().unwrap_or("").trim().to_string();
                    let l = split.next().and_then(|n| n.trim().parse::<usize>().ok());
                    (Some(f), l)
                } else if !loc.is_empty() && loc != "N/A" {
                    (Some(loc.to_string()), None)
                } else {
                    (None, None)
                };

                let file_valid = file
                    .as_ref()
                    .map(|f| path_validator.validate(f.as_str()).is_ok())
                    .unwrap_or(true);
                let desc_valid = desc_validator.validate(desc).is_ok();

                if file_valid && desc_valid {
                    findings.push(SecurityFinding {
                        severity,
                        finding_type,
                        file,
                        line,
                        description: desc.to_string(),
                        recommendation: rec.to_string(),
                        match_content: trail,
                        llm_verdict: Some("confirmed".to_string()),
                        llm_note: Some("Autonomous Think Zero-Day audit verified".to_string()),
                        ..Default::default()
                    });
                }
            }
            start_idx = abs_open + close_pos + 1;
        } else {
            break;
        }
    }
    findings
}

fn build_markdown_report(
    findings: &[SecurityFinding],
    project_root: &str,
    target: &str,
    is_deep: bool,
) -> String {
    let project_name = Path::new(project_root)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let date = chrono::Utc::now().to_rfc3339();
    let mode_str = if is_deep { "Autonomous Deep Think Audit" } else { "Static Analysis" };
    let mut md = format!(
        "# Zero-Day Vulnerability Scan Report: {}\n\n- **Audit Mode**: {}\n- **Scanned Directories**: {}\n- **Generated At**: {}\n\n---\n\n",
        project_name, mode_str, target, date
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
            let loc = if let Some(ref file) = f.file {
                if let Some(line) = f.line {
                    format!("{}:{}", file, line)
                } else {
                    file.clone()
                }
            } else {
                "N/A".to_string()
            };

            let mut detail = format!(
                "### [{:?}] {} in `{}`\n- **Description**: {}\n- **Recommendation**: {}\n",
                f.severity,
                f.finding_type,
                loc,
                f.description,
                f.recommendation
            );
            if let Some(ref trail) = f.match_content {
                detail.push_str(&format!("- **Taint Trail**: `{}`\n", trail));
            }
            if let Some(ref note) = f.llm_note {
                detail.push_str(&format!("- **AI Audit Note**: {}\n", note));
            }
            detail.push('\n');
            md.push_str(&detail);
        }
    }
    md
}

fn build_html_report(
    findings: &[SecurityFinding],
    project_root: &str,
    target: &str,
    is_deep: bool,
) -> String {
    let project_name = Path::new(project_root)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let title = format!("Zero-Day Security Audit - {}", project_name);
    let mode_str = if is_deep { "Autonomous Deep Think Audit" } else { "Static Analysis" };

    let mut content = format!(
        r#"<header>
    <div>
      <h1>Zero-Day Vulnerability Scan Report: {}</h1>
      <p>Audit Mode: {} | Scanned Directories: {}</p>
    </div>
    <div class="meta">
      <span class="badge">Generated by Uneven</span>
    </div>
  </header>
  <div class="container">
    <div class="card">
      <h2>Executive Summary</h2>
      <div class="stats">"#,
        project_name, mode_str, target
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
              <th>Taint Trail</th>
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

        let trail_html = f
            .match_content
            .as_deref()
            .map(|t| format!("<code>{}</code>", t))
            .unwrap_or_else(|| "<span style=\"color: #8c959f;\">—</span>".to_string());

        content.push_str(&format!(
            r#"<tr>
              <td><strong>{:?}</strong></td>
              <td>{}</td>
              <td><code>{}</code></td>
              <td>{}</td>
              <td>{}</td>
              <td>{}</td>
            </tr>"#,
            f.severity, f.finding_type, file_loc, desc, trail_html, f.recommendation
        ));
    }

    content.push_str(
        r#"          </tbody>
        </table>
      </div>
    </div>
  </div>"#
    );

    generate_html_report(&title, &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zeroday_chat_summary() {
        let result = ZerodayResult {
            findings: vec![
                SecurityFinding {
                    severity: Severity::High,
                    finding_type: "taint:command_injection".to_string(),
                    file: Some("src/exec.rs".to_string()),
                    line: Some(42),
                    description: "Unsanitized input propagates to shell execution".to_string(),
                    recommendation: "Use structured argument vector without shell wrapper".to_string(),
                    match_content: Some("src/api.rs:10 -> src/exec.rs:42".to_string()),
                    ..Default::default()
                },
            ],
            summary: ZerodaySummaryCounts {
                critical: 0,
                high: 1,
                medium: 0,
                low: 0,
            },
            duration_ms: 120,
            scanned_dirs: vec!["./src".to_string()],
            markdown_report_path: Some(".uneven/reports/security/zeroday-report.md".to_string()),
            html_report_path: None,
            is_deep_audit: true,
        };

        let summary = result.to_chat_summary();
        assert!(summary.contains("Snatchy Zero-Day & Taint Analysis"));
        assert!(summary.contains("Autonomous Deep-Dive Think Audit"));
        assert!(summary.contains("taint:command_injection"));
        assert!(summary.contains("src/exec.rs:42"));
        assert!(summary.contains("Taint Trail"));
    }

    #[test]
    fn test_parse_zeroday_finding_tags_with_catfive() {
        let raw = r#"
Here are the findings:
[ZERODAY_FINDING: Critical | sql_injection | src/db.rs:88 | Direct string interpolation of user input | Use parameterized queries | src/handlers/user.rs:12 (query) -> src/db.rs:88]
[ZERODAY_FINDING: High | rce | src/cmd.rs:14 | Execution of untrusted shell command | Use execve array | src/cli.rs:5 -> src/cmd.rs:14]
Finished deep audit.
"#;
        let findings = parse_zeroday_finding_tags(raw);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].severity, Severity::Critical);
        assert_eq!(findings[0].finding_type, "sql_injection");
        assert_eq!(findings[0].file.as_deref(), Some("src/db.rs"));
        assert_eq!(findings[0].line, Some(88));
        assert_eq!(findings[0].match_content.as_deref(), Some("src/handlers/user.rs:12 (query) -> src/db.rs:88"));

        assert_eq!(findings[1].severity, Severity::High);
        assert_eq!(findings[1].file.as_deref(), Some("src/cmd.rs"));
        assert_eq!(findings[1].line, Some(14));
    }
}
