use anyhow::{anyhow, Result};
use console::style;
use std::fs;
use std::io::Write;
use std::path::Path;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::security::laws;
use uneven_core::infrastructure::security::SafetyGuard;
use uneven_core::infrastructure::llm::inference::chat_template::{
    detect_template, apply_template,
};
use uneven_core::infrastructure::llm::{
    InferParams, init as init_llm, infer_with_params_streaming,
};

pub fn build_explain_prompt(file_path: &str, content: &str, focus: &str) -> String {
    let numbered_content = uneven_core::infrastructure::utils::formatter::format_with_line_numbers(content);
    let truncated = if numbered_content.len() > 15000 {
        format!(
            "{}\n\n[file truncated — showing first 15000 of {} chars]",
            &numbered_content[..15000],
            numbered_content.len()
        )
    } else {
        numbered_content
    };

    let focus_section = if !focus.trim().is_empty() {
        format!("## Question\nExplain specifically: {}\n", focus.trim())
    } else {
        "## Task\nProvide a clear explanation of this file covering:\n\
         1. What it does and why it exists\n\
         2. Key functions, classes, or exports and their responsibilities\n\
         3. Notable design decisions or patterns\n\
         4. Any potential issues or areas worth noting\n"
            .to_string()
    };

    format!(
        "You are a senior software engineer explaining code to a colleague. Be clear and concise.\n\n\
         ## File: {}\n\n\
         ```\n\
         {}\n\
         ```\n\n\
         ---\n\
         {}",
        file_path, truncated, focus_section
    )
}

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

    extern "system" {
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

    extern "C" {
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

#[cfg(target_os = "linux")]
pub fn get_free_memory_mb() -> u64 {
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemAvailable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return kb / 1024;
                    }
                }
            }
        }
    }
    1024
}

#[cfg(target_os = "linux")]
pub fn get_cpu_stats() -> Option<(u64, u64)> {
    let content = fs::read_to_string("/proc/stat").ok()?;
    let line = content.lines().next()?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 5 {
        let user = parts[1].parse::<u64>().ok()?;
        let nice = parts[2].parse::<u64>().ok()?;
        let system = parts[3].parse::<u64>().ok()?;
        let idle = parts[4].parse::<u64>().ok()?;
        let total = user + nice + system + idle;
        Some((idle, total))
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
pub fn get_free_memory_mb() -> u64 {
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
            return status.ullAvailPhys / (1024 * 1024);
        }
    }
    1024
}

#[cfg(target_os = "windows")]
pub fn get_cpu_stats() -> Option<(u64, u64)> {
    unsafe {
        let mut idle_ft = win32::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let mut kernel_ft = win32::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let mut user_ft = win32::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        
        if win32::GetSystemTimes(&mut idle_ft, &mut kernel_ft, &mut user_ft) != 0 {
            let idle = win32::filetime_to_u64(&idle_ft);
            let kernel = win32::filetime_to_u64(&kernel_ft);
            let user = win32::filetime_to_u64(&user_ft);
            let total = kernel + user;
            return Some((idle, total));
        }
    }
    None
}

#[cfg(target_os = "macos")]
pub fn get_free_memory_mb() -> u64 {
    unsafe {
        let mut page_size: u64 = 0;
        let mut size = std::mem::size_of::<u64>();
        if macos::sysctlbyname(
            b"hw.pagesize\0".as_ptr() as *const i8,
            &mut page_size as *mut u64 as *mut std::ffi::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0 {
            page_size = 4096;
        }

        let mut vm_stats = [0i32; 15];
        let mut count = 15u32;
        let host_port = macos::mach_host_self();
        if macos::host_statistics(host_port, 2, vm_stats.as_mut_ptr(), &mut count) == 0 {
            let free_pages = vm_stats[0] as u64;
            let inactive_pages = vm_stats[2] as u64;
            let available_mem = (free_pages + inactive_pages) * page_size;
            return available_mem / (1024 * 1024);
        }
    }
    1024
}

#[cfg(target_os = "macos")]
pub fn get_cpu_stats() -> Option<(u64, u64)> {
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
            let total = user + system + idle + nice;
            return Some((idle, total));
        }
    }
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn get_free_memory_mb() -> u64 {
    1024
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn get_cpu_stats() -> Option<(u64, u64)> {
    None
}

pub async fn run(
    file_path_str: String,
    focus: Option<String>,
    provider_opt: Option<String>,
    model_opt: Option<String>,
    no_guardian: bool,
) -> Result<()> {
    // 1. Resolve path and read physical file content
    let root = loader::find_project_root(None);
    let abs_path = fs::canonicalize(Path::new(&file_path_str))
        .map_err(|e| anyhow!("Could not resolve path '{}': {}", file_path_str, e))?;
    
    let content = fs::read_to_string(&abs_path)
        .map_err(|e| anyhow!("Could not read file '{}': {}", abs_path.display(), e))?;

    let lines = content.lines().count();
    let bytes = content.len();
    let kb = (bytes as f64 / 1024.0).to_string();
    let is_truncated = bytes > 15000;

    let rel_path = abs_path.strip_prefix(&root).unwrap_or(&abs_path);
    println!(
        "{} {} ({})",
        style("✓").green().bold(),
        style(rel_path.display()).green(),
        style(format!(
            "{} lines, {:.1} KB{}",
            lines,
            kb,
            if is_truncated { ", truncated" } else { "" }
        ))
        .dim()
    );

    // 2. Safety Guard Validation
    let safety = SafetyGuard::new();
    let prompt_label = format!("Explain file: {}", rel_path.display());
    if let Err(err) = safety.validate_intent(&prompt_label) {
        println!("\n{} {}", style("⚠️ [SECURITY POLICY]:").yellow().bold(), err);
        return Ok(());
    }

    // 3. Load configuration
    let config = loader::load_config(&root)?;
    let mut brain = config.get_brain_for_command("explain");
    if let Some(ref p) = provider_opt {
        brain.provider = p.clone();
    }
    if let Some(ref m) = model_opt {
        brain.model = m.clone();
    }

    // 4. Initialize LLM
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    init_llm(&brain.provider, &brain.model, threads).await?;

    // 5. Setup Resource Guardian
    let stop_guardian = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_guardian_clone = stop_guardian.clone();

    if !no_guardian {
        tokio::spawn(async move {
            let mut cpu_sample = get_cpu_stats();
            let mut cpu_violations = 0;

            while !stop_guardian_clone.load(std::sync::atomic::Ordering::Relaxed) {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                // Memory check
                let free_mem = get_free_memory_mb();
                if free_mem < 200 {
                    uneven_core::infrastructure::llm::interrupt_local_inference();
                    eprintln!("\n{}", style("  Safety interrupt: Memory critically low").red().bold());
                    eprintln!("{}", style("  (Use --no-guardian to bypass resource limits)").dim());
                    {
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                        let _ = std::io::stderr().flush();
                    }
                    std::process::exit(0);
                }

                // CPU check
                if let Some((idle1, total1)) = cpu_sample {
                    if let Some((idle2, total2)) = get_cpu_stats() {
                        let idle_diff = idle2.saturating_sub(idle1);
                        let total_diff = total2.saturating_sub(total1);
                        if total_diff > 0 {
                            let usage = (1.0 - (idle_diff as f64) / (total_diff as f64)) * 100.0;
                            if usage > 87.0 {
                                cpu_violations += 1;
                                if cpu_violations >= 3 {
                                    uneven_core::infrastructure::llm::interrupt_local_inference();
                                    eprintln!("\n{}", style("  Safety interrupt: Sustained CPU usage too high").red().bold());
                                    eprintln!("{}", style("  (Use --no-guardian to bypass resource limits)").dim());
                                    {
                                        use std::io::Write;
                                        let _ = std::io::stdout().flush();
                                        let _ = std::io::stderr().flush();
                                    }
                                    std::process::exit(0);
                                }
                            } else {
                                cpu_violations = 0;
                            }
                        }
                        cpu_sample = Some((idle2, total2));
                    }
                } else {
                    cpu_sample = get_cpu_stats();
                }
            }
        });
    } else {
        println!("{}", style("  ◈ ResourceGuardian disabled (unlimited CPU/RAM usage)").dim());
    }

    // 6. Prompt formatting
    let focus_str = focus.unwrap_or_default();
    let raw_prompt = build_explain_prompt(&rel_path.to_string_lossy(), &content, &focus_str);
    let system_instructions = laws::get_context("explain");

    let chat_template = detect_template(&brain.model);
    let formatted_prompt = apply_template(&chat_template, &system_instructions, &[], &raw_prompt);

    let params = InferParams {
        model: brain.model.clone(),
        threads,
        max_tokens: brain.max_tokens,
        temperature: brain.temperature as f64,
        repeat_penalty: 1.1,
        max_context: 4096,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: Some(system_instructions.clone()),
        history: None,
    };

    println!("\n{}", style("  ┌─ Explanation").dim());
    print!("  │\n  │  ");
    std::io::stdout().flush()?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
    let tx_clone = tx.clone();
    let prompt_to_use = if brain.provider == "local" {
        formatted_prompt
    } else {
        raw_prompt
    };
    let prompt_to_use_clone = prompt_to_use.clone();
    let params_clone = InferParams {
        model: params.model.clone(),
        threads: params.threads,
        max_tokens: params.max_tokens,
        temperature: params.temperature,
        repeat_penalty: params.repeat_penalty,
        max_context: params.max_context,
        grammar: params.grammar.clone(),
        provider: params.provider.clone(),
        system: params.system.clone(),
        history: params.history.clone(),
    };

    let inference_handle = tokio::spawn(async move {
        infer_with_params_streaming(
            &prompt_to_use_clone,
            &params_clone,
            move |token| {
                let _ = tx_clone.try_send(token);
            }
        ).await
    });

    drop(tx);

    while let Some(token) = rx.recv().await {
        let parts: Vec<&str> = token.split('\n').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                print!("\n  │  ");
            }
            print!("{}", style(part).color256(153));
        }
        std::io::stdout().flush()?;
    }

    // Stop guardian thread
    stop_guardian.store(true, std::sync::atomic::Ordering::Relaxed);

    let result = inference_handle.await??;
    println!("\n{}", style("  └─ Done").dim());
    let (cost, saved) = uneven_core::infrastructure::llm::pricing::calculate_cost_and_saved(
        &result.model,
        result.prompt_tokens,
        result.completion_tokens,
    );
    println!(
        "{}",
        style(format!(
            "  [Tokens used: {} | Cost: ${:.5} | Saved: ${:.5}]",
            result.tokens_used, cost, saved
        )).dim()
    );
    println!();

    Ok(())
}
