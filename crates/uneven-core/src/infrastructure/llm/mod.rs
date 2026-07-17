//! LLM orchestrator module
//!
//! Handles local and external LLM inference, embedding generation,
//! and routing between different providers.

pub mod inference;
pub mod embeddings;
pub mod providers;
pub mod pricing;
pub mod history_manager;

use crate::UnevenError;
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;

pub static LLM_POOL: Lazy<std::sync::RwLock<Option<rayon::ThreadPool>>> =
    Lazy::new(|| std::sync::RwLock::new(None));

pub fn run_in_llm_pool<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    let guard = LLM_POOL.read().unwrap();
    if let Some(ref pool) = *guard {
        pool.install(f)
    } else {
        f()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub tokens_used: usize,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_requests: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_requests: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_requests: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_tokens: Option<f64>,
}

/// Parameters for a top-level inference call.
///
/// Passed by reference to `infer_with_params` and `infer_with_params_streaming`
/// to avoid exceeding clippy's `too_many_arguments` limit and to make call
/// sites self-documenting.
pub struct InferParams {
    pub model:          String,
    pub threads:        u32,
    pub max_tokens:     usize,
    pub temperature:    f64,
    pub repeat_penalty: f32,
    pub max_context:    usize,
    pub grammar:        Option<String>,
    pub provider:       Option<String>,
    pub system:         Option<String>,
    pub history:        Option<Vec<(String, String)>>,
}

/// Initialize LLM module.
///
/// Only initializes the embedding model and the main local model when `provider == "local"`.
/// For API providers (claude, openai, gemini, ollama) local models (including BERT embeddings)
/// are never loaded, keeping RAM usage extremely low.
pub async fn init(provider: &str, model: &str, threads: u32) -> Result<(), UnevenError> {
    let mut n = threads.max(1);
    let detector = crate::infrastructure::utils::hardware_detector::HardwareDetector::new();
    if let Ok(info) = detector.detect_system() {
        if info.is_weak_system {
            n = n.min(3);
        }
    }
    std::env::set_var("RAYON_NUM_THREADS", n.to_string());

    // Initialize custom LLM thread pool with the dynamic threads count
    match rayon::ThreadPoolBuilder::new().num_threads(n as usize).build() {
        Ok(pool) => {
            let mut guard = LLM_POOL.write().unwrap();
            *guard = Some(pool);
            tracing::info!("LLM: custom thread pool initialized with {} threads", n);
        }
        Err(e) => {
            tracing::error!("LLM: failed to build custom thread pool: {:?}", e);
        }
    }

    tracing::info!("Initializing LLM module (provider={}, model={}, threads={})", provider, model, n);

    if provider == "local" {
        embeddings::init_embeddings().await?;
        match inference::load_model(model, n).await {
            Ok(()) => tracing::info!("LLM: local model '{}' loaded", model),
            Err(UnevenError::LlmError(ref msg)) if msg.contains("not found") => {
                tracing::warn!(
                    "LLM: model '{}' not found — run `uneven init` to download it.", model
                );
            }
            Err(e) => return Err(e),
        }
    } else {
        inference::unload_model().await;
        tracing::info!("LLM: provider='{}' — local model not loaded", provider);
    }

    Ok(())
}

/// Unload the local model from memory (e.g. after switching to an API provider)
pub async fn unload_local_model() {
    inference::unload_model().await;
}

/// Return true if local inference is available (model loaded)
pub fn is_local_inference_ready() -> bool {
    inference::is_model_loaded()
}

/// Return true if real BERT embeddings are active
pub fn is_real_embeddings_ready() -> bool {
    embeddings::is_real_model_loaded()
}

/// Generate embedding for text ({EMBEDDING_DIM}-dim)
pub async fn embed(text: &str) -> Result<Vec<f32>, UnevenError> {
    tracing::debug!("Embedding: text len={}", text.len());
    embeddings::generate_embedding(text).await
}

/// Batch generate embeddings for multiple texts
pub async fn batch_embed(texts: Vec<&str>) -> Result<Vec<Vec<f32>>, UnevenError> {
    tracing::info!("Batch embedding {} texts", texts.len());
    embeddings::batch_embed(texts).await
}

/// Run inference on LLM with conservative defaults.
///
/// Returns LlmResponse with generated content.
pub async fn infer(prompt: &str) -> Result<LlmResponse, UnevenError> {
    tracing::info!("Running LLM inference (prompt len={})", prompt.len());

    let (content, tokens_used) = inference::run_inference(
        prompt,
        512,
        0.0,        // greedy — deterministic
        None,       // top_p off
        1.1,        // mild repetition penalty
        2048,       // 2 k context window
        None,
    ).await?;

    let model = inference::loaded_model_name()
        .unwrap_or_else(|| "local".to_string());
    let prompt_tokens = (prompt.len() as f64 / 4.0).ceil() as usize;

    let project_root = crate::infrastructure::config::loader::find_project_root(None);
    let _ = pricing::register_call(&project_root, &model, prompt_tokens, tokens_used);

    Ok(LlmResponse {
        content,
        prompt_tokens,
        completion_tokens: tokens_used,
        tokens_used: prompt_tokens + tokens_used,
        model,
        remaining_requests: None,
        remaining_tokens: None,
        limit_requests: None,
        limit_tokens: None,
        reset_requests: None,
        reset_tokens: None,
    })
}

/// Run inference with custom parameters.
///
/// * `temperature`    — 0.0 = greedy; 0.7 = mild sampling
/// * `repeat_penalty` — 1.0 = off; 1.1 = recommended default
/// * `max_context`    — context-window cap (tokens); 2048 is a safe default
pub async fn infer_with_params(
    prompt: &str,
    params: &InferParams,
) -> Result<LlmResponse, UnevenError> {
    let provider_str = params.provider.clone().unwrap_or_else(|| "local".to_string());
    let provider = providers::Provider::from_string(&provider_str)?;

    if provider != providers::Provider::Local {
        let resp = providers::route_inference(
            provider,
            &params.model,
            prompt,
            params.system.as_deref(),
            params.history.as_deref(),
            params.max_tokens,
            params.temperature,
        ).await?;
        let system_len = params.system.as_ref().map_or(0, |s| s.len());
        let prompt_tokens = resp.prompt_tokens.unwrap_or_else(|| ((prompt.len() + system_len) as f64 / 4.0).ceil() as usize);
        let completion_tokens = resp.completion_tokens.unwrap_or_else(|| (resp.content.len() as f64 / 4.0).ceil() as usize);

        let project_root = crate::infrastructure::config::loader::find_project_root(None);
        let _ = pricing::register_call(&project_root, &params.model, prompt_tokens, completion_tokens);

        return Ok(LlmResponse {
            content: resp.content,
            prompt_tokens,
            completion_tokens,
            tokens_used: prompt_tokens + completion_tokens,
            model: params.model.clone(),
            remaining_requests: resp.remaining_requests,
            remaining_tokens: resp.remaining_tokens,
            limit_requests: resp.limit_requests,
            limit_tokens: resp.limit_tokens,
            reset_requests: resp.reset_requests,
            reset_tokens: resp.reset_tokens,
        });
    }

    tracing::info!(
        "Running LLM inference with params: model={}, max_tokens={}, \
         threads={}, temperature={}, repeat_penalty={}, max_context={}",
        params.model, params.max_tokens, params.threads,
        params.temperature, params.repeat_penalty, params.max_context
    );

    // Load model if different from current — warn if not found
    match inference::load_model(&params.model, params.threads).await {
        Ok(()) => {}
        Err(UnevenError::LlmError(ref msg)) if msg.contains("not found") => {
            tracing::warn!("Model '{}' not found — using stub inference", params.model);
        }
        Err(e) => return Err(e),
    }

    let (mut content, tokens_used) = inference::run_inference(
        prompt, params.max_tokens,
        params.temperature, None,
        params.repeat_penalty, params.max_context,
        params.grammar.clone(),
    ).await?;

    if params.model.to_lowercase().contains("deepseek") {
        content = inference::chat_template::strip_thinking_tokens(&content);
    }

    let system_len = params.system.as_ref().map_or(0, |s| s.len());
    let prompt_tokens = ((prompt.len() + system_len) as f64 / 4.0).ceil() as usize;

    let project_root = crate::infrastructure::config::loader::find_project_root(None);
    let _ = pricing::register_call(&project_root, &params.model, prompt_tokens, tokens_used);

    Ok(LlmResponse {
        content,
        prompt_tokens,
        completion_tokens: tokens_used,
        tokens_used: prompt_tokens + tokens_used,
        model: params.model.clone(),
        remaining_requests: None,
        remaining_tokens: None,
        limit_requests: None,
        limit_tokens: None,
        reset_requests: None,
        reset_tokens: None,
    })
}

/// Run inference with custom parameters and per-token streaming callback.
///
/// Identical to `infer_with_params` except `on_token` is called for each
/// incremental piece of text as it is produced. Returns a complete
/// `LlmResponse` with the full text and actual tokens generated.
pub async fn infer_with_params_streaming(
    prompt: &str,
    params: &InferParams,
    on_token: impl Fn(String) + Send + Sync + 'static,
) -> Result<LlmResponse, UnevenError> {
    let provider_str = params.provider.clone().unwrap_or_else(|| "local".to_string());
    let provider = providers::Provider::from_string(&provider_str)?;

    if provider != providers::Provider::Local {
        let resp = providers::route_inference_streaming(
            provider,
            &params.model,
            prompt,
            params.system.as_deref(),
            params.history.as_deref(),
            params.max_tokens,
            params.temperature,
            on_token,
        ).await?;
        let system_len = params.system.as_ref().map_or(0, |s| s.len());
        let prompt_tokens = resp.prompt_tokens.unwrap_or_else(|| ((prompt.len() + system_len) as f64 / 4.0).ceil() as usize);
        let completion_tokens = resp.completion_tokens.unwrap_or_else(|| (resp.content.len() as f64 / 4.0).ceil() as usize);

        let project_root = crate::infrastructure::config::loader::find_project_root(None);
        let _ = pricing::register_call(&project_root, &params.model, prompt_tokens, completion_tokens);

        return Ok(LlmResponse {
            content: resp.content,
            prompt_tokens,
            completion_tokens,
            tokens_used: prompt_tokens + completion_tokens,
            model: params.model.clone(),
            remaining_requests: resp.remaining_requests,
            remaining_tokens: resp.remaining_tokens,
            limit_requests: resp.limit_requests,
            limit_tokens: resp.limit_tokens,
            reset_requests: resp.reset_requests,
            reset_tokens: resp.reset_tokens,
        });
    }

    tracing::info!(
        "Running streaming LLM inference: model={}, max_tokens={}, temperature={}",
        params.model, params.max_tokens, params.temperature
    );

    match inference::load_model(&params.model, params.threads).await {
        Ok(()) => {}
        Err(UnevenError::LlmError(ref msg)) if msg.contains("not found") => {
            tracing::warn!("Model '{}' not found — streaming inference unavailable", params.model);
        }
        Err(e) => return Err(e),
    }

    let (mut content, tokens_used) = inference::run_inference_streaming(
        prompt, params.max_tokens,
        params.temperature, None,
        params.repeat_penalty, params.max_context,
        params.grammar.clone(),
        on_token,
    ).await?;

    if params.model.to_lowercase().contains("deepseek") {
        content = inference::chat_template::strip_thinking_tokens(&content);
    }

    let system_len = params.system.as_ref().map_or(0, |s| s.len());
    let prompt_tokens = ((prompt.len() + system_len) as f64 / 4.0).ceil() as usize;

    let project_root = crate::infrastructure::config::loader::find_project_root(None);
    let _ = pricing::register_call(&project_root, &params.model, prompt_tokens, tokens_used);

    Ok(LlmResponse {
        content,
        prompt_tokens,
        completion_tokens: tokens_used,
        tokens_used: prompt_tokens + tokens_used,
        model: params.model.clone(),
        remaining_requests: None,
        remaining_tokens: None,
        limit_requests: None,
        limit_tokens: None,
        reset_requests: None,
        reset_tokens: None,
    })
}

/// Request an interrupt of the current local inference
pub fn interrupt_local_inference() {
    inference::interrupt();
}

/// Reset the interrupt flag
pub fn reset_local_interrupt() {
    inference::reset_interrupt();
}
