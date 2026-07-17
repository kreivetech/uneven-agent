use crate::UnevenError;
use std::process::Command;
use std::thread;

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub detected: bool,
    pub name: Option<String>,
    pub vram_total: Option<u32>, // in MB
    pub vram_free: Option<u32>,  // in MB
    pub use_gpu: bool,
    pub gpu_layers: u32,
}

#[derive(Debug, Clone)]
pub struct HardwareInfo {
    pub gpu: GpuInfo,
    pub cpu_cores: usize,
    pub ram_total_mb: u64,
    pub is_weak_system: bool,
    pub recommendation: String,
    pub recommended_model_id: String,
}

pub struct HardwareDetector;

impl Default for HardwareDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareDetector {
    pub fn new() -> Self {
        HardwareDetector
    }

    pub fn detect_system(&self) -> Result<HardwareInfo, UnevenError> {
        let gpu = self.detect_gpu();
        let cpu_cores = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let ram_total_mb = self.detect_ram();

        // Minimum for smooth local LLM: 8GB RAM, 4 cores
        let is_weak_system = ram_total_mb < 7500 || cpu_cores < 4;

        let mut recommended_model_id = "llama-3.2-1b-q4".to_string();

        if gpu.detected {
            if let Some(vram_free) = gpu.vram_free {
                if vram_free > 4000 {
                    recommended_model_id = "qwen-2.5-3b-q4".to_string();
                } else if vram_free > 2000 {
                    recommended_model_id = "deepseek-r1-1.5b-q8".to_string();
                } else {
                    recommended_model_id = "qwen-2.5-1.5b-q8".to_string();
                }
            }
        } else {
            // CPU-only logic
            if ram_total_mb > 12000 {
                recommended_model_id = "qwen-2.5-3b-q4".to_string();
            } else if ram_total_mb > 7500 {
                recommended_model_id = "deepseek-r1-1.5b-q8".to_string();
            } else if ram_total_mb > 4500 {
                recommended_model_id = "qwen-2.5-0.5b-q4".to_string();
            } else {
                recommended_model_id = "smollm2-135m-q4".to_string();
            }
        }

        let recommendation = if is_weak_system {
            "Low hardware detected. Local LLM might be slow. Consider Cloud providers.".to_string()
        } else {
            "Your system is well-equipped for Uneven AI.".to_string()
        };

        Ok(HardwareInfo {
            gpu,
            cpu_cores,
            ram_total_mb,
            is_weak_system,
            recommendation,
            recommended_model_id,
        })
    }

    pub fn detect_gpu(&self) -> GpuInfo {
        // Attempt using nvidia-smi
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total,memory.free",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let line = stdout.trim();
                if !line.is_empty() {
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 3 {
                        let name = parts[0].to_string();
                        let total = parts[1].parse::<u32>().unwrap_or(0);
                        let free = parts[2].parse::<u32>().unwrap_or(0);

                        tracing::info!(
                            "HardwareDetector: NVIDIA GPU detected: {} ({}MB VRAM)",
                            name,
                            total
                        );

                        let use_gpu = free > 1024;
                        let gpu_layers = if free > 4096 {
                            35
                        } else if free > 2048 {
                            32
                        } else if free > 1024 {
                            16
                        } else {
                            0
                        };

                        return GpuInfo {
                            detected: true,
                            name: Some(name),
                            vram_total: Some(total),
                            vram_free: Some(free),
                            use_gpu,
                            gpu_layers,
                        };
                    }
                }
            }
        }

        // Fallbacks: Check presence of NVIDIA card
        let os = std::env::consts::OS;
        if os == "windows" {
            if let Ok(output) = Command::new("wmic")
                .args(["path", "win32_VideoController", "get", "name"])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
                if stdout.contains("nvidia") {
                    tracing::debug!(
                        "HardwareDetector: NVIDIA card seen via wmic, but drivers might be missing."
                    );
                }
            }
        } else {
            // Linux/macOS
            if let Ok(output) = Command::new("sh")
                .arg("-c")
                .arg("lspci 2>/dev/null | grep -i vga || true")
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
                if stdout.contains("nvidia") {
                    tracing::debug!(
                        "HardwareDetector: NVIDIA card seen via lspci, but drivers might be missing."
                    );
                }
            }
        }

        tracing::debug!("HardwareDetector: No compatible NVIDIA GPU detected or drivers missing.");
        GpuInfo {
            detected: false,
            name: None,
            vram_total: None,
            vram_free: None,
            use_gpu: false,
            gpu_layers: 0,
        }
    }

    fn detect_ram(&self) -> u64 {
        let os = std::env::consts::OS;
        if os == "linux" {
            if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
                for line in content.lines() {
                    if line.starts_with("MemTotal:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            if let Ok(kb) = parts[1].parse::<u64>() {
                                return kb / 1024;
                            }
                        }
                    }
                }
            }
        } else if os == "macos" {
            if let Ok(output) = Command::new("sysctl")
                .args(["-n", "hw.memsize"])
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if let Ok(bytes) = stdout.parse::<u64>() {
                        return bytes / (1024 * 1024);
                    }
                }
            }
        } else if os == "windows" {
            if let Ok(output) = Command::new("wmic")
                .args(["ComputerSystem", "get", "TotalPhysicalMemory"])
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        let val = line.trim();
                        if !val.is_empty() && val.chars().all(|c| c.is_ascii_digit()) {
                            if let Ok(bytes) = val.parse::<u64>() {
                                return bytes / (1024 * 1024);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: 8 GB
        8192
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_detector_lifecycle() {
        let detector = HardwareDetector::new();
        let info = detector.detect_system().unwrap();
        assert!(info.cpu_cores > 0);
        assert!(info.ram_total_mb > 0);
        if info.gpu.detected {
            assert!(info.gpu.name.is_some());
            assert!(info.gpu.vram_total.is_some());
            assert!(info.gpu.vram_free.is_some());
        }
    }
}
