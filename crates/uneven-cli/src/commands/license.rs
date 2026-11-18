use anyhow::{anyhow, Result};
use console::style;
use uneven_core::application::license_service::{read_license, LicenseService};
pub use uneven_core::application::license_service::{read_usage, UsageData};

pub fn check_pro() -> bool {
    LicenseService::check_pro()
}

pub fn require_pro() -> Result<()> {
    LicenseService::require_pro().map_err(|e| anyhow!("{}", e))
}

pub fn check_team() -> bool {
    LicenseService::check_team()
}

pub fn check_and_increment_usage(command: &str) -> Result<usize> {
    LicenseService::check_and_increment_usage(command).map_err(|e| anyhow!("{}", e))
}

pub async fn run_activate(key: String) -> Result<()> {
    let cache = LicenseService::activate(&key)
        .await
        .map_err(|e| anyhow!("{}", e))?;

    println!(
        "\n  {} {}\n",
        style("✓").green().bold(),
        style(format!(
            "License activated. Tier: {} · {} machine(s).",
            cache.tier.to_uppercase(),
            cache.seats
        ))
        .bold()
    );

    Ok(())
}

pub async fn run_status() -> Result<()> {
    if let Some(license) = read_license() {
        let now = chrono::Utc::now().timestamp();
        let active =
            license.expires_at == 0 || now < license.expires_at || now < license.grace_until;

        let status_str = if active {
            style("ACTIVE").green().bold()
        } else {
            style("EXPIRED").red().bold()
        };

        println!();
        println!("  {}", style("Uneven License Status").bold());
        println!("  {}", style("─".repeat(40)).dim());
        println!("  Status     : {}", status_str);
        println!(
            "  Tier       : {}",
            style(license.tier.to_uppercase()).color256(208).bold()
        );
        println!("  Seats      : {}", license.seats);

        if license.expires_at == 0 {
            println!("  Expires    : Never");
        } else {
            let exp_date = chrono::DateTime::from_timestamp(license.expires_at, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "N/A".to_string());
            println!("  Expires    : {}", exp_date);
        }

        println!("  Machine ID : {}", style(license.machine_id).dim());
        println!();
    } else {
        println!(
            "\n  {} {}\n",
            style("ℹ").blue().bold(),
            style("No active license. Running on Free tier.").dim()
        );
    }
    Ok(())
}

pub async fn run_deactivate() -> Result<()> {
    if LicenseService::deactivate()
        .await
        .map_err(|e| anyhow!("{}", e))?
    {
        println!(
            "\n  {} {}\n",
            style("✓").green().bold(),
            style("License deactivated locally.").bold()
        );
    } else {
        println!("\n  No active license to deactivate.\n");
    }
    Ok(())
}
