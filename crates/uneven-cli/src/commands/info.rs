use anyhow::Result;
use console::style;
use std::io::Write;
use uneven_core::infrastructure::utils::hardware_detector::HardwareDetector;

pub async fn run() -> Result<()> {
    println!(
        "\n{}",
        style("🛰️  UNEVEN AGENT — SYSTEM DIAGNOSTICS").cyan().bold()
    );
    println!();

    let version = env!("CARGO_PKG_VERSION");
    println!(
        "  {} {}",
        style("Engine Version :").white(),
        style(version).green()
    );
    println!(
        "  {} {}",
        style("CLI Version    :").white(),
        style(version).green()
    );

    print!("  {} Scanning...", style("Hardware Scan  :").white());
    std::io::stdout().flush()?;

    let detector = HardwareDetector::new();
    let sys_info = detector.detect_system()?;

    // Clear the "Scanning..." text
    // On Unix we can print carriage return and spaces, or using console's Term
    if let Some(_term) = console::Term::stdout().size_checked() {
        print!("\r");
        // Clear the current line by printing spaces (rough estimate of line length)
        print!("{}", " ".repeat(40));
        print!("\r");
        std::io::stdout().flush()?;
    } else {
        println!();
    }

    if sys_info.gpu.detected {
        if let Some(ref name) = sys_info.gpu.name {
            println!(
                "  {} {}",
                style("GPU Detected   :").white(),
                style(name).color256(220)
            ); // Gold
        }
        if let Some(total) = sys_info.gpu.vram_total {
            println!(
                "  {} {} MB",
                style("VRAM Total     :").white(),
                style(total).cyan()
            );
        }
        if let Some(free) = sys_info.gpu.vram_free {
            println!(
                "  {} {} MB",
                style("VRAM Free      :").white(),
                style(free).cyan()
            );
        }
        println!(
            "  {} {}",
            style("Acceleration   :").white(),
            style("⚡ ACTIVE (CUDA/GPU)").green().bold()
        );
        println!(
            "  {} {}",
            style("Auto-Layers    :").white(),
            style(sys_info.gpu.gpu_layers).color256(153)
        ); // Light blue/cyan
    } else {
        println!(
            "  {} {}",
            style("GPU Status     :").white(),
            style("Not found or drivers missing").yellow()
        );
        println!(
            "  {} {}",
            style("Acceleration   :").white(),
            style("CPU Only (AVX/NEON optimized)").dim()
        );
    }

    println!("\n{}", style("---").dim());
    println!(
        "{}",
        style("Uneven Agent is optimized for your hardware.").dim()
    );
    let root = uneven_core::infrastructure::config::loader::find_project_root(None);
    let config_path =
        uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
    println!(
        "{}",
        style(format!("Configuration: {}\n", config_path.display())).dim()
    );

    Ok(())
}
