use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[cfg(windows)]
mod win32 {
    #[repr(C)]
    #[allow(non_snake_case)]
    pub struct MEMORYSTATUSEX {
        pub dwLength: u32,
        pub dwMemoryLoad: u32,
        pub ullTotalPhys: u64,
        pub ullAvailPhys: u64,
        pub ullTotalPageFile: u64,
        pub ullAvailPageFile: u64,
        pub ullTotalVirtual: u64,
        pub ullAvailVirtual: u64,
        pub ullAvailExtendedVirtual: u64,
    }

    #[repr(C)]
    pub struct FILETIME {
        pub dwLowDateTime: u32,
        pub dwHighDateTime: u32,
    }

    unsafe extern "system" {
        pub fn GlobalMemoryStatusEx(lpBuffer: *mut MEMORYSTATUSEX) -> i32;
        pub fn GetSystemTimes(
            lpIdleTime: *mut FILETIME,
            lpKernelTime: *mut FILETIME,
            lpUserTime: *mut FILETIME,
        ) -> i32;
    }

    pub fn filetime_to_u64(ft: &FILETIME) -> u64 {
        ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    pub const HOST_CPU_LOAD_INFO: i32 = 3;
    pub const HOST_CPU_LOAD_INFO_COUNT: usize = 4;

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct host_cpu_load_info {
        pub cpu_ticks: [u32; 4],
    }

    unsafe extern "C" {
        pub fn mach_host_self() -> u32;
        pub fn host_statistics(
            host_priv: u32,
            flavor: i32,
            info_out: *mut i32,
            info_out_cnt: *mut u32,
        ) -> i32;
        pub fn sysctlbyname(
            name: *const i8,
            oldp: *mut std::ffi::c_void,
            oldlenp: *mut usize,
            newp: *mut std::ffi::c_void,
            newlen: usize,
        ) -> i32;
    }
}

/// System memory metrics: (available_mb, used_percent)
#[cfg(target_os = "linux")]
pub fn get_memory_info() -> (u64, f64) {
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        let mut total_kb = 0u64;
        let mut avail_kb = 0u64;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    total_kb = parts[1].parse::<u64>().unwrap_or(0);
                }
            } else if line.starts_with("MemAvailable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    avail_kb = parts[1].parse::<u64>().unwrap_or(0);
                }
            }
        }

        if total_kb > 0 {
            let used_kb = total_kb.saturating_sub(avail_kb);
            let used_pct = (used_kb as f64 / total_kb as f64) * 100.0;
            return (avail_kb / 1024, used_pct);
        }
    }
    (1024, 50.0)
}

#[cfg(target_os = "windows")]
pub fn get_memory_info() -> (u64, f64) {
    unsafe {
        let mut status = win32::MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<win32::MEMORYSTATUSEX>() as u32,
            dwMemoryLoad: 0,
            ullTotalPhys: 0,
            ullAvailPhys: 0,
            ullTotalPageFile: 0,
            ullAvailPageFile: 0,
            ullTotalVirtual: 0,
            ullAvailVirtual: 0,
            ullAvailExtendedVirtual: 0,
        };
        if win32::GlobalMemoryStatusEx(&mut status) != 0 {
            let free_mb = status.ullAvailPhys / (1024 * 1024);
            let used_pct = status.dwMemoryLoad as f64;
            return (free_mb, used_pct);
        }
    }
    (1024, 50.0)
}

#[cfg(target_os = "macos")]
pub fn get_memory_info() -> (u64, f64) {
    unsafe {
        let mut page_size: u64 = 4096;
        let mut size = std::mem::size_of::<u64>();
        let _ = macos::sysctlbyname(
            b"hw.pagesize\0".as_ptr() as *const i8,
            &mut page_size as *mut u64 as *mut std::ffi::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        );

        let mut total_mem: u64 = 0;
        let mut total_size = std::mem::size_of::<u64>();
        let _ = macos::sysctlbyname(
            b"hw.memsize\0".as_ptr() as *const i8,
            &mut total_mem as *mut u64 as *mut std::ffi::c_void,
            &mut total_size,
            std::ptr::null_mut(),
            0,
        );

        let mut vm_stats = [0i32; 15];
        let mut count = 15u32;
        let host_port = macos::mach_host_self();
        if macos::host_statistics(host_port, 2, vm_stats.as_mut_ptr(), &mut count) == 0 {
            let free_pages = vm_stats[0] as u64;
            let inactive_pages = vm_stats[2] as u64;
            let avail_bytes = (free_pages + inactive_pages) * page_size;
            let avail_mb = avail_bytes / (1024 * 1024);
            let used_pct = if total_mem > 0 {
                (total_mem.saturating_sub(avail_bytes) as f64 / total_mem as f64) * 100.0
            } else {
                50.0
            };
            return (avail_mb, used_pct);
        }
    }
    (1024, 50.0)
}

/// Raw CPU counters: (idle_ticks, total_ticks)
pub type CpuSample = (u64, u64);
/// Aggregate CPU counters and per-core counters
pub type DetailedCpuStats = (Option<CpuSample>, Vec<CpuSample>);

/// Reads per-core CPU and total CPU counters: (total_cpu, Vec<per_core_cpu>)
#[cfg(target_os = "linux")]
pub fn get_cpu_stats_detailed() -> DetailedCpuStats {
    let mut total_stat = None;
    let mut core_stats = Vec::new();

    if let Ok(content) = fs::read_to_string("/proc/stat") {
        for line in content.lines() {
            if line.starts_with("cpu ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let user = parts[1].parse::<u64>().unwrap_or(0);
                    let nice = parts[2].parse::<u64>().unwrap_or(0);
                    let system = parts[3].parse::<u64>().unwrap_or(0);
                    let idle = parts[4].parse::<u64>().unwrap_or(0);
                    let total = user + nice + system + idle;
                    total_stat = Some((idle, total));
                }
            } else if line.starts_with("cpu") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 && parts[0][3..].chars().all(|c| c.is_ascii_digit()) {
                    let user = parts[1].parse::<u64>().unwrap_or(0);
                    let nice = parts[2].parse::<u64>().unwrap_or(0);
                    let system = parts[3].parse::<u64>().unwrap_or(0);
                    let idle = parts[4].parse::<u64>().unwrap_or(0);
                    let total = user + nice + system + idle;
                    core_stats.push((idle, total));
                }
            }
        }
    }

    (total_stat, core_stats)
}

#[cfg(not(target_os = "linux"))]
pub fn get_cpu_stats_detailed() -> DetailedCpuStats {
    // On non-Linux systems, fallback to overall CPU stats
    (get_cpu_stats_aggregate(), Vec::new())
}

#[cfg(target_os = "windows")]
fn get_cpu_stats_aggregate() -> Option<(u64, u64)> {
    unsafe {
        let mut idle_ft = win32::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let mut kernel_ft = win32::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let mut user_ft = win32::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };

        if win32::GetSystemTimes(&mut idle_ft, &mut kernel_ft, &mut user_ft) != 0 {
            let idle = win32::filetime_to_u64(&idle_ft);
            let kernel = win32::filetime_to_u64(&kernel_ft);
            let user = win32::filetime_to_u64(&user_ft);
            return Some((idle, kernel + user));
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn get_cpu_stats_aggregate() -> Option<(u64, u64)> {
    unsafe {
        let host_port = macos::mach_host_self();
        let mut cpu_info = macos::host_cpu_load_info { cpu_ticks: [0; 4] };
        let mut count = macos::HOST_CPU_LOAD_INFO_COUNT as u32;

        if macos::host_statistics(
            host_port,
            macos::HOST_CPU_LOAD_INFO,
            &mut cpu_info as *mut macos::host_cpu_load_info as *mut i32,
            &mut count,
        ) == 0 {
            let user = cpu_info.cpu_ticks[0] as u64;
            let system = cpu_info.cpu_ticks[1] as u64;
            let idle = cpu_info.cpu_ticks[2] as u64;
            let nice = cpu_info.cpu_ticks[3] as u64;
            return Some((idle, user + system + idle + nice));
        }
    }
    None
}

/// Reads integrated GPU active frequency (MHz)
pub fn get_igpu_frequency_mhz() -> Option<u32> {
    let candidate_paths = [
        "/sys/class/drm/card1/gt_act_freq_mhz",
        "/sys/class/drm/card0/gt_act_freq_mhz",
    ];

    for p in candidate_paths {
        if let Ok(content) = fs::read_to_string(p)
            && let Ok(freq) = content.trim().parse::<u32>() {
                return Some(freq);
            }
    }

    None
}

/// Incident payload reported when the guardian triggers a safety interrupt.
#[derive(Debug, Clone)]
pub struct GuardianIncident {
    pub timestamp: String,
    pub reason: String,
    pub provider: String,
    pub cpu_usage: f64,
    pub per_core_cpu: Vec<f64>,
    pub ram_used_percent: f64,
    pub free_mem_mb: u64,
    pub igpu_freq: Option<u32>,
}

/// Configuration thresholds for the ResourceGuardian.
#[derive(Debug, Clone)]
pub struct GuardianConfig {
    pub provider: String,
    pub check_interval: Duration,
    pub cpu_core_threshold: f64,
    pub consecutive_cpu_violations: usize,
    pub ram_percent_threshold: f64,
    pub min_free_ram_mb: u64,
    pub hard_exit_on_interrupt: bool,
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            provider: "local".to_string(),
            check_interval: Duration::from_millis(1000),
            cpu_core_threshold: 89.0,
            consecutive_cpu_violations: 3,
            ram_percent_threshold: 85.0,
            min_free_ram_mb: 400,
            hard_exit_on_interrupt: true,
        }
    }
}

/// High-performance background watchdog protecting systems from resource starvation during AI inference.
pub struct ResourceGuardian {
    stop_signal: Arc<AtomicBool>,
}

impl ResourceGuardian {
    pub fn start(provider: String, project_root: PathBuf) -> Self {
        let config = GuardianConfig {
            provider,
            ..Default::default()
        };
        Self::start_with_config(config, project_root, None)
    }

    pub fn start_with_config(
        config: GuardianConfig,
        project_root: PathBuf,
        on_interrupt: Option<Arc<dyn Fn(GuardianIncident) + Send + Sync>>,
    ) -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_signal);

        let _ = std::thread::Builder::new()
            .name("resource-guardian".to_string())
            .spawn(move || {
                let log_dir = project_root.join(".uneven/logs");
                let _ = fs::create_dir_all(&log_dir);
                let log_path = log_dir.join("guardian.md");

                let mut prev_total_cpu = None;
                let mut prev_core_cpu: Vec<(u64, u64)> = Vec::new();
                let mut cpu_violations = 0;
                let mut last_log_time: Option<std::time::Instant> = None;

                while !stop_clone.load(Ordering::Relaxed) {
                    std::thread::sleep(config.check_interval);

                let (free_mem_mb, ram_used_pct) = get_memory_info();
                let igpu_freq = get_igpu_frequency_mhz();

                let (curr_total_sample, curr_core_sample) = get_cpu_stats_detailed();

                // Compute overall CPU %
                let cpu_usage = if let (Some((i1, t1)), Some((i2, t2))) = (prev_total_cpu, curr_total_sample) {
                    let idle_diff = i2.saturating_sub(i1);
                    let total_diff = t2.saturating_sub(t1);
                    if total_diff > 0 {
                        ((1.0 - (idle_diff as f64) / (total_diff as f64)) * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                prev_total_cpu = curr_total_sample;

                // Compute per-core CPU %
                let mut per_core_cpu = Vec::new();
                if !prev_core_cpu.is_empty() && prev_core_cpu.len() == curr_core_sample.len() {
                    for ((i1, t1), (i2, t2)) in prev_core_cpu.iter().zip(curr_core_sample.iter()) {
                        let idle_diff = i2.saturating_sub(*i1);
                        let total_diff = t2.saturating_sub(*t1);
                        let pct = if total_diff > 0 {
                            ((1.0 - (idle_diff as f64) / (total_diff as f64)) * 100.0).clamp(0.0, 100.0)
                        } else {
                            0.0
                        };
                        per_core_cpu.push(pct);
                    }
                }
                prev_core_cpu = curr_core_sample;

                // Check if all available CPU cores are saturated
                let all_cores_saturated = if !per_core_cpu.is_empty() {
                    per_core_cpu.iter().all(|&c| c >= config.cpu_core_threshold)
                } else {
                    cpu_usage >= config.cpu_core_threshold
                };

                let should_log = last_log_time.is_none()
                    || ram_used_pct >= 80.0
                    || free_mem_mb < config.min_free_ram_mb
                    || all_cores_saturated
                    || last_log_time.is_some_and(|t| t.elapsed() >= Duration::from_secs(30));

                if should_log {
                    last_log_time = Some(std::time::Instant::now());
                    let cores_str = if !per_core_cpu.is_empty() {
                        per_core_cpu
                            .iter()
                            .enumerate()
                            .map(|(i, c)| format!("C{i}:{c:.0}%"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    } else {
                        "N/A".to_string()
                    };

                    let igpu_str = igpu_freq.map(|f| format!("{f}MHz")).unwrap_or_else(|| "N/A".to_string());
                    let now = chrono::Utc::now().to_rfc3339();

                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
                        let _ = writeln!(
                            file,
                            "[{now}] Provider: {}, Total CPU: {:.1}%, Cores: [{cores_str}], RAM: {:.1}% ({}MB free), iGPU: {}",
                            config.provider, cpu_usage, ram_used_pct, free_mem_mb, igpu_str
                        );
                    }
                }

                // Check RAM thresholds
                let ram_violated = ram_used_pct >= config.ram_percent_threshold
                    || free_mem_mb < config.min_free_ram_mb;

                if ram_violated {
                    let reason = format!(
                        "Memory limit reached: RAM usage is at {:.1}% ({}MB free, safety threshold: {:.1}%)",
                        ram_used_pct, free_mem_mb, config.ram_percent_threshold
                    );
                    let incident = GuardianIncident {
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        reason,
                        provider: config.provider.clone(),
                        cpu_usage,
                        per_core_cpu,
                        ram_used_percent: ram_used_pct,
                        free_mem_mb,
                        igpu_freq,
                    };
                    Self::trigger_interrupt(&log_path, incident, &config, on_interrupt.as_ref());
                    break;
                }

                // Check CPU thresholds
                if all_cores_saturated {
                    cpu_violations += 1;
                    if cpu_violations >= config.consecutive_cpu_violations {
                        let reason = format!(
                            "Sustained CPU saturation: all cores reached >= {:.1}% for {} consecutive samples",
                            config.cpu_core_threshold, config.consecutive_cpu_violations
                        );
                        let incident = GuardianIncident {
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            reason,
                            provider: config.provider.clone(),
                            cpu_usage,
                            per_core_cpu,
                            ram_used_percent: ram_used_pct,
                            free_mem_mb,
                            igpu_freq,
                        };
                        Self::trigger_interrupt(&log_path, incident, &config, on_interrupt.as_ref());
                        break;
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

    fn trigger_interrupt(
        log_path: &Path,
        incident: GuardianIncident,
        config: &GuardianConfig,
        on_interrupt: Option<&Arc<dyn Fn(GuardianIncident) + Send + Sync>>,
    ) {
        // 1. Immediately signal inference engine to abort token generation
        crate::infrastructure::llm::interrupt_local_inference();

        // 2. Persist incident to audit log (all in English)
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = writeln!(
                file,
                "[{}] SAFETY INTERRUPT: {} | Total CPU: {:.1}% | RAM: {:.1}% ({}MB free) | Provider: {}",
                incident.timestamp, incident.reason, incident.cpu_usage, incident.ram_used_percent, incident.free_mem_mb, incident.provider
            );
        }

        // 3. Dispatch incident to optional callback (e.g. UI alert banner)
        if let Some(cb) = on_interrupt {
            cb(incident.clone());
        }

        // 4. Output English warning to terminal
        eprintln!("\n\x1b[1;38;2;255;107;53m  [SAFETY INTERRUPT] {}\x1b[0m", incident.reason);
        if incident.provider == "local" {
            eprintln!("\x1b[2m  Inference process paused to prevent system freezing.\x1b[0m\n");
        } else {
            eprintln!("\x1b[2m  System is under heavy external load (mode: {}).\x1b[0m", incident.provider);
            eprintln!("\x1b[2m  Process safely paused to keep machine responsive.\x1b[0m\n");
        }

        if config.hard_exit_on_interrupt {
            std::process::exit(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_info_retrieval() {
        let (avail_mb, used_pct) = get_memory_info();
        assert!(avail_mb > 0, "Available memory should be greater than 0");
        assert!(used_pct >= 0.0 && used_pct <= 100.0, "Used memory percent should be between 0 and 100");
    }

    #[test]
    fn test_guardian_default_config() {
        let config = GuardianConfig::default();
        assert_eq!(config.cpu_core_threshold, 89.0);
        assert_eq!(config.consecutive_cpu_violations, 3);
        assert_eq!(config.ram_percent_threshold, 85.0);
        assert_eq!(config.min_free_ram_mb, 400);
    }

    #[test]
    fn test_guardian_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let config = GuardianConfig {
            provider: "test".to_string(),
            check_interval: Duration::from_millis(50),
            hard_exit_on_interrupt: false,
            ..Default::default()
        };

        let guardian = ResourceGuardian::start_with_config(config, dir.path().to_path_buf(), None);
        std::thread::sleep(Duration::from_millis(150));
        guardian.stop();
    }
}
