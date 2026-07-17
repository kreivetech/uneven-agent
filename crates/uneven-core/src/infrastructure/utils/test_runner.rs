use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use regex::Regex;
use crate::domain::testing::{TestFramework, TestResult, TestRunnerConfig};
use crate::UnevenError;

pub struct FrameworkDetection {
    pub framework: TestFramework,
    pub command: String,
}

pub struct TestRunner {
    project_root: PathBuf,
}

impl TestRunner {
    pub fn new<P: AsRef<Path>>(project_root: P) -> Self {
        Self {
            project_root: project_root.as_ref().to_path_buf(),
        }
    }

    pub fn detect(&self) -> FrameworkDetection {
        let root = &self.project_root;

        let has_package = |pkg: &str| -> bool {
            let pkg_json_path = root.join("package.json");
            if let Ok(content) = fs::read_to_string(pkg_json_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let dep = json.get("dependencies").and_then(|d| d.get(pkg)).is_some();
                    let dev_dep = json.get("devDependencies").and_then(|d| d.get(pkg)).is_some();
                    return dep || dev_dep;
                }
            }
            false
        };

        let has_script = |keyword: &str| -> bool {
            let pkg_json_path = root.join("package.json");
            if let Ok(content) = fs::read_to_string(pkg_json_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) {
                        return scripts.values().any(|val| {
                            val.as_str().map(|s| s.contains(keyword)).unwrap_or(false)
                        });
                    }
                }
            }
            false
        };

        let file_exists = |filename: &str| -> bool {
            root.join(filename).exists()
        };

        // 1. Vitest
        if has_package("vitest")
            || has_script("vitest")
            || file_exists("vite.config.ts")
            || file_exists("vitest.config.ts")
        {
            return FrameworkDetection {
                framework: TestFramework::Vitest,
                command: "npx vitest run --reporter=verbose".to_string(),
            };
        }

        // 2. Jest
        if has_package("jest")
            || has_package("@jest/core")
            || has_script("jest")
            || file_exists("jest.config.ts")
            || file_exists("jest.config.js")
            || file_exists("jest.config.json")
        {
            return FrameworkDetection {
                framework: TestFramework::Jest,
                command: "npx jest --no-coverage".to_string(),
            };
        }

        // 3. Mocha
        if has_package("mocha")
            || has_script("mocha")
            || file_exists(".mocharc.js")
            || file_exists(".mocharc.json")
            || file_exists(".mocharc.yml")
        {
            return FrameworkDetection {
                framework: TestFramework::Mocha,
                command: "npx mocha --reporter spec".to_string(),
            };
        }

        // 4. Jasmine
        if has_package("jasmine")
            || file_exists("jasmine.json")
            || file_exists("spec/support/jasmine.json")
        {
            return FrameworkDetection {
                framework: TestFramework::Jasmine,
                command: "npx jasmine".to_string(),
            };
        }

        // 5. Pytest
        if file_exists("pytest.ini")
            || file_exists("pyproject.toml")
            || file_exists("setup.cfg")
            || file_exists("conftest.py")
        {
            return FrameworkDetection {
                framework: TestFramework::Pytest,
                command: "python -m pytest -v".to_string(),
            };
        }

        // 6. Cargo test
        if file_exists("Cargo.toml") {
            return FrameworkDetection {
                framework: TestFramework::CargoTest,
                command: "cargo test".to_string(),
            };
        }

        // 7. Go test
        if file_exists("go.mod") {
            return FrameworkDetection {
                framework: TestFramework::GoTest,
                command: "go test ./...".to_string(),
            };
        }

        // 8. PHPUnit
        if file_exists("phpunit.xml")
            || file_exists("phpunit.xml.dist")
            || has_package("phpunit/phpunit")
        {
            return FrameworkDetection {
                framework: TestFramework::Phpunit,
                command: "vendor/bin/phpunit".to_string(),
            };
        }

        // 9. RSpec
        if file_exists("Gemfile")
            && (file_exists(".rspec") || file_exists("spec/spec_helper.rb"))
        {
            return FrameworkDetection {
                framework: TestFramework::Rspec,
                command: "bundle exec rspec".to_string(),
            };
        }

        FrameworkDetection {
            framework: TestFramework::Unknown,
            command: "".to_string(),
        }
    }

    pub async fn run(&self, config: TestRunnerConfig) -> Result<TestResult, UnevenError> {
        let detection = self.detect();
        if detection.framework == TestFramework::Unknown || detection.command.is_empty() {
            return Ok(TestResult {
                framework: TestFramework::Unknown,
                passed: true,
                total: 0,
                passing: 0,
                failing: 0,
                skipped: 0,
                duration: 0,
                output: "".to_string(),
                failed_tests: Vec::new(),
            });
        }

        let mut full_command = detection.command;

        if let Some(ref target_file) = config.target_file {
            let path_obj = Path::new(target_file);
            let file_stem = path_obj.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            
            match detection.framework {
                TestFramework::Vitest | TestFramework::Jest => {
                    let escaped = regex::escape(file_stem);
                    full_command = format!("{} --testPathPattern={}", full_command, escaped);
                }
                TestFramework::Mocha => {
                    full_command = format!("{} --grep \"{}\"", full_command, file_stem);
                }
                TestFramework::Pytest => {
                    if let Some(parent) = path_obj.parent() {
                        full_command = format!("{} {}", full_command, parent.to_string_lossy());
                    }
                }
                TestFramework::Rspec => {
                    full_command = format!("{} spec/{}_spec.rb", full_command, file_stem);
                }
                _ => {}
            }
        }

        let start = Instant::now();
        let timeout_ms = config.timeout_ms.unwrap_or(120_000);

        let project_root_str = self.project_root.to_string_lossy().to_string();
        
        let output_res = tokio::task::spawn_blocking(move || {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", &full_command])
               .current_dir(project_root_str)
               .env("CI", "true")
               .env("FORCE_COLOR", "0")
               .stdout(std::process::Stdio::piped())
               .stderr(std::process::Stdio::piped());

            if let Some(ref extra_env) = config.env {
                for (k, v) in extra_env {
                    cmd.env(k, v);
                }
            }

            let mut child = cmd.spawn()?;
            
            let mut stdout = child.stdout.take().ok_or_else(|| std::io::Error::other("Failed to capture stdout"))?;
            let mut stderr = child.stderr.take().ok_or_else(|| std::io::Error::other("Failed to capture stderr"))?;

            use std::sync::{Arc, Mutex};
            let output_buffer = Arc::new(Mutex::new(Vec::new()));
            
            let output_clone1 = output_buffer.clone();
            std::thread::spawn(move || {
                let mut buf = [0; 4096];
                while let Ok(n) = stdout.read(&mut buf) {
                    if n == 0 { break; }
                    let mut lock = output_clone1.lock().unwrap();
                    if lock.len() < 2 * 1024 * 1024 {
                        lock.extend_from_slice(&buf[..n]);
                    }
                }
            });

            let output_clone2 = output_buffer.clone();
            std::thread::spawn(move || {
                let mut buf = [0; 4096];
                while let Ok(n) = stderr.read(&mut buf) {
                    if n == 0 { break; }
                    let mut lock = output_clone2.lock().unwrap();
                    if lock.len() < 2 * 1024 * 1024 {
                        lock.extend_from_slice(&buf[..n]);
                    }
                }
            });

            let timeout_duration = Duration::from_millis(timeout_ms);
            let check_interval = Duration::from_millis(100);
            let timeout_start = Instant::now();

            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let final_buf = output_buffer.lock().unwrap().clone();
                        let final_output = String::from_utf8_lossy(&final_buf).to_string();
                        return Ok((status, final_output, false));
                    }
                    Ok(None) => {
                        if timeout_start.elapsed() >= timeout_duration {
                            let _ = child.kill();
                            let status = child.wait()?;
                            let final_buf = output_buffer.lock().unwrap().clone();
                            let final_output = String::from_utf8_lossy(&final_buf).to_string();
                            return Ok((status, final_output, true));
                        }
                        std::thread::sleep(check_interval);
                    }
                    Err(e) => return Err(e),
                }
            }
        }).await.map_err(|e| UnevenError::AgentError(format!("Blocking task failed: {}", e)))?;

        let (status, raw_output, timed_out) = output_res.map_err(UnevenError::IoError)?;
        let duration = start.elapsed().as_millis() as u64;

        let (total, passing, failing, skipped, failed_tests) = parse_summary(detection.framework, &raw_output);

        let passed = !timed_out && status.success();

        Ok(TestResult {
            framework: detection.framework,
            passed,
            total,
            passing,
            failing: if timed_out { 0 } else { failing },
            skipped,
            duration,
            output: if raw_output.len() > 4000 {
                raw_output[raw_output.len() - 4000..].to_string()
            } else {
                raw_output
            },
            failed_tests,
        })
    }
}

fn parse_summary(framework: TestFramework, output: &str) -> (u32, u32, u32, u32, Vec<String>) {
    match framework {
        TestFramework::Jest | TestFramework::Vitest => parse_jest(output),
        TestFramework::Mocha => parse_mocha(output),
        TestFramework::Pytest => parse_pytest(output),
        TestFramework::CargoTest => parse_cargo(output),
        TestFramework::GoTest => parse_go(output),
        _ => parse_generic(output),
    }
}

fn parse_jest(output: &str) -> (u32, u32, u32, u32, Vec<String>) {
    let mut failed_tests = Vec::new();
    let re_fail = Regex::new(r"● (.+)").unwrap();
    for cap in re_fail.captures_iter(output) {
        let name = cap[1].trim();
        if !name.starts_with("Console") && !name.starts_with("----") {
            failed_tests.push(name.to_string());
        }
    }

    let re_summary = Regex::new(r"Tests:\s+(?:(\d+) failed,\s*)?(?:(\d+) skipped,\s*)?(\d+) passed,\s*(\d+) total").unwrap();
    if let Some(cap) = re_summary.captures(output) {
        let failing = cap.get(1).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        let skipped = cap.get(2).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        let passing = cap.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        let total = cap.get(4).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        return (total, passing, failing, skipped, failed_tests);
    }

    let re_pass = Regex::new(r"(\d+) passing").unwrap();
    let re_fail_mocha = Regex::new(r"(\d+) failing").unwrap();
    let passing = re_pass.captures(output).and_then(|c| c.get(1)).map_or(0, |m| m.as_str().parse().unwrap_or(0));
    let failing = re_fail_mocha.captures(output).and_then(|c| c.get(1)).map_or(0, |m| m.as_str().parse().unwrap_or(0));
    (passing + failing, passing, failing, 0, failed_tests)
}

fn parse_mocha(output: &str) -> (u32, u32, u32, u32, Vec<String>) {
    let mut failed_tests = Vec::new();
    let re_fail = Regex::new(r"\d+\) (.+)").unwrap();
    for cap in re_fail.captures_iter(output) {
        failed_tests.push(cap[1].trim().to_string());
    }

    let re_pass = Regex::new(r"(\d+) passing").unwrap();
    let re_fail_mocha = Regex::new(r"(\d+) failing").unwrap();
    let re_pending = Regex::new(r"(\d+) pending").unwrap();

    let passing = re_pass.captures(output).and_then(|c| c.get(1)).map_or(0, |m| m.as_str().parse().unwrap_or(0));
    let failing = re_fail_mocha.captures(output).and_then(|c| c.get(1)).map_or(0, |m| m.as_str().parse().unwrap_or(0));
    let pending = re_pending.captures(output).and_then(|c| c.get(1)).map_or(0, |m| m.as_str().parse().unwrap_or(0));

    (passing + failing + pending, passing, failing, pending, failed_tests)
}

fn parse_pytest(output: &str) -> (u32, u32, u32, u32, Vec<String>) {
    let mut failed_tests = Vec::new();
    let re_fail = Regex::new(r"FAILED (.+)").unwrap();
    for cap in re_fail.captures_iter(output) {
        failed_tests.push(cap[1].trim().to_string());
    }

    let re_summary = Regex::new(r"(\d+) passed(?:, (\d+) failed)?(?:, (\d+) skipped)?").unwrap();
    if let Some(cap) = re_summary.captures(output) {
        let passing = cap.get(1).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        let failing = cap.get(2).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        let skipped = cap.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        return (passing + failing + skipped, passing, failing, skipped, failed_tests);
    }
    (0, 0, 0, 0, failed_tests)
}

fn parse_cargo(output: &str) -> (u32, u32, u32, u32, Vec<String>) {
    let mut failed_tests = Vec::new();
    let re_fail = Regex::new(r"test (.+) \.\.\. FAILED").unwrap();
    for cap in re_fail.captures_iter(output) {
        failed_tests.push(cap[1].trim().to_string());
    }

    let re_summary = Regex::new(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored").unwrap();
    if let Some(cap) = re_summary.captures(output) {
        let passing = cap.get(1).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        let failing = cap.get(2).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        let skipped = cap.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        return (passing + failing + skipped, passing, failing, skipped, failed_tests);
    }
    (0, 0, 0, 0, failed_tests)
}

fn parse_go(output: &str) -> (u32, u32, u32, u32, Vec<String>) {
    let mut failed_tests = Vec::new();
    let re_fail = Regex::new(r"--- FAIL: (.+) \(").unwrap();
    for cap in re_fail.captures_iter(output) {
        failed_tests.push(cap[1].trim().to_string());
    }

    let passing = output.split("--- PASS").count() as u32 - 1;
    let failing = failed_tests.len() as u32;
    (passing + failing, passing, failing, 0, failed_tests)
}

fn parse_generic(output: &str) -> (u32, u32, u32, u32, Vec<String>) {
    let re_pass = Regex::new(r"(?i)(\d+) pass(?:ed|ing)").unwrap();
    let re_fail = Regex::new(r"(?i)(\d+) fail(?:ed|ing)").unwrap();
    let passing = re_pass.captures(output).and_then(|c| c.get(1)).map_or(0, |m| m.as_str().parse().unwrap_or(0));
    let failing = re_fail.captures(output).and_then(|c| c.get(1)).map_or(0, |m| m.as_str().parse().unwrap_or(0));
    (passing + failing, passing, failing, 0, Vec::new())
}
