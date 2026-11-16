//! Hardware detection module
//!
//! Powered by `infercraft`.

pub use infercraft::{GpuInfo, GpuKind, HardwareDetector, HardwareInfo};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_detector_lifecycle() {
        let detector = HardwareDetector::new();
        let info = detector.detect_system();
        assert!(info.is_ok());
        let info = info.unwrap();
        assert!(info.cpu_cores > 0);
        assert!(info.ram_total_mb > 0);
    }

    #[test]
    fn test_integrated_gpu_calculation() {
        let detector = HardwareDetector::new();
        let gpu = detector.detect_gpu();
        if gpu.detected && gpu.is_integrated {
            assert!(gpu.gpu_layers > 0);
        }
    }
}
