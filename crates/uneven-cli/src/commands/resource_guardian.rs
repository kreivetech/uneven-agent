use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use crate::ui::{truecolor_bold, blank, dim};
use super::explain::{get_free_memory_mb, get_cpu_stats};

pub struct ResourceGuardian {
    stop_signal: Arc<AtomicBool>,
}

impl ResourceGuardian {
    pub fn start(
        provider: String,
        project_root: std::path::PathBuf,
    ) -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();

        tokio::spawn(async move {
            let log_dir = project_root.join(".uneven/logs");
            let _ = fs::create_dir_all(&log_dir);
            let log_path = log_dir.join("guardian.md");

            let mut cpu_sample = get_cpu_stats();
            let mut cpu_violations = 0;
            let mut last_log_time: Option<std::time::Instant> = None;

            while !stop_clone.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_secs(1)).await;

                let free_mem = get_free_memory_mb();
                let cpu_usage = if let Some((idle1, total1)) = cpu_sample {
                    if let Some((idle2, total2)) = get_cpu_stats() {
                        let idle_diff = idle2.saturating_sub(idle1);
                        let total_diff = total2.saturating_sub(total1);
                        cpu_sample = Some((idle2, total2));
                        if total_diff > 0 {
                            (1.0 - (idle_diff as f64) / (total_diff as f64)) * 100.0
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                } else {
                    cpu_sample = get_cpu_stats();
                    0.0
                };

                let should_log = last_log_time.is_none()
                    || free_mem < 500
                    || cpu_usage > 80.0
                    || last_log_time.unwrap().elapsed() >= Duration::from_secs(30);

                if should_log {
                    last_log_time = Some(std::time::Instant::now());
                    // Log stats to guardian.md
                    if let Ok(mut file) = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&log_path)
                    {
                        let now = chrono::Utc::now().to_rfc3339();
                        let _ = writeln!(
                            file,
                            "[{}] Provider: {}, CPU Usage: {:.1}%, Free Mem: {}MB",
                            now, provider, cpu_usage, free_mem
                        );
                    }
                }

                // Check RAM
                if free_mem < 200 {
                    let reason = format!("Memory critically low ({}MB free)", free_mem);
                    Self::log_interrupt(&log_path, &provider, &reason);
                    Self::interrupt(&provider, &reason);
                }

                // Check CPU
                if cpu_usage > 87.0 {
                    cpu_violations += 1;
                    if cpu_violations >= 3 {
                        let reason = format!("Sustained CPU usage too high ({:.1}%)", cpu_usage);
                        Self::log_interrupt(&log_path, &provider, &reason);
                        Self::interrupt(&provider, &reason);
                    }
                } else {
                    cpu_violations = 0;
                }
            }
        });

        Self { stop_signal }
    }

    pub fn stop(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }

    fn log_interrupt(log_path: &std::path::Path, provider: &str, reason: &str) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            let now = chrono::Utc::now().to_rfc3339();
            let _ = writeln!(
                file,
                "[{}] SAFETY INTERRUPT: {} (Provider: {})",
                now, reason, provider
            );
        }
    }

    fn interrupt(provider: &str, reason: &str) -> ! {
        // Interrupt inference
        uneven_core::infrastructure::llm::interrupt_local_inference();

        // Print warning matching TS style
        println!("\n{}", truecolor_bold(&format!("  Safety interrupt: {}", reason), 255, 107, 53));
        blank();
        if provider == "local" {
            println!("{}", dim("  Action stopped to prevent system freeze during local inference."));
        } else {
            println!("{}", dim(&format!("  System is under heavy external load ({} mode).", provider)));
            println!("{}", dim("  Action stopped to keep your machine responsive."));
        }
        blank();
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        std::process::exit(0);
    }
}
