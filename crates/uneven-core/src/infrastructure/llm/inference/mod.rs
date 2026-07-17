use crate::UnevenError;
use once_cell::sync::{Lazy, OnceCell};
use std::sync::atomic::{AtomicBool, Ordering, AtomicU64};
use tokio::sync::RwLock;
use std::time::{Instant, Duration};
use tokio::time::sleep;

static LAST_ACTIVITY: Lazy<std::sync::Mutex<Option<Instant>>> =
    Lazy::new(|| std::sync::Mutex::new(None));

static UNLOAD_TASK_SPAWNED: Lazy<AtomicBool> =
    Lazy::new(|| AtomicBool::new(false));

static UNLOAD_TIMEOUT_SECS: Lazy<AtomicU64> =
    Lazy::new(|| AtomicU64::new(300));

static RAYON_POOL_INIT: OnceCell<()> = OnceCell::new();

fn init_thread_pool(threads: u32) {
    RAYON_POOL_INIT.get_or_init(|| {
        let detector = crate::infrastructure::utils::hardware_detector::HardwareDetector::new();
        let mut n = threads.max(1) as usize;
        if let Ok(info) = detector.detect_system() {
            if info.is_weak_system {
                n = n.min(3);
                tracing::info!("Inference: low-resource system detected. Throttling Rayon threads to {}", n);
            }
        }
        match rayon::ThreadPoolBuilder::new().num_threads(n).build_global() {
            Ok(_) => tracing::info!("Inference: Rayon thread pool limited to {} threads", n),
            Err(e) => tracing::warn!("Inference: could not configure Rayon thread pool: {e}"),
        }
    });
}

pub mod types;
pub mod device;
pub mod loader;
pub mod engine;
pub mod grammar;
pub mod chat_template;

pub use types::GenerateParams;
pub use loader::LlamaState;

#[cfg(feature = "llm")]
pub static LLAMA: Lazy<tokio::sync::Mutex<Option<LlamaState<loader::AnyModel>>>> =
    Lazy::new(|| tokio::sync::Mutex::new(None));

pub static LOADED_MODEL: Lazy<RwLock<Option<String>>> =
    Lazy::new(|| RwLock::new(None));

pub static INTERRUPT: Lazy<AtomicBool> =
    Lazy::new(|| AtomicBool::new(false));

pub static MODEL_LOADED: Lazy<AtomicBool> =
    Lazy::new(|| AtomicBool::new(false));

pub async fn load_model(model_name: &str, threads: u32) -> Result<(), UnevenError> {
    init_thread_pool(threads);
    // Quick read-only check before acquiring the write lock.
    {
        let current = LOADED_MODEL.read().await;
        if current.as_deref() == Some(model_name) {
            return Ok(());
        }
    }

    #[cfg(feature = "llm")]
    {
        // Acquire LLAMA lock first so concurrent load_model calls are serialized.
        // This eliminates the TOCTOU between the read check above and the write below.
        let mut guard = LLAMA.lock().await;

        // Re-check under the lock: another task may have loaded while we waited.
        {
            let current = LOADED_MODEL.read().await;
            if current.as_deref() == Some(model_name) {
                return Ok(());
            }
        }

        // Model loading is synchronous and CPU/disk-bound — use block_in_place so
        // the tokio executor is not blocked while we read the GGUF file.
        let model_name_owned = model_name.to_string();
        let state = tokio::task::block_in_place(|| {
            crate::infrastructure::llm::run_in_llm_pool(|| {
                loader::try_load(&model_name_owned)
            })
        })
        .map_err(|e| UnevenError::LlmError(format!("Failed to load model: {e}")))?;

        let device_name = state.device_name.clone();
        *guard = Some(state);
        drop(guard);

        let mut current = LOADED_MODEL.write().await;
        *current = Some(model_name.to_string());
        MODEL_LOADED.store(true, Ordering::Release);
        tracing::info!("Inference: model '{model_name}' ready on {device_name}");

        let root = crate::infrastructure::config::loader::find_project_root(None);
        let timeout_mins = crate::infrastructure::config::loader::load_config(&root)
            .ok()
            .and_then(|c| c.brain.local.as_ref().and_then(|l| l.unload_timeout))
            .unwrap_or(5); // Default 5 minutes
        
        let timeout_secs = timeout_mins * 60;
        UNLOAD_TIMEOUT_SECS.store(timeout_secs, Ordering::Release);
        
        {
            if let Ok(mut act) = LAST_ACTIVITY.lock() {
                *act = Some(Instant::now());
            }
        }

        if timeout_secs > 0 && !UNLOAD_TASK_SPAWNED.swap(true, Ordering::SeqCst) {
            tokio::spawn(async {
                loop {
                    sleep(Duration::from_secs(1)).await;
                    
                    if !MODEL_LOADED.load(Ordering::Acquire) {
                        UNLOAD_TASK_SPAWNED.store(false, Ordering::Release);
                        break;
                    }

                    let last_act = {
                        match LAST_ACTIVITY.lock() {
                            Ok(act) => *act,
                            Err(_) => None,
                        }
                    };

                    if let Some(last) = last_act {
                        let timeout_secs = UNLOAD_TIMEOUT_SECS.load(Ordering::Acquire);
                        if timeout_secs > 0 && last.elapsed() >= Duration::from_secs(timeout_secs) {
                            tracing::info!(
                                "Inference: model auto-unload triggered due to {} seconds of inactivity",
                                timeout_secs
                            );
                            unload_model().await;
                            UNLOAD_TASK_SPAWNED.store(false, Ordering::Release);
                            break;
                        }
                    }
                }
            });
        }
    }
    #[cfg(not(feature = "llm"))]
    return Err(UnevenError::LlmError("LLM feature disabled".into()));

    Ok(())
}

#[cfg(feature = "llm")]
async fn cpu_fallback<F>(run: F) -> Result<(String, usize), UnevenError>
where
    F: FnOnce(&mut LlamaState<loader::AnyModel>) -> anyhow::Result<(String, usize)> + Send,
{
    let model_name = LOADED_MODEL.read().await.clone().unwrap_or_default();
    tracing::warn!("Inference: OOM on GPU, falling back to CPU for '{model_name}'");

    // Model loading is synchronous and CPU/disk-bound — block_in_place prevents
    // stalling the tokio executor during the fallback GGUF read.
    let mut cpu_state = tokio::task::block_in_place(|| {
        crate::infrastructure::llm::run_in_llm_pool(|| {
            loader::try_load_on_device(&model_name, candle_core::Device::Cpu)
        })
    })
    .map_err(|e| UnevenError::OutOfMemory(format!("CPU fallback load failed: {e}")))?;

    let retry = tokio::task::block_in_place(|| {
        crate::infrastructure::llm::run_in_llm_pool(|| {
            run(&mut cpu_state)
        })
    });

    let mut guard = LLAMA.lock().await;
    *guard = Some(cpu_state);
    retry.map_err(|e| UnevenError::LlmError(format!("CPU fallback failed: {e}")))
}

pub async fn run_inference(
    prompt: &str,
    max_tokens: usize,
    temperature: f64,
    top_p: Option<f64>,
    repeat_penalty: f32,
    max_context: usize,
    grammar: Option<String>,
) -> Result<(String, usize), UnevenError> {
    INTERRUPT.store(false, Ordering::SeqCst);
    let params = GenerateParams { max_tokens, temperature, top_p, repeat_penalty, max_context, grammar };

    #[cfg(feature = "llm")]
    {
        let mut guard = LLAMA.lock().await;
        let Some(state) = guard.as_mut() else {
            return Err(UnevenError::ModelNotFound("Model not loaded".into()));
        };

        let result = tokio::task::block_in_place(|| {
            crate::infrastructure::llm::run_in_llm_pool(|| {
                engine::generate(state, prompt, &params)
            })
        });
        if let Ok(mut act) = LAST_ACTIVITY.lock() {
            *act = Some(Instant::now());
        }

        match result {
            Ok(r) => Ok(r),
            Err(e) if device::is_oom(&e) => {
                *guard = None;
                drop(guard);
                cpu_fallback(|s| engine::generate(s, prompt, &params)).await
            }
            Err(e) => Err(UnevenError::LlmError(format!("Generation failed: {e}"))),
        }
    }
    #[cfg(not(feature = "llm"))]
    Err(UnevenError::LlmError("LLM feature disabled".into()))
}

#[allow(clippy::too_many_arguments)]
pub async fn run_inference_streaming(
    prompt: &str,
    max_tokens: usize,
    temperature: f64,
    top_p: Option<f64>,
    repeat_penalty: f32,
    max_context: usize,
    grammar: Option<String>,
    on_token: impl Fn(String) + Send + Sync + 'static,
) -> Result<(String, usize), UnevenError> {
    INTERRUPT.store(false, Ordering::SeqCst);
    let params = GenerateParams { max_tokens, temperature, top_p, repeat_penalty, max_context, grammar };

    #[cfg(feature = "llm")]
    {
        let mut guard = LLAMA.lock().await;
        let Some(state) = guard.as_mut() else {
            return Err(UnevenError::ModelNotFound("Model not loaded".into()));
        };

        let result = tokio::task::block_in_place(|| {
            crate::infrastructure::llm::run_in_llm_pool(|| {
                engine::generate_streaming(state, prompt, &params, &on_token, &INTERRUPT)
            })
        });
        if let Ok(mut act) = LAST_ACTIVITY.lock() {
            *act = Some(Instant::now());
        }

        match result {
            Ok(r) => Ok(r),
            Err(e) if device::is_oom(&e) => {
                *guard = None;
                drop(guard);
                cpu_fallback(|s| engine::generate_streaming(s, prompt, &params, &on_token, &INTERRUPT)).await
            }
            Err(_e) if INTERRUPT.load(Ordering::SeqCst) => Err(UnevenError::Interrupted),
            Err(e) => Err(UnevenError::LlmError(format!("Streaming failed: {e}"))),
        }
    }
    #[cfg(not(feature = "llm"))]
    Err(UnevenError::LlmError("LLM feature disabled".into()))
}

pub fn is_model_loaded() -> bool {
    MODEL_LOADED.load(Ordering::Acquire)
}

pub fn loaded_model_name() -> Option<String> {
    LOADED_MODEL.try_read().ok().and_then(|g| g.clone())
}

pub async fn unload_model() {
    #[cfg(feature = "llm")]
    {
        let mut guard = LLAMA.lock().await;
        *guard = None;
    }
    let mut current = LOADED_MODEL.write().await;
    *current = None;
    MODEL_LOADED.store(false, Ordering::Release);
}

pub fn interrupt() {
    INTERRUPT.store(true, Ordering::SeqCst);
}

pub fn reset_interrupt() {
    INTERRUPT.store(false, Ordering::SeqCst);
}

pub fn active_device_name() -> String {
    #[cfg(feature = "llm")]
    if let Ok(guard) = LLAMA.try_lock() {
        if let Some(state) = guard.as_ref() {
            return state.device_name.clone();
        }
    }
    if is_model_loaded() { "CPU".to_string() } else { "none".to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_llama_mutex_availability() {
        // Verify that the static mutex can be locked and updated
        let mut guard = LLAMA.lock().await;
        *guard = None; // Reset for test
        assert!(guard.is_none());
        drop(guard);

        // Verify it can be locked again (using lock to be resilient to parallel test execution)
        let _guard2 = LLAMA.lock().await;
    }

    #[tokio::test]
    async fn test_mutex_resilience_on_failure() {
        // Simulate a scenario where a "task" holds the lock 
        // This is a sanity check for tokio's RAII drop behavior in our engine context.
        {
            let _guard = LLAMA.lock().await;
            // Guard is held here
            assert!(LLAMA.try_lock().is_err()); 
        }
        
        // Guard should be dropped now
        let _guard = LLAMA.lock().await;
    }

    #[tokio::test]
    async fn test_auto_unload_trigger() {
        // Setup initial states
        MODEL_LOADED.store(true, Ordering::Release);
        {
            let mut current = LOADED_MODEL.write().await;
            *current = Some("test-model".to_string());
        }
        UNLOAD_TIMEOUT_SECS.store(1, Ordering::Release);
        {
            if let Ok(mut act) = LAST_ACTIVITY.lock() {
                *act = Some(Instant::now());
            }
        }
        
        // Spawn the task manually for this test if it's not spawned
        if !UNLOAD_TASK_SPAWNED.swap(true, Ordering::SeqCst) {
            tokio::spawn(async {
                loop {
                    sleep(Duration::from_secs(1)).await;
                    
                    if !MODEL_LOADED.load(Ordering::Acquire) {
                        UNLOAD_TASK_SPAWNED.store(false, Ordering::Release);
                        break;
                    }

                    let last_act = {
                        match LAST_ACTIVITY.lock() {
                            Ok(act) => *act,
                            Err(_) => None,
                        }
                    };

                    if let Some(last) = last_act {
                        let timeout_secs = UNLOAD_TIMEOUT_SECS.load(Ordering::Acquire);
                        if last.elapsed() >= Duration::from_secs(timeout_secs) {
                            unload_model().await;
                            UNLOAD_TASK_SPAWNED.store(false, Ordering::Release);
                            break;
                        }
                    }
                }
            });
        }

        // Wait for it to trigger
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

        // Verify model is unloaded
        assert!(!is_model_loaded());
        assert!(loaded_model_name().is_none());
    }
}
