//! LLM provider routing and bridge to llmcraft
//!
//! Routes requests to local inference (Candle) or delegates to `llmcraft`
//! for cloud and remote LLM providers (OpenAI, Claude, Gemini, Ollama, DeepSeek, Mistral, Groq).

use crate::UnevenError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    Local,
    OpenAi,
    Claude,
    Gemini,
    Ollama,
    DeepSeek,
    Mistral,
    Groq,
}

impl Provider {
    pub fn from_string(s: &str) -> Result<Self, UnevenError> {
        match s {
            "local" => Ok(Provider::Local),
            "openai" => Ok(Provider::OpenAi),
            "claude" => Ok(Provider::Claude),
            "gemini" => Ok(Provider::Gemini),
            "ollama" => Ok(Provider::Ollama),
            "deepseek" => Ok(Provider::DeepSeek),
            "mistral" => Ok(Provider::Mistral),
            "groq" => Ok(Provider::Groq),
            _ => Err(UnevenError::LlmError(format!("Unknown provider: {}", s))),
        }
    }

    pub fn to_llmcraft_provider(&self) -> Result<llmcraft::Provider, UnevenError> {
        match self {
            Provider::OpenAi => Ok(llmcraft::Provider::OpenAi),
            Provider::Claude => Ok(llmcraft::Provider::Claude),
            Provider::Gemini => Ok(llmcraft::Provider::Gemini),
            Provider::Ollama => Ok(llmcraft::Provider::Ollama),
            Provider::DeepSeek => Ok(llmcraft::Provider::DeepSeek),
            Provider::Mistral => Ok(llmcraft::Provider::Mistral),
            Provider::Groq => Ok(llmcraft::Provider::Groq),
            Provider::Local => Err(UnevenError::LlmError(
                "Local provider is not routed through external client".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderResponse {
    pub content: String,
    pub prompt_tokens: Option<usize>,
    pub completion_tokens: Option<usize>,
    pub remaining_requests: Option<usize>,
    pub remaining_tokens: Option<usize>,
    pub limit_requests: Option<usize>,
    pub limit_tokens: Option<usize>,
    pub reset_requests: Option<f64>,
    pub reset_tokens: Option<f64>,
}

fn get_active_reasoning_effort() -> Option<String> {
    let project_root = crate::infrastructure::config::loader::find_project_root(None);
    if let Ok(config) = crate::infrastructure::config::loader::load_config(&project_root) {
        config.brain.reasoning_effort
    } else {
        None
    }
}

fn build_llmcraft_params(
    prompt: &str,
    system: Option<&str>,
    history: Option<&[(String, String)]>,
    max_tokens: usize,
    temperature: f64,
) -> llmcraft::CompletionParams {
    let mut params = llmcraft::CompletionParams::new(prompt)
        .with_max_tokens(max_tokens)
        .with_temperature(temperature);

    if let Some(sys) = system {
        params = params.with_system(sys);
    }

    if let Some(hist) = history {
        params = params.with_history_tuples(hist);
    }

    if let Some(effort) = get_active_reasoning_effort() {
        params = params.with_reasoning_effort(effort);
    }

    params
}

pub async fn route_inference(
    provider: Provider,
    model: &str,
    prompt: &str,
    system: Option<&str>,
    history: Option<&[(String, String)]>,
    max_tokens: usize,
    temperature: f64,
) -> Result<ProviderResponse, UnevenError> {
    match provider {
        Provider::Local => {
            let (text, _) = super::inference::run_inference(
                prompt,
                max_tokens,
                temperature,
                None,
                1.1,
                2048,
                None,
            )
            .await?;
            Ok(ProviderResponse {
                content: text,
                ..Default::default()
            })
        }
        ref external => {
            let craft_provider = external.to_llmcraft_provider()?;
            let client = llmcraft::LlmClient::builder(craft_provider)
                .model(model)
                .build()
                .map_err(|e| UnevenError::LlmError(e.to_string()))?;

            let params = build_llmcraft_params(prompt, system, history, max_tokens, temperature);
            let res = client
                .complete_with_params(params)
                .await
                .map_err(|e| UnevenError::LlmError(e.to_string()))?;

            Ok(ProviderResponse {
                content: res.content,
                prompt_tokens: res.usage.as_ref().map(|u| u.prompt_tokens),
                completion_tokens: res.usage.as_ref().map(|u| u.completion_tokens),
                remaining_requests: res.rate_limits.remaining_requests,
                remaining_tokens: res.rate_limits.remaining_tokens,
                limit_requests: res.rate_limits.limit_requests,
                limit_tokens: res.rate_limits.limit_tokens,
                reset_requests: res.rate_limits.reset_requests,
                reset_tokens: res.rate_limits.reset_tokens,
            })
        }
    }
}

pub async fn route_inference_streaming<F>(
    provider: Provider,
    model: &str,
    prompt: &str,
    system: Option<&str>,
    history: Option<&[(String, String)]>,
    max_tokens: usize,
    temperature: f64,
    on_token: F,
) -> Result<ProviderResponse, UnevenError>
where
    F: Fn(String) + Send + Sync + 'static,
{
    match provider {
        Provider::Local => {
            let (text, _) = super::inference::run_inference_streaming(
                prompt,
                max_tokens,
                temperature,
                None,
                1.1,
                2048,
                None,
                on_token,
            )
            .await?;
            Ok(ProviderResponse {
                content: text,
                ..Default::default()
            })
        }
        ref external => {
            let craft_provider = external.to_llmcraft_provider()?;
            let client = llmcraft::LlmClient::builder(craft_provider)
                .model(model)
                .build()
                .map_err(|e| UnevenError::LlmError(e.to_string()))?;

            let params = build_llmcraft_params(prompt, system, history, max_tokens, temperature);
            let res = client
                .stream_with_params(params, on_token)
                .await
                .map_err(|e| UnevenError::LlmError(e.to_string()))?;

            Ok(ProviderResponse {
                content: res.content,
                prompt_tokens: res.usage.as_ref().map(|u| u.prompt_tokens),
                completion_tokens: res.usage.as_ref().map(|u| u.completion_tokens),
                remaining_requests: res.rate_limits.remaining_requests,
                remaining_tokens: res.rate_limits.remaining_tokens,
                limit_requests: res.rate_limits.limit_requests,
                limit_tokens: res.rate_limits.limit_tokens,
                reset_requests: res.rate_limits.reset_requests,
                reset_tokens: res.rate_limits.reset_tokens,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_valid_providers_parse() {
        for s in &[
            "local", "openai", "claude", "gemini", "ollama", "deepseek", "mistral", "groq",
        ] {
            assert!(Provider::from_string(s).is_ok(), "failed for: {s}");
        }
    }

    #[test]
    fn unknown_provider_returns_err() {
        assert!(Provider::from_string("mistral-unknown").is_err());
        assert!(Provider::from_string("").is_err());
    }
}
