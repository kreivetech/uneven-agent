//! Local LLM inference module
//!
//! Powered by `infercraft`.

pub use infercraft::*;

pub async fn load_model(model_name: &str, threads: u32) -> Result<(), crate::UnevenError> {
    infercraft::load_model(model_name, threads)
        .await
        .map_err(Into::into)
}

pub async fn run_inference(
    prompt: &str,
    max_tokens: usize,
    temperature: f64,
    top_p: Option<f64>,
    repeat_penalty: f32,
    max_context: usize,
    grammar: Option<String>,
) -> Result<(String, usize), crate::UnevenError> {
    infercraft::run_inference(
        prompt,
        max_tokens,
        temperature,
        top_p,
        repeat_penalty,
        max_context,
        grammar,
    )
    .await
    .map_err(Into::into)
}

pub async fn run_inference_streaming(
    prompt: &str,
    max_tokens: usize,
    temperature: f64,
    top_p: Option<f64>,
    repeat_penalty: f32,
    max_context: usize,
    grammar: Option<String>,
    on_token: impl Fn(String) + Send + Sync + 'static,
) -> Result<(String, usize), crate::UnevenError> {
    infercraft::run_inference_streaming(
        prompt,
        max_tokens,
        temperature,
        top_p,
        repeat_penalty,
        max_context,
        grammar,
        on_token,
    )
    .await
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auto_unload_trigger() {
        MODEL_LOADED.store(true, std::sync::atomic::Ordering::Release);
        {
            let mut current = LOADED_MODEL.write().await;
            *current = Some("test-model".to_string());
        }
        unload_model().await;
        assert!(!is_model_loaded());
        assert!(loaded_model_name().is_none());
    }
}
