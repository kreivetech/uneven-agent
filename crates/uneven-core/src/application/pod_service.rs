use crate::application::think_service::{execute_thought_loop_with_system, ThoughtPhase};
use crate::domain::pod::{AgentPod, PodHandoffReport, PodStatus};
use crate::infrastructure::llm::history_manager;
use crate::infrastructure::patch::FileObject;
use crate::infrastructure::security::laws;
use crate::UnevenConfig;
use anyhow::{anyhow, Result};
use std::path::Path;

/// Outcome of a dedicated Pod macro-mission.
#[derive(Debug, Clone, PartialEq)]
pub struct PodMissionOutcome {
    pub handoff: PodHandoffReport,
    pub file_object: Option<FileObject>,
    pub summary_message: String,
}

/// Dispatches a high-level architectural mission to a specific Pod cluster.
///
/// 1. Locates the designated AgentPod in project pods (`.uneven/pods.json`).
/// 2. Sets its status to `Running` and persists state.
/// 3. Injects the Pod's strict domain law (`laws::get_pod_context`).
/// 4. Executes the reasoning/planning loop (`execute_thought_loop_with_system`).
/// 5. Persists the conversation turn into the Pod's disk session `{session_id}_{pod_id}.json`.
/// 6. Generates the structured `PodHandoffReport` with exported contracts and advisory notes.
/// 7. Updates the Pod in `.uneven/pods.json` (status = `Completed`, `completed_tasks += 1`, `last_handoff = Some`).
/// 8. Returns `PodMissionOutcome` so the Master Agent can deliver feedback and designate the next layer.
pub async fn dispatch_pod_mission<F>(
    pod_id_or_keyword: &str,
    mission: &str,
    config: &UnevenConfig,
    root: &Path,
    session_id: &str,
    mut progress_callback: Option<F>,
) -> Result<PodMissionOutcome>
where
    F: FnMut(ThoughtPhase) + Send + Sync,
{
    let mut pods = AgentPod::load_project_pods(root);
    if pods.is_empty() {
        pods = AgentPod::default_project_pods();
    }

    let search_term = pod_id_or_keyword.trim().to_lowercase();
    let pod_idx = pods
        .iter()
        .position(|p| {
            p.id.to_lowercase() == search_term
                || p.title.to_lowercase().contains(&search_term)
                || match &p.category {
                    crate::domain::pod::PodCategory::Database => {
                        search_term.contains("data")
                            || search_term.contains("db")
                            || search_term.contains("banco")
                            || search_term.contains("sql")
                    }
                    crate::domain::pod::PodCategory::Backend => {
                        search_term.contains("back")
                            || search_term.contains("api")
                            || search_term.contains("serv")
                    }
                    crate::domain::pod::PodCategory::Frontend => {
                        search_term.contains("front")
                            || search_term.contains("ui")
                            || search_term.contains("client")
                    }
                    crate::domain::pod::PodCategory::Auth => {
                        search_term.contains("auth") || search_term.contains("login")
                    }
                    crate::domain::pod::PodCategory::DevOps => {
                        search_term.contains("devops")
                            || search_term.contains("deploy")
                            || search_term.contains("ci")
                    }
                    crate::domain::pod::PodCategory::Custom(c) => {
                        c.to_lowercase().contains(&search_term)
                    }
                }
        })
        .ok_or_else(|| {
            anyhow!(
                "Pod '{}' not found among registered project pods.",
                pod_id_or_keyword
            )
        })?;

    // Mark as running
    pods[pod_idx].status = PodStatus::Running;
    let _ = AgentPod::save_project_pods(root, &pods);

    let pod = pods[pod_idx].clone();
    let pod_sess_key = format!("{}_{}", session_id, pod.id);
    let mut history_turns =
        history_manager::get_history_from_disk(&pod_sess_key, root).unwrap_or_default();

    // Compose strict system law for the Pod
    let pod_law = laws::get_pod_context("think", &pod);

    let max_retries = 2;
    let mut attempt = 0;
    let mut current_mission = mission.to_string();

    loop {
        attempt += 1;
        let cb = progress_callback.as_mut().map(|cb| move |p: ThoughtPhase| cb(p));

        let think_res = execute_thought_loop_with_system(
            &current_mission,
            &pod.target_files,
            config,
            root,
            None,
            None,
            cb,
            Some(&pod_law),
        )
        .await;

        match think_res {
            Ok(result) => {
                // Save generated file actions to disk as pending patch
                if let Some(fo) = result.file_object.as_ref().filter(|fo| !fo.patch.is_empty()) {
                    let _ = history_manager::save_pending_patch_to_disk(fo, session_id, root);
                    let _ = history_manager::save_pending_patch_to_disk(fo, "default", root);
                }

                // Build structured handoff report
                let handoff = PodHandoffReport::from_response(
                    &pod,
                    mission,
                    &result.final_response,
                    result.file_object.as_ref(),
                );

                // Update Pod metadata in .uneven/pods.json
                let mut updated_pods = AgentPod::load_project_pods(root);
                if let Some(target) = updated_pods.iter_mut().find(|p| p.id == pod.id) {
                    target.status = PodStatus::Completed;
                    target.completed_tasks = (target.completed_tasks + 1).min(target.total_tasks.max(1));
                    target.last_handoff = Some(handoff.clone());
                    let _ = AgentPod::save_project_pods(root, &updated_pods);
                }

                // Persist the conversation turn in the pod's disk session
                history_turns.push(history_manager::BrainTurn {
                    role: "user".to_string(),
                    content: mission.to_string(),
                });
                history_turns.push(history_manager::BrainTurn {
                    role: "assistant".to_string(),
                    content: result.final_response.clone(),
                });
                let _ = history_manager::save_history_to_disk(&history_turns, &pod_sess_key, root);

                let summary_message = handoff.to_executive_summary();

                return Ok(PodMissionOutcome {
                    handoff,
                    file_object: result.file_object,
                    summary_message,
                });
            }
            Err(e) => {
                let err_msg = e.to_string();
                if attempt <= max_retries {
                    // Supervisor auto-healing: reformulate mission with specific corrective instructions
                    current_mission = format!(
                        "{}\n\n[SUPERVISION FEEDBACK - AUTO-HEALING ATTEMPT {}/{}]:\n\
                        The previous attempt encountered an issue: {}\n\
                        Root Cause Resolution Required:\n\
                        - Fix this specific error.\n\
                        - Ensure all code blocks and diffs are syntactically valid and strictly in English.\n\
                        - Do not introduce unrequested changes.",
                        mission, attempt, max_retries, err_msg
                    );
                    continue;
                } else {
                    let mut updated_pods = AgentPod::load_project_pods(root);
                    if let Some(target) = updated_pods.iter_mut().find(|p| p.id == pod.id) {
                        target.status = PodStatus::Failed;
                        let _ = AgentPod::save_project_pods(root, &updated_pods);
                    }
                    return Err(anyhow!(
                        "Pod '{}' failed after {} attempts. Last error: {}",
                        pod.title,
                        attempt,
                        err_msg
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_pod_search_by_category() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let pods = AgentPod::default_project_pods();
        AgentPod::save_project_pods(root, &pods).unwrap();

        let loaded = AgentPod::load_project_pods(root);
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].id, "pod-database");
    }
}
