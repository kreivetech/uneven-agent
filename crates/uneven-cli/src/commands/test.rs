use anyhow::Result;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use uneven_core::domain::testing::{TestFramework, TestRunnerConfig};
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::utils::test_runner::TestRunner;

pub async fn run(file: Option<String>) -> Result<()> {
    let root = loader::find_project_root(None);
    let runner = TestRunner::new(&root);

    let detection = runner.detect();
    if detection.framework == TestFramework::Unknown {
        println!("{}", style("  [UE-02000001] ⚠ No test framework detected.").yellow());
        println!("{}", style("  Supported: Jest, Vitest, Mocha, Jasmine, Pytest, Cargo test, Go test, PHPUnit, RSpec").dim());
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        std::process::exit(1);
    }

    let framework_name = format!("{:?}", detection.framework);

    let msg = if let Some(ref f) = file {
        format!("Running {} tests for {}...", style(&framework_name).cyan(), f)
    } else {
        format!("Running {} tests...", style(&framework_name).cyan())
    };

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")?,
    );
    spinner.set_message(msg);
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let config = TestRunnerConfig {
        timeout_ms: Some(120_000),
        target_file: file.clone(),
        env: None,
    };

    match runner.run(config).await {
        Ok(result) => {
            spinner.finish_and_clear();

            if result.passed {
                println!(
                    "{} {}  {}",
                    style("✓").green().bold(),
                    style(format!("{}/{} tests passed", result.passing, result.total)).green(),
                    style(format!("[{:?}] {}ms", result.framework, result.duration)).dim()
                );
            } else {
                println!(
                    "{} {}  {}",
                    style("✗").red().bold(),
                    style(format!("{}/{} tests failed", result.failing, result.total)).red(),
                    style(format!("[{:?}] {}ms", result.framework, result.duration)).dim()
                );

                if !result.failed_tests.is_empty() {
                    println!();
                    println!("{}", style("Failed tests:").bold());
                    for t in result.failed_tests.iter().take(10) {
                        println!("  {} {}", style("✗").red(), t);
                    }
                    if result.failed_tests.len() > 10 {
                        println!("{}", style(format!("  ... and {} more", result.failed_tests.len() - 10)).dim());
                    }
                }

                if result.skipped > 0 {
                    println!("\n  {} skipped", result.skipped);
                }

                {
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }
                std::process::exit(1);
            }
        }
        Err(err) => {
            spinner.finish_and_clear();
            println!("{}", style(format!("Test run failed: {}", err)).red());
            {
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            std::process::exit(1);
        }
    }

    Ok(())
}
