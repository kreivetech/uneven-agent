use chrono::{DateTime, Datelike, Duration, Local};
use serde::{Deserialize, Serialize};
use crate::infrastructure::llm::pricing::{load_global_stats, SessionStats};

/// Internal benchmark base price in USD used to calibrate wholesale token limits.
/// (e.g. $110.00 / month reference tier)
pub const WHOLESALE_BENCHMARK_USD: f64 = 110.0;

/// Default calibrated token limits derived from wholesale plan pricing:
/// Input rate (~$3/M) + Output rate (~$15/M) weighted 75/25 -> ~$6/M tokens average.
/// At $110/mo baseline:
/// - Monthly capacity: ~5,000,000 tokens
/// - Weekly capacity: ~1,250,000 tokens
/// - 5-Hour shift capacity: ~400,000 tokens
pub const DEFAULT_MONTHLY_TOKEN_CAP: usize = 5_000_000;
pub const DEFAULT_WEEKLY_TOKEN_CAP: usize = 1_250_000;
pub const DEFAULT_TURN_5H_TOKEN_CAP: usize = 400_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenQuotaMetrics {
    // 5-Hour Turn Sliding Window
    pub turn_5h_tokens: usize,
    pub turn_5h_limit: usize,
    pub turn_5h_percent: f64,

    // Weekly Sliding Window (7 Days)
    pub weekly_tokens: usize,
    pub weekly_limit: usize,
    pub weekly_percent: f64,

    // Monthly Quota Window (Current Calendar Month)
    pub monthly_tokens: usize,
    pub monthly_limit: usize,
    pub monthly_percent: f64,

    // Percentage of tokens/cost saved through optimization (local inference, caching, efficient routing)
    pub optimization_savings_percent: f64,

    // Timing & Reset Information
    pub turn_remaining_minutes: usize,
    pub month_days_remaining: usize,
}

pub struct TokenQuotaService;

impl TokenQuotaService {
    /// Loads global usage stats from the binary root and computes token quota metrics.
    pub fn get_quota_metrics() -> TokenQuotaMetrics {
        let stats = load_global_stats();
        Self::compute_from_stats(&stats)
    }

    /// Computes token quota metrics from a given `SessionStats` reference.
    pub fn compute_from_stats(stats: &SessionStats) -> TokenQuotaMetrics {
        let now = Local::now();
        let five_hours_ago = now - Duration::hours(5);
        let seven_days_ago = now - Duration::days(7);

        let mut turn_5h_tokens: usize = 0;
        let mut weekly_tokens: usize = 0;
        let mut monthly_tokens: usize = 0;
        let mut monthly_cost: f64 = 0.0;
        let mut monthly_saved: f64 = 0.0;
        let mut oldest_in_turn: Option<DateTime<Local>> = None;

        for stat in &stats.history {
            if let Ok(dt) = DateTime::parse_from_rfc3339(&stat.timestamp) {
                let local_dt = dt.with_timezone(&Local);
                let tokens = stat.prompt_tokens + stat.completion_tokens;

                // 5-Hour Turn Sliding Window
                if local_dt >= five_hours_ago {
                    turn_5h_tokens += tokens;
                    if oldest_in_turn.is_none() || Some(local_dt) < oldest_in_turn {
                        oldest_in_turn = Some(local_dt);
                    }
                }

                // Weekly Sliding Window (7 days)
                if local_dt >= seven_days_ago {
                    weekly_tokens += tokens;
                }

                // Current Calendar Month
                if local_dt.year() == now.year() && local_dt.month() == now.month() {
                    monthly_tokens += tokens;
                    monthly_cost += stat.cost;
                    monthly_saved += stat.saved;
                }
            }
        }

        // Derive monthly token cap: if user configured a monthly_limit_usd, scale proportionally
        let monthly_limit = if stats.monthly_limit_usd > 0.0 && stats.monthly_limit_usd != 10.0 {
            let scale = stats.monthly_limit_usd / WHOLESALE_BENCHMARK_USD;
            ((DEFAULT_MONTHLY_TOKEN_CAP as f64) * scale).round() as usize
        } else if stats.monthly_limit > 0 && stats.monthly_limit != 1_000_000 {
            stats.monthly_limit
        } else {
            DEFAULT_MONTHLY_TOKEN_CAP
        };

        let weekly_limit = (monthly_limit / 4).max(1);
        let turn_5h_limit = ((weekly_limit as f64) * 0.32).round() as usize; // ~400k for 1.25M weekly

        let turn_5h_percent = if turn_5h_limit > 0 {
            ((turn_5h_tokens as f64 / turn_5h_limit as f64) * 100.0).min(100.0)
        } else {
            0.0
        };

        let weekly_percent = if weekly_limit > 0 {
            ((weekly_tokens as f64 / weekly_limit as f64) * 100.0).min(100.0)
        } else {
            0.0
        };

        let monthly_percent = if monthly_limit > 0 {
            ((monthly_tokens as f64 / monthly_limit as f64) * 100.0).min(100.0)
        } else {
            0.0
        };

        // Savings percentage compared to flagship baseline
        let baseline = monthly_cost + monthly_saved;
        let optimization_savings_percent = if baseline > 0.0 {
            ((monthly_saved / baseline) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        // Calculate time remaining in 5h window
        let turn_remaining_minutes = if let Some(oldest) = oldest_in_turn {
            let elapsed_turn = (now - oldest).num_minutes().max(0) as usize;
            300usize.saturating_sub(elapsed_turn)
        } else {
            300
        };

        // Calculate days remaining in current month
        let month_days_remaining = {
            let days_in_month: usize = match now.month() {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 => {
                    let y = now.year();
                    if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
                        29
                    } else {
                        28
                    }
                }
                _ => 30,
            };
            days_in_month.saturating_sub(now.day() as usize)
        };

        TokenQuotaMetrics {
            turn_5h_tokens,
            turn_5h_limit,
            turn_5h_percent,
            weekly_tokens,
            weekly_limit,
            weekly_percent,
            monthly_tokens,
            monthly_limit,
            monthly_percent,
            optimization_savings_percent,
            turn_remaining_minutes,
            month_days_remaining,
        }
    }

    /// Records token usage globally in the binary root stats file.
    pub fn record_usage(model: &str, prompt_tokens: usize, completion_tokens: usize) -> (f64, f64) {
        crate::infrastructure::llm::pricing::register_global_call(model, prompt_tokens, completion_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::llm::pricing::CallStat;

    #[test]
    fn test_compute_from_empty_stats() {
        let stats = SessionStats::default();
        let metrics = TokenQuotaService::compute_from_stats(&stats);

        assert_eq!(metrics.turn_5h_tokens, 0);
        assert_eq!(metrics.turn_5h_percent, 0.0);
        assert_eq!(metrics.weekly_tokens, 0);
        assert_eq!(metrics.weekly_percent, 0.0);
        assert_eq!(metrics.monthly_tokens, 0);
        assert_eq!(metrics.monthly_percent, 0.0);
        assert!(metrics.turn_5h_limit > 0);
        assert!(metrics.weekly_limit > 0);
        assert!(metrics.monthly_limit > 0);
    }

    #[test]
    fn test_compute_with_recent_calls() {
        let now = Local::now();
        let mut stats = SessionStats::default();
        stats.history.push(CallStat {
            timestamp: now.to_rfc3339(),
            model: "claude-3-5-sonnet".to_string(),
            prompt_tokens: 10_000,
            completion_tokens: 2_000,
            cost: 0.06,
            saved: 0.12,
        });

        let metrics = TokenQuotaService::compute_from_stats(&stats);
        assert_eq!(metrics.turn_5h_tokens, 12_000);
        assert_eq!(metrics.weekly_tokens, 12_000);
        assert_eq!(metrics.monthly_tokens, 12_000);
        assert!(metrics.turn_5h_percent > 0.0);
        assert!(metrics.optimization_savings_percent > 0.0);
    }
}
