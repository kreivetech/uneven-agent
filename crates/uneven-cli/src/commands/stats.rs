use anyhow::Result;
use uneven_core::application::TokenQuotaService;
use crate::ui;

fn render_progress_bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);

    let filled_str = "█".repeat(filled);
    let empty_str = "░".repeat(empty);
    format!("[{}{}]", ui::green(&filled_str), ui::dim(&empty_str))
}

fn format_token_compact(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub async fn run() -> Result<()> {
    let metrics = TokenQuotaService::get_quota_metrics();

    println!();
    println!("  {}", ui::accent_bold("◈  TOKEN USAGE & QUOTAS"));
    println!();

    let bar_width = 20;

    // 5-Hour Shift / Turn
    let turn_bar = render_progress_bar(metrics.turn_5h_percent, bar_width);
    let turn_detail = format!(
        "{} / {} tokens",
        format_token_compact(metrics.turn_5h_tokens),
        format_token_compact(metrics.turn_5h_limit)
    );
    println!(
        "    {} {} {:>5.1}%  ({})",
        ui::pad("5h Work Turn:", 16),
        turn_bar,
        metrics.turn_5h_percent,
        ui::dim(&turn_detail)
    );

    // Weekly Window
    let weekly_bar = render_progress_bar(metrics.weekly_percent, bar_width);
    let weekly_detail = format!(
        "{} / {} tokens",
        format_token_compact(metrics.weekly_tokens),
        format_token_compact(metrics.weekly_limit)
    );
    println!(
        "    {} {} {:>5.1}%  ({})",
        ui::pad("Weekly Limit:", 16),
        weekly_bar,
        metrics.weekly_percent,
        ui::dim(&weekly_detail)
    );

    // Monthly Quota
    let monthly_bar = render_progress_bar(metrics.monthly_percent, bar_width);
    let monthly_detail = format!(
        "{} / {} tokens",
        format_token_compact(metrics.monthly_tokens),
        format_token_compact(metrics.monthly_limit)
    );
    println!(
        "    {} {} {:>5.1}%  ({})",
        ui::pad("Monthly Limit:", 16),
        monthly_bar,
        metrics.monthly_percent,
        ui::dim(&monthly_detail)
    );

    println!();
    println!("  {}", ui::accent_bold("◈  QUOTA STATUS & EFFICIENCY"));
    println!();

    let turn_hours = metrics.turn_remaining_minutes / 60;
    let turn_mins = metrics.turn_remaining_minutes % 60;
    let reset_str = if turn_hours > 0 {
        format!("In {}h {}m", turn_hours, turn_mins)
    } else {
        format!("In {}m", turn_mins)
    };

    println!("    {} {}", ui::pad("Active Turn Reset:", 22), ui::bright(&reset_str));
    println!(
        "    {} {}",
        ui::pad("Monthly Cycle:", 22),
        ui::bright(&format!("{} days remaining", metrics.month_days_remaining))
    );

    if metrics.optimization_savings_percent > 0.0 {
        println!(
            "    {} {}",
            ui::pad("Optimization:", 22),
            ui::green(&format!("+{:.1}% saved through smart routing & caching", metrics.optimization_savings_percent))
        );
    } else {
        println!(
            "    {} {}",
            ui::pad("Quota State:", 22),
            ui::green("Optimal")
        );
    }
    println!();

    Ok(())
}
