pub mod ask_service;
pub mod command_guard;
pub mod context_assembler;
pub mod context_compactor;
pub mod engine_service;
pub mod license_service;
pub mod model_routing;
pub mod multi_agent_service;
pub mod pentest_service;
pub mod pod_service;
pub mod scan_service;
pub mod shell_classifier;
pub mod think_service;
pub mod token_quota_service;
pub mod update_service;
pub mod zeroday_service;

pub use ask_service::{execute_ask, execute_ask_with_system, AskResult};
pub use command_guard::*;
pub use context_assembler::ContextAssembler;
pub use context_compactor::{CompactionOutcome, ContextCompactorService};
pub use engine_service::UnevenEngine;
pub use license_service::{LicenseCache, LicenseService};
pub use model_routing::{allocate_brains, match_model_string, parse_multi_brains};
pub use multi_agent_service::{
    execute_multi_agent_workflow, MultiAgentResult, SubtaskSpec, SupervisorEvent,
};
pub use pentest_service::{PentestResult, PentestService, PentestSummaryCounts};
pub use pod_service::{dispatch_pod_mission, PodMissionOutcome};
pub use scan_service::{
    CategoryGroup, DepVulnerability, ScanResult, ScanService, ScanSummary,
};
pub use shell_classifier::{
    classify_intent, classify_shell_intent, classify_with_brain, extract_file_path, json_to_intent,
    tokenize_shell_input, ClassifiedIntent, ShellAction,
};
pub use think_service::{
    execute_thought_loop, execute_thought_loop_with_history, execute_thought_loop_with_system, ThoughtPhase, ThoughtResult,
};
pub use token_quota_service::{TokenQuotaMetrics, TokenQuotaService};
pub use update_service::{IdeUpdateInfo, UpdateService};
pub use zeroday_service::{ZerodayResult, ZerodayService, ZerodaySummaryCounts};


