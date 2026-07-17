use anyhow::Result;
use chrono::{DateTime, Local, Duration, Datelike};
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::llm::pricing::load_stats;
use crate::ui;

pub async fn run() -> Result<()> {
    let root = loader::find_project_root(None);
    let stats = load_stats(&root);

    let now = Local::now();
    let one_hour_ago = now - Duration::hours(1);

    let mut session_tokens = 0;
    let mut session_cost = 0.0;

    let mut today_tokens = 0;
    let mut today_cost = 0.0;

    let mut month_tokens = 0;
    let mut month_cost = 0.0;
    let mut month_saved = 0.0;
    let mut month_calls = 0;
    let mut model_usage: std::collections::HashMap<String, (usize, f64)> = std::collections::HashMap::new();

    for stat in &stats.history {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&stat.timestamp) {
            let local_dt = dt.with_timezone(&Local);
            let tokens = stat.prompt_tokens + stat.completion_tokens;

            // Session (last 1 hour)
            if local_dt >= one_hour_ago {
                session_tokens += tokens;
                session_cost += stat.cost;
            }

            // Today (local day)
            if local_dt.date_naive() == now.date_naive() {
                today_tokens += tokens;
                today_cost += stat.cost;
            }

            // Month (local month and year)
            if local_dt.year() == now.year() && local_dt.month() == now.month() {
                month_tokens += tokens;
                month_cost += stat.cost;
                month_saved += stat.saved;
                month_calls += 1;

                let entry = model_usage.entry(stat.model.clone()).or_insert((0, 0.0));
                entry.0 += tokens;
                entry.1 += stat.cost;
            }
        }
    }

    let limit_usd = stats.monthly_limit_usd;
    let percent = if limit_usd > 0.0 {
        (month_cost / limit_usd) * 100.0
    } else {
        0.0
    };

    let bar_width = 20;
    let filled = ((percent / 100.0) * bar_width as f64).round() as usize;
    let filled = filled.min(bar_width);
    let empty = bar_width - filled;

    let filled_str = "█".repeat(filled);
    let empty_str = "░".repeat(empty);
    let bar_colored = format!(
        "[{}{}]",
        ui::green(&filled_str),
        ui::dim(&empty_str)
    );

    println!();
    println!("  {}", ui::accent_bold("◈  TOKEN USAGE AND COSTS"));
    println!();

    fn format_num(n: usize) -> String {
        let s = n.to_string();
        let mut result = String::new();
        for (idx, c) in s.chars().rev().enumerate() {
            if idx > 0 && idx % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        result.chars().rev().collect()
    }

    println!("    {} {} tokens ({})", ui::pad("Current Session:", 18), ui::bright(&format_num(session_tokens)), ui::green(&format!("${:.4}", session_cost)));
    println!("    {} {} tokens ({})", ui::pad("Today:", 18), ui::bright(&format_num(today_tokens)), ui::green(&format!("${:.4}", today_cost)));
    println!(
        "    {} ${:.4} / ${:.2} limit ({} tokens)",
        ui::pad("Current Month:", 18),
        month_cost,
        limit_usd,
        format_num(month_tokens)
    );
    println!();
    println!("    {} {} of monthly budget limit used", bar_colored, ui::accent(&format!("{:.1}%", percent)));
    println!();
    println!(
        "    You saved {} this month running local/efficient models compared to GPT-5.5!",
        ui::green(&format!("${:.2}", month_saved))
    );
    println!();

    // --- CONSUMPTION INSIGHTS ---
    let avg_tokens = month_tokens.checked_div(month_calls).unwrap_or(0);
    let avg_cost = if month_calls > 0 { month_cost / month_calls as f64 } else { 0.0 };

    let mut model_list: Vec<(String, usize, f64)> = model_usage
        .into_iter()
        .map(|(model, (tokens, cost))| (model, tokens, cost))
        .collect();
    model_list.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.cmp(&a.1))
    });

    println!("  {}", ui::accent_bold("◈  CONSUMPTION INSIGHTS"));
    println!();
    println!("    {} {} tokens", ui::pad("Average Tokens/Call:", 22), ui::bright(&format_num(avg_tokens)));
    println!("    {} {}", ui::pad("Average Cost/Call:", 22), ui::green(&format!("${:.4}", avg_cost)));
    println!("    {} {}", ui::pad("Total Month Calls:", 22), ui::bright(&format_num(month_calls)));
    println!();

    if !model_list.is_empty() {
        println!("    Top Models by Cost:");
        for (idx, (model, tokens, cost)) in model_list.iter().enumerate() {
            let pct = if month_cost > 0.0 {
                (cost / month_cost) * 100.0
            } else {
                0.0
            };
            
            let cost_str = if *cost == 0.0 {
                format!("$0.0000 (0.0% - local/free, {} tokens)", format_num(*tokens))
            } else {
                format!("${:.4} ({:.1}%, {} tokens)", cost, pct, format_num(*tokens))
            };

            println!(
                "      {}. {} : {}",
                idx + 1,
                ui::bright(model),
                ui::green(&cost_str)
            );
        }
        println!();
    }

    Ok(())
}
