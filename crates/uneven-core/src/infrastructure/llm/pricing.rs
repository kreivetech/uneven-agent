use std::path::Path;

// Re-export core telemetry types and calculations from llmcraft
pub use llmcraft::pricing::{
    calculate_cost_and_saved, get_comparison_rates, get_model_rates,
    get_model_rates_by_provider_and_model, record_call_stat, resolve_rate,
    CallStat, ModelCostRule, SessionStats, COST_TABLE,
};

pub fn load_global_stats() -> SessionStats {
    let stats_path = crate::infrastructure::utils::paths::global_stats_path();
    llmcraft::pricing::load_stats_from_path(&stats_path)
}

pub fn save_global_stats(stats: &SessionStats) -> Result<(), std::io::Error> {
    let stats_path = crate::infrastructure::utils::paths::global_stats_path();
    llmcraft::pricing::save_stats_to_path(&stats_path, stats)
}

pub fn load_stats(_project_root: &Path) -> SessionStats {
    load_global_stats()
}

pub fn save_stats(_project_root: &Path, stats: &SessionStats) -> Result<(), std::io::Error> {
    save_global_stats(stats)
}

pub fn register_call(
    _project_root: &Path,
    model: &str,
    prompt_tokens: usize,
    completion_tokens: usize,
) -> (f64, f64) {
    register_global_call(model, prompt_tokens, completion_tokens)
}

pub fn register_global_call(
    model: &str,
    prompt_tokens: usize,
    completion_tokens: usize,
) -> (f64, f64) {
    let mut stats = load_global_stats();
    let (cost, saved) = llmcraft::pricing::record_call_stat(
        &mut stats,
        model,
        prompt_tokens,
        completion_tokens,
    );
    let _ = save_global_stats(&stats);
    (cost, saved)
}
