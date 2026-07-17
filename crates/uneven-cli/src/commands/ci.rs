use anyhow::{anyhow, Result};
use std::fs;
use std::time::Instant;
use std::path::Path;
use serde::{Serialize, Deserialize};

use crate::commands::license;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::utils::process_lock::ProcessLock;
use uneven_core::infrastructure::pentester;
use uneven_core::infrastructure::utils::test_runner::TestRunner;
use uneven_core::domain::testing::{TestRunnerConfig, TestFramework};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StepResult {
    pub name: String,
    pub passed: bool,
    pub skipped: bool,
    pub duration: u128,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CiSummary {
    pub passed: bool,
    pub steps: Vec<StepResult>,
    pub timestamp: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: u128,
}

pub async fn run(
    dir: String,
    skip_security: bool,
    skip_tests: bool,
    strict_security: bool,
    output: Option<String>,
    github: bool,
) -> Result<()> {
    license::require_pro()?;
    let root = loader::find_project_root(None);
    let mut lock = ProcessLock::new(&root);
    if let Some(existing) = lock.acquire("ci")? {
        return Err(anyhow!("Cannot run CI. Another command '{}' has active lock (PID {}).", existing.command, existing.pid));
    }

    let started_at = Instant::now();
    let project_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("project");
    let scan_dir = if dir.is_empty() { "./".to_string() } else { dir };
    let uneven_dir = root.join(".uneven");
    fs::create_dir_all(&uneven_dir)?;

    let mut steps = vec![];
    let mut overall_passed = true;

    println!("\n{}", "=".repeat(60));
    println!("  Uneven CI — {}", project_name);
    println!("============================================================");
    println!("  Timestamp : {}", chrono::Utc::now().to_rfc3339());
    println!("  Directory : {}", root.display());
    println!("  Scan dir  : {}", scan_dir);
    println!();

    // 1. TypeScript Typecheck
    {
        let t0 = Instant::now();
        let name = "TypeScript typecheck".to_string();
        let mut passed = true;
        let mut skipped = false;
        let detail: String;

        let has_tsconfig = root.join("tsconfig.json").exists();
        if !has_tsconfig {
            skipped = true;
            detail = "no tsconfig.json found — skipped".to_string();
        } else {
            let output = std::process::Command::new("npx")
                .args(["tsc", "--noEmit"])
                .current_dir(&root)
                .output();

            match output {
                Ok(out) => {
                    if !out.status.success() {
                        passed = false;
                        overall_passed = false;
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let combined = format!("{}\n{}", stdout, stderr);
                        let error_count = combined.matches("error TS").count();
                        detail = format!("{} type error(s) found", error_count);
                        println!("\n--- TypeScript output ---");
                        println!("{}", &combined[..combined.len().min(2000)]);
                        println!("---");
                    } else {
                        detail = "no type errors".to_string();
                    }
                }
                Err(e) => {
                    passed = false;
                    overall_passed = false;
                    detail = format!("failed to run typecheck: {}", e);
                }
            }
        }

        let duration = t0.elapsed().as_millis();
        steps.push(StepResult {
            name: name.clone(),
            passed,
            skipped,
            duration,
            detail: detail.clone(),
        });

        let icon = if skipped { "⊙" } else if passed { "✓" } else { "✗" };
        println!("{}  {}: {}  ({}ms)", icon, name, detail, duration);

        if !passed {
            println!("\n  [FAIL-FAST] Aborting pipeline due to type errors.");
            finish_ci(false, steps, started_at, &uneven_dir, output, project_name, github, lock).await?;
            return Ok(());
        }
    }

    // 2. Security Scan
    {
        let t0 = Instant::now();
        let name = "Security scan".to_string();
        let mut passed = true;
        let mut skipped = false;
        let detail: String;

        if skip_security {
            skipped = true;
            detail = "skipped via --skip-security".to_string();
            steps.push(StepResult {
                name: name.clone(),
                passed: true,
                skipped,
                duration: 0,
                detail: detail.clone(),
            });
            println!("⊙  {}: skipped", name);
        } else {
            println!("Running security scan on {}...", scan_dir);
            let static_findings = pentester::static_analysis(&[&scan_dir]).await.unwrap_or_default();
            let supply_findings = pentester::supply_chain::run_audit(&root.to_string_lossy()).await.unwrap_or_default();
            
            let mut all_findings = static_findings;
            all_findings.extend(supply_findings);

            let critical = all_findings.iter().filter(|f| f.severity == pentester::Severity::Critical).count();
            let high = all_findings.iter().filter(|f| f.severity == pentester::Severity::High).count();
            let medium = all_findings.iter().filter(|f| f.severity == pentester::Severity::Medium).count();

            detail = format!("{} findings ({} critical, {} high, {} medium)", all_findings.len(), critical, high, medium);

            let fail_on = if strict_security {
                critical + high + medium > 0
            } else {
                critical + high > 0
            };

            if fail_on {
                passed = false;
                overall_passed = false;
            }

            let duration = t0.elapsed().as_millis();
            steps.push(StepResult {
                name: name.clone(),
                passed,
                skipped,
                duration,
                detail: detail.clone(),
            });
            let icon = if passed { "✓" } else { "✗" };
            println!("{}  {}: {}  ({}ms)", icon, name, detail, duration);

            if !passed {
                println!("\n  [FAIL-FAST] Aborting pipeline due to security findings.");
                finish_ci(false, steps, started_at, &uneven_dir, output, project_name, github, lock).await?;
                return Ok(());
            }
        }
    }

    // 3. Test Suite
    {
        let t0 = Instant::now();
        let name = "Test suite".to_string();
        let mut passed = true;
        let mut skipped = false;
        let detail: String;

        if skip_tests {
            skipped = true;
            detail = "skipped via --skip-tests".to_string();
            steps.push(StepResult {
                name: name.clone(),
                passed: true,
                skipped,
                duration: 0,
                detail: detail.clone(),
            });
            println!("⊙  {}: skipped", name);
        } else {
            let runner = TestRunner::new(&root);
            let detected = runner.detect();

            if detected.framework == TestFramework::Unknown {
                skipped = true;
                detail = "no test framework detected — skipped".to_string();
                let duration = t0.elapsed().as_millis();
                steps.push(StepResult {
                    name: name.clone(),
                    passed: true,
                    skipped,
                    duration,
                    detail: detail.clone(),
                });
                println!("⊙  {}: {}", name, detail);
            } else {
                println!("\n  Framework : {}", detected.framework);
                println!("  Command   : {}\n", detected.command);

                let test_config = TestRunnerConfig {
                    target_file: None,
                    ..Default::default()
                };

                match runner.run(test_config).await {
                    Ok(res) => {
                        let duration = t0.elapsed().as_millis();
                        detail = format!(
                            "{}/{} passed, {} failed, {} skipped",
                            res.passing, res.total, res.failing, res.skipped
                        );

                        if !res.passed && !res.failed_tests.is_empty() {
                            println!("\n--- Failed tests ---");
                            for t in res.failed_tests.iter().take(20) {
                                println!("  ✗ {}", t);
                            }
                            println!("---");
                        }

                        if !res.passed {
                            passed = false;
                            overall_passed = false;
                        }

                        steps.push(StepResult {
                            name: name.clone(),
                            passed,
                            skipped,
                            duration,
                            detail: detail.clone(),
                        });
                        let icon = if passed { "✓" } else { "✗" };
                        println!("{}  {}: {}  ({}ms)", icon, name, detail, duration);
                    }
                    Err(e) => {
                        passed = false;
                        overall_passed = false;
                        detail = format!("failed to run tests: {}", e);
                        let duration = t0.elapsed().as_millis();
                        steps.push(StepResult {
                            name: name.clone(),
                            passed,
                            skipped,
                            duration,
                            detail: detail.clone(),
                        });
                        println!("✗  {}: {}", name, detail);
                    }
                }
            }
        }
    }

    finish_ci(overall_passed, steps, started_at, &uneven_dir, output, project_name, github, lock).await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn finish_ci(
    passed: bool,
    steps: Vec<StepResult>,
    started_at: Instant,
    uneven_dir: &Path,
    output: Option<String>,
    project_name: &str,
    github: bool,
    lock: ProcessLock,
) -> Result<()> {
    let total_duration = started_at.elapsed().as_millis();
    let summary = CiSummary {
        passed,
        steps: steps.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        duration_ms: total_duration,
    };

    println!("\n{}", "=".repeat(60));
    println!("  Summary  ({}ms)", total_duration);
    println!("============================================================");
    for s in &steps {
        let icon = if s.skipped { "⊙" } else if s.passed { "✓" } else { "✗" };
        println!("  {}  {:28} {}", icon, s.name, s.detail);
    }
    println!();
    println!("{}", if passed { "  RESULT: PASSED" } else { "  RESULT: FAILED" });
    println!();

    let json_path = output.unwrap_or_else(|| {
        let p = uneven_dir.join("ci-summary.json");
        p.to_string_lossy().to_string()
    });

    let json_content = serde_json::to_string_pretty(&summary)?;
    fs::write(&json_path, json_content)?;
    println!("  Report: {}", json_path);

    if github {
        if let Ok(summary_path) = std::env::var("GITHUB_STEP_SUMMARY") {
            if !summary_path.is_empty() {
                let status_icon = if passed { "✅" } else { "❌" };
                let mut rows = String::new();
                for s in &steps {
                    let icon = if s.skipped { "⊙" } else if s.passed { "✅" } else { "❌" };
                    rows.push_str(&format!(
                        "| {} | {} | {} | {}ms |\n",
                        icon, s.name, s.detail, s.duration
                    ));
                }

                let content = format!(
                    "## {} Uneven CI — {}\n\n\
                     **Result:** {}\n\
                     **Duration:** {}ms\n\
                     **Timestamp:** {}\n\n\
                     | Status | Step | Detail | Duration |\n\
                     |--------|------|--------|----------|\n\
                     {}",
                    status_icon,
                    project_name,
                    if passed { "PASSED" } else { "FAILED" },
                    total_duration,
                    summary.timestamp,
                    rows
                );

                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&summary_path)?;
                use std::io::Write;
                write!(file, "{}", content)?;
            }
        }
    }

    if !passed {
        drop(lock);
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let _ = std::io::stderr().flush();
        }
        std::process::exit(1);
    }

    Ok(())
}
