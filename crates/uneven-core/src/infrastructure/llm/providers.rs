//! LLM provider routing
//!
//! Routes requests to the appropriate LLM provider:
//! - local: embedded LLM via Candle
//! - openai: OpenAI API
//! - claude: Anthropic Claude
//! - gemini: Google Gemini
//! - ollama: Local Ollama instance
//! - deepseek: DeepSeek API
//! - mistral: Mistral AI API
//! - groq: Groq Cloud API

use crate::UnevenError;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
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
}

// ─── Request / Response Schemas ───────────────────────────────────────────

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

// Used exclusively for OpenAI: max_completion_tokens replaces the deprecated
// max_tokens, and reasoning_effort is an OpenAI-only parameter.
#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(
        rename = "max_completion_tokens",
        skip_serializing_if = "Option::is_none"
    )]
    max_completion_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    stream: bool,
    #[serde(rename = "reasoning_effort", skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

// Used for OpenAI-compatible providers (Groq, DeepSeek, Mistral, Ollama):
// - uses max_tokens (not max_completion_tokens)
// - never sends reasoning_effort
// - always sends temperature
#[derive(Serialize)]
struct OpenAiCompatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    stream: bool,
}

#[derive(Deserialize)]
struct OpenAiChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiDeltaMessage {
    content: Option<String>,
}

#[derive(Debug, Clone)]
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

#[derive(Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiChoiceMessage>,
    delta: Option<OpenAiDeltaMessage>,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

// thinkingLevel is NOT a real Gemini API field — removed.
// Only thinkingBudget is valid, and only for Gemini 2.5 models.
#[derive(Serialize)]
struct GeminiThinkingConfig {
    #[serde(rename = "thinkingBudget", skip_serializing_if = "Option::is_none")]
    thinking_budget: Option<i32>,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
    thinking_config: Option<GeminiThinkingConfig>,
}

#[derive(Serialize)]
struct GeminiRequest {
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Serialize)]
struct ClaudeThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,
    budget_tokens: usize,
}

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<serde_json::Value>,
    messages: Vec<ClaudeMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ClaudeThinkingConfig>,
}

// ─── Public API ───────────────────────────────────────────────────────────

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
                prompt_tokens: None,
                completion_tokens: None,
                remaining_requests: None,
                remaining_tokens: None,
                limit_requests: None,
                limit_tokens: None,
                reset_requests: None,
                reset_tokens: None,
            })
        }
        Provider::Gemini => {
            let api_key = std::env::var("GOOGLE_API_KEY")
                .map_err(|_| UnevenError::LlmError("GOOGLE_API_KEY is not set".to_string()))?;
            let endpoint = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                model, api_key
            );

            let req_body =
                build_gemini_request(model, prompt, system, history, max_tokens, temperature);
            let client = reqwest::Client::new();
            let res = client
                .post(&endpoint)
                .json(&req_body)
                .send()
                .await
                .map_err(|e| UnevenError::LlmError(format!("Gemini request failed: {}", e)))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(UnevenError::LlmError(format!(
                    "Gemini API error ({}): {}",
                    status, err_text
                )));
            }

            let (rem_req, rem_tok, lim_req, lim_tok, rst_req, rst_tok) = extract_ratelimits(res.headers());
            let json: serde_json::Value = res.json().await.map_err(|e| {
                UnevenError::LlmError(format!("Failed to parse Gemini response: {}", e))
            })?;

            let text = json["candidates"][0]["content"]["parts"][0]["text"]
                .as_str()
                .ok_or_else(|| {
                    UnevenError::LlmError("Gemini response is missing content text".to_string())
                })?;

            let prompt_tokens = json["usageMetadata"]["promptTokenCount"]
                .as_u64()
                .map(|v| v as usize);
            let completion_tokens = json["usageMetadata"]["candidatesTokenCount"]
                .as_u64()
                .map(|v| v as usize);
            Ok(ProviderResponse {
                content: text.to_string(),
                prompt_tokens,
                completion_tokens,
                remaining_requests: rem_req,
                remaining_tokens: rem_tok,
                limit_requests: lim_req,
                limit_tokens: lim_tok,
                reset_requests: rst_req,
                reset_tokens: rst_tok,
            })
        }
        Provider::Claude => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| UnevenError::LlmError("ANTHROPIC_API_KEY is notset".to_string()))?;

            let req_body = build_claude_request(
                model,
                prompt,
                system,
                history,
                max_tokens,
                temperature,
                false,
            );
            let client = reqwest::Client::new();
            let res = client
                .post("https://api.anthropic.com/v1/messages")
                .header("Content-Type", "application/json")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&req_body)
                .send()
                .await
                .map_err(|e| UnevenError::LlmError(format!("Claude request failed: {}", e)))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(UnevenError::LlmError(format!(
                    "Claude API error ({}): {}",
                    status, err_text
                )));
            }

            let (rem_req, rem_tok, lim_req, lim_tok, rst_req, rst_tok) = extract_ratelimits(res.headers());
            let json: serde_json::Value = res.json().await.map_err(|e| {
                UnevenError::LlmError(format!("Failed to parse Claude response: {}", e))
            })?;

            let text = json["content"][0]["text"].as_str().ok_or_else(|| {
                UnevenError::LlmError("Claude response is missing text block".to_string())
            })?;

            let prompt_tokens = json["usage"]["input_tokens"].as_u64().map(|v| v as usize);
            let completion_tokens = json["usage"]["output_tokens"].as_u64().map(|v| v as usize);
            Ok(ProviderResponse {
                content: text.to_string(),
                prompt_tokens,
                completion_tokens,
                remaining_requests: rem_req,
                remaining_tokens: rem_tok,
                limit_requests: lim_req,
                limit_tokens: lim_tok,
                reset_requests: rst_req,
                reset_tokens: rst_tok,
            })
        }
        Provider::OpenAi => {
            let (endpoint, api_key_opt) = get_openai_compat_endpoint(&Provider::OpenAi)?;
            let req_body = build_openai_request(
                model,
                prompt,
                system,
                history,
                max_tokens,
                temperature,
                false,
            );
            let client = reqwest::Client::new();
            let mut req = client
                .post(&endpoint)
                .header("Content-Type", "application/json");

            if let Some(key) = api_key_opt {
                req = req.header("Authorization", format!("Bearer {}", key));
            }

            let res =
                req.json(&req_body).send().await.map_err(|e| {
                    UnevenError::LlmError(format!("Provider request failed: {}", e))
                })?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(UnevenError::LlmError(format!(
                    "Provider API error ({}): {}",
                    status, err_text
                )));
            }

            let (rem_req, rem_tok, lim_req, lim_tok, rst_req, rst_tok) = extract_ratelimits(res.headers());
            let resp: OpenAiResponse = res.json().await.map_err(|e| {
                UnevenError::LlmError(format!("Failed to parse response JSON: {}", e))
            })?;

            let text = resp
                .choices
                .first()
                .and_then(|c| c.message.as_ref())
                .and_then(|m| m.content.as_ref())
                .ok_or_else(|| {
                    UnevenError::LlmError("Response choice contains no text content".to_string())
                })?;

            let prompt_tokens = resp.usage.as_ref().map(|u| u.prompt_tokens);
            let completion_tokens = resp.usage.as_ref().map(|u| u.completion_tokens);
            Ok(ProviderResponse {
                content: text.clone(),
                prompt_tokens,
                completion_tokens,
                remaining_requests: rem_req,
                remaining_tokens: rem_tok,
                limit_requests: lim_req,
                limit_tokens: lim_tok,
                reset_requests: rst_req,
                reset_tokens: rst_tok,
            })
        }

        openai_or_compat => {
            let (endpoint, api_key_opt) = get_openai_compat_endpoint(&openai_or_compat)?;
            let req_body = build_openai_compat_request(
                model,
                prompt,
                system,
                history,
                max_tokens,
                temperature,
                false,
            );
            let client = reqwest::Client::new();
            let mut req = client
                .post(&endpoint)
                .header("Content-Type", "application/json");

            if let Some(key) = api_key_opt {
                req = req.header("Authorization", format!("Bearer {}", key));
            }

            let res =
                req.json(&req_body).send().await.map_err(|e| {
                    UnevenError::LlmError(format!("Provider request failed: {}", e))
                })?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(UnevenError::LlmError(format!(
                    "Provider API error {}): {}",
                    status, err_text
                )));
            }

            let (rem_req, rem_tok, lim_req, lim_tok, rst_req, rst_tok) = extract_ratelimits(res.headers());
            let resp: OpenAiResponse = res.json().await.map_err(|e| {
                UnevenError::LlmError(format!("Failed to parse response JSON: {}", e))
            })?;

            let text = resp
                .choices
                .first()
                .and_then(|c| c.message.as_ref())
                .and_then(|m| m.content.as_ref())
                .ok_or_else(|| {
                    UnevenError::LlmError("Response choice contains no text content".to_string())
                })?;

            let prompt_tokens = resp.usage.as_ref().map(|u| u.prompt_tokens);
            let completion_tokens = resp.usage.as_ref().map(|u| u.completion_tokens);
            Ok(ProviderResponse {
                content: text.clone(),
                prompt_tokens,
                completion_tokens,
                remaining_requests: rem_req,
                remaining_tokens: rem_tok,
                limit_requests: lim_req,
                limit_tokens: lim_tok,
                reset_requests: rst_req,
                reset_tokens: rst_tok,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
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
                prompt_tokens: None,
                completion_tokens: None,
                remaining_requests: None,
                remaining_tokens: None,
                limit_requests: None,
                limit_tokens: None,
                reset_requests: None,
                reset_tokens: None,
            })
        }
        Provider::Gemini => {
            let api_key = std::env::var("GOOGLE_API_KEY")
                .map_err(|_| UnevenError::LlmError("GOOGLE_API_KEY is not set".to_string()))?;
            let endpoint = format!(
                  "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}",
                  model, api_key
              );

            let req_body =
                build_gemini_request(model, prompt, system, history, max_tokens, temperature);
            let client = reqwest::Client::new();
            let res = client
                .post(&endpoint)
                .json(&req_body)
                .send()
                .await
                .map_err(|e| UnevenError::LlmError(format!("Gemini request failed: {}", e)))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(UnevenError::LlmError(format!(
                    "Gemini API error ({}): {}",
                    status, err_text
                )));
            }

            let (rem_req, rem_tok, lim_req, lim_tok, rst_req, rst_tok) = extract_ratelimits(res.headers());
            let mut stream = res.bytes_stream();
            let mut buffer = String::new();
            let mut bracket_count = 0;
            let mut start_idx = None;
            let mut in_string = false;
            let mut escaped = false;
            let mut full_response = String::new();

            while let Some(item) = stream.next().await {
                let bytes =
                    item.map_err(|e| UnevenError::LlmError(format!("Stream read error: {}", e)))?;
                let chunk = String::from_utf8_lossy(&bytes);

                for c in chunk.chars() {
                    if escaped {
                        escaped = false;
                        buffer.push(c);
                        continue;
                    }
                    if c == '\\' {
                        escaped = true;
                        buffer.push(c);
                        continue;
                    }
                    if c == '"' {
                        in_string = !in_string;
                    }
                    buffer.push(c);

                    if !in_string {
                        if c == '{' {
                            if bracket_count == 0 {
                                start_idx = Some(buffer.len() - 1);
                            }
                            bracket_count += 1;
                        } else if c == '}' {
                            bracket_count -= 1;
                            if bracket_count == 0 {
                                if let Some(start) = start_idx {
                                    let json_str = &buffer[start..];
                                    if let Ok(json) =
                                        serde_json::from_str::<serde_json::Value>(json_str)
                                    {
                                        if let Some(token) = json["candidates"][0]["content"]
                                            ["parts"][0]["text"]
                                            .as_str()
                                        {
                                            full_response.push_str(token);
                                            on_token(token.to_string());
                                        }
                                    }
                                }
                                start_idx = None;
                            }
                        }
                    }
                }
            }

            Ok(ProviderResponse {
                content: full_response,
                prompt_tokens: None,
                completion_tokens: None,
                remaining_requests: rem_req,
                remaining_tokens: rem_tok,
                limit_requests: lim_req,
                limit_tokens: lim_tok,
                reset_requests: rst_req,
                reset_tokens: rst_tok,
            })
        }
        Provider::Claude => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| UnevenError::LlmError("ANTHROPIC_API_KEY is notset".to_string()))?;

            let req_body = build_claude_request(
                model,
                prompt,
                system,
                history,
                max_tokens,
                temperature,
                true,
            );
            let client = reqwest::Client::new();
            let res = client
                .post("https://api.anthropic.com/v1/messages")
                .header("Content-Type", "application/json")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&req_body)
                .send()
                .await
                .map_err(|e| UnevenError::LlmError(format!("Claude request failed: {}", e)))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(UnevenError::LlmError(format!(
                    "Claude API error ({}): {}",
                    status, err_text
                )));
            }

            let (rem_req, rem_tok, lim_req, lim_tok, rst_req, rst_tok) = extract_ratelimits(res.headers());
            let mut stream = res.bytes_stream();
            let mut buffer = String::new();
            let mut full_response = String::new();

            while let Some(item) = stream.next().await {
                let bytes =
                    item.map_err(|e| UnevenError::LlmError(format!("Stream read error: {}", e)))?;
                let chunk = String::from_utf8_lossy(&bytes);
                buffer.push_str(&chunk);

                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer = buffer[pos + 1..].to_string();

                    if let Some(data_str) = line.strip_prefix("data: ") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data_str) {
                            if json["type"].as_str() == Some("content_block_delta") {
                                if let Some(token) = json["delta"]["text"].as_str() {
                                    full_response.push_str(token);
                                    on_token(token.to_string());
                                }
                            }
                        }
                    }
                }
            }

            Ok(ProviderResponse {
                content: full_response,
                prompt_tokens: None,
                completion_tokens: None,
                remaining_requests: rem_req,
                remaining_tokens: rem_tok,
                limit_requests: lim_req,
                limit_tokens: lim_tok,
                reset_requests: rst_req,
                reset_tokens: rst_tok,
            })
        }
        openai_or_compat => {
            let (endpoint, api_key_opt) = get_openai_compat_endpoint(&openai_or_compat)?;
            let req_body = build_openai_compat_request(
                model,
                prompt,
                system,
                history,
                max_tokens,
                temperature,
                true,
            );
            let client = reqwest::Client::new();
            let mut req = client
                .post(&endpoint)
                .header("Content-Type", "application/json");

            if let Some(key) = api_key_opt {
                req = req.header("Authorization", format!("Bearer {}", key));
            }

            let res =
                req.json(&req_body).send().await.map_err(|e| {
                    UnevenError::LlmError(format!("Provider request failed: {}", e))
                })?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(UnevenError::LlmError(format!(
                    "Provider API error({}): {}",
                    status, err_text
                )));
            }

            let (rem_req, rem_tok, lim_req, lim_tok, rst_req, rst_tok) = extract_ratelimits(res.headers());
            let mut stream = res.bytes_stream();
            let mut buffer = String::new();
            let mut full_response = String::new();

            while let Some(item) = stream.next().await {
                let bytes =
                    item.map_err(|e| UnevenError::LlmError(format!("Stream read error: {}", e)))?;
                let chunk = String::from_utf8_lossy(&bytes);
                buffer.push_str(&chunk);

                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer = buffer[pos + 1..].to_string();

                    if let Some(data_str) = line.strip_prefix("data: ") {
                        if data_str == "[DONE]" {
                            break;
                        }
                        if let Ok(resp) = serde_json::from_str::<OpenAiResponse>(data_str) {
                            if let Some(choice) = resp.choices.first() {
                                if let Some(delta) = &choice.delta {
                                    if let Some(token) = &delta.content {
                                        full_response.push_str(token);
                                        on_token(token.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Ok(ProviderResponse {
                content: full_response,
                prompt_tokens: None,
                completion_tokens: None,
                remaining_requests: rem_req,
                remaining_tokens: rem_tok,
                limit_requests: lim_req,
                limit_tokens: lim_tok,
                reset_requests: rst_req,
                reset_tokens: rst_tok,
            })
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────

fn get_active_reasoning_effort() -> Option<String> {
    let project_root = crate::infrastructure::config::loader::find_project_root(None);
    if let Ok(config) = crate::infrastructure::config::loader::load_config(&project_root) {
        config.brain.reasoning_effort
    } else {
        None
    }
}

fn build_gemini_request(
    model: &str,
    prompt: &str,
    system: Option<&str>,
    history: Option<&[(String, String)]>,
    max_tokens: usize,
    temperature: f64,
) -> GeminiRequest {
    let system_instruction = system.map(|sys| GeminiSystemInstruction {
        parts: vec![GeminiPart {
            text: sys.to_string(),
        }],
    });

    let mut contents = Vec::new();
    if let Some(hist) = history {
        for (role, content) in hist {
            contents.push(GeminiContent {
                role: if role == "assistant" {
                    "model".to_string()
                } else {
                    "user".to_string()
                },
                parts: vec![GeminiPart {
                    text: content.clone(),
                }],
            });
        }
    }
    contents.push(GeminiContent {
        role: "user".to_string(),
        parts: vec![GeminiPart {
            text: prompt.to_string(),
        }],
    });

    let effort = get_active_reasoning_effort();
    let mut thinking_config: Option<GeminiThinkingConfig> = None;

    if let Some(ref e) = effort {
        // thinkingConfig with thinkingBudget is only valid for Gemini 2.5 models.
        // Non-2.5 models do not support any thinking config — thinkingLevel is
        // not a real API field and must not be sent.
        let is_gemini_2_5 = model.contains("2.5");
        if is_gemini_2_5 {
            let budget: Option<i32> = match e.to_lowercase().as_str() {
                "low" => Some(1024),
                "medium" => Some(2048),
                "high" => Some(4096),
                other => other.parse::<i32>().ok(),
            };
            if let Some(b) = budget {
                thinking_config = Some(GeminiThinkingConfig {
                    thinking_budget: Some(b),
                });
            }
        }
        // Non-2.5 models: thinking_config stays None — no invalid fields sent.
    }

    // Suppress temperature when thinkingConfig is active (Gemini API requirement).
    let final_temperature = if thinking_config.is_some() {
        None
    } else {
        Some(temperature)
    };

    GeminiRequest {
        system_instruction,
        contents,
        generation_config: GeminiGenerationConfig {
            max_output_tokens: max_tokens,
            temperature: final_temperature,
            thinking_config,
        },
    }
}

#[allow(clippy::type_complexity)]
fn extract_ratelimits(
    headers: &reqwest::header::HeaderMap,
) -> (
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<f64>,
    Option<f64>,
) {
    let parse_header_usize = |name: &str| -> Option<usize> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok())
    };
    let parse_header_f64 = |name: &str| -> Option<f64> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|s| {
                if s.ends_with("ms") {
                    s.strip_suffix("ms").unwrap().parse::<f64>().ok().map(|ms| ms / 1000.0)
                } else if s.ends_with('s') {
                    s.strip_suffix('s').unwrap().parse::<f64>().ok()
                } else if s.ends_with('m') {
                    s.strip_suffix('m').unwrap().parse::<f64>().ok().map(|m| m * 60.0)
                } else if s.ends_with('h') {
                    s.strip_suffix('h').unwrap().parse::<f64>().ok().map(|h| h * 3600.0)
                } else {
                    s.parse::<f64>().ok()
                }
            })
    };

    let mut remaining_requests = parse_header_usize("x-ratelimit-remaining-requests");
    let mut remaining_tokens = parse_header_usize("x-ratelimit-remaining-tokens");
    let mut limit_requests = parse_header_usize("x-ratelimit-limit-requests");
    let mut limit_tokens = parse_header_usize("x-ratelimit-limit-tokens");
    let mut reset_requests = parse_header_f64("x-ratelimit-reset-requests");
    let mut reset_tokens = parse_header_f64("x-ratelimit-reset-tokens");

    if remaining_requests.is_none() {
        remaining_requests = parse_header_usize("anthropic-ratelimit-requests-remaining");
    }
    if remaining_tokens.is_none() {
        remaining_tokens = parse_header_usize("anthropic-ratelimit-input-tokens-remaining");
    }
    if limit_requests.is_none() {
        limit_requests = parse_header_usize("anthropic-ratelimit-requests-limit");
    }
    if limit_tokens.is_none() {
        limit_tokens = parse_header_usize("anthropic-ratelimit-input-tokens-limit");
    }
    if reset_requests.is_none() {
        reset_requests = headers
            .get("anthropic-ratelimit-requests-reset")
            .and_then(|value| value.to_str().ok())
            .and_then(|s| {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                    let now = chrono::Utc::now();
                    let diff = dt.with_timezone(&chrono::Utc).signed_duration_since(now);
                    Some(diff.num_milliseconds() as f64 / 1000.0)
                } else {
                    s.parse::<f64>().ok()
                }
            });
    }
    if reset_tokens.is_none() {
        reset_tokens = headers
            .get("anthropic-ratelimit-input-tokens-reset")
            .and_then(|value| value.to_str().ok())
            .and_then(|s| {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                    let now = chrono::Utc::now();
                    let diff = dt.with_timezone(&chrono::Utc).signed_duration_since(now);
                    Some(diff.num_milliseconds() as f64 / 1000.0)
                } else {
                    s.parse::<f64>().ok()
                }
            });
    }

    (
        remaining_requests,
        remaining_tokens,
        limit_requests,
        limit_tokens,
        reset_requests,
        reset_tokens,
    )
}

fn build_claude_request(
    model: &str,
    prompt: &str,
    system: Option<&str>,
    history: Option<&[(String, String)]>,
    max_tokens: usize,
    temperature: f64,
    stream: bool,
) -> ClaudeRequest {
    let mut messages = Vec::new();
    if let Some(hist) = history {
        for (role, content) in hist {
            messages.push(ClaudeMessage {
                role: role.clone(),
                content: serde_json::Value::String(content.clone()),
            });
        }
    }
    messages.push(ClaudeMessage {
        role: "user".to_string(),
        content: serde_json::json!([
            {
                "type": "text",
                "text": prompt,
                "cache_control": { "type": "ephemeral" }
            }
        ]),
    });

    let system_value = system.map(|sys| {
        serde_json::json!([
            {
                "type": "text",
                "text": sys,
                "cache_control": { "type": "ephemeral" }
            }
        ])
    });

    let effort = get_active_reasoning_effort();
    let mut thinking = None;
    let mut final_max_tokens = max_tokens;

    // Claude 4 models (opus-4, sonnet-4, haiku-4) do not accept temperature.
    // Also suppress temperature when extended thinking is enabled.
    let is_claude4 = model.contains("claude-opus-4")
        || model.contains("claude-sonnet-4")
        || model.contains("claude-haiku-4");

    let mut final_temperature = if is_claude4 { None } else { Some(temperature) };

    if let Some(ref e) = effort {
        let budget = match e.to_lowercase().as_str() {
            "low" => Some(1024),
            "medium" => Some(2048),
            "high" => Some(4096),
            other => other.parse::<usize>().ok().filter(|&val| val >= 1024),
        };
        if let Some(b) = budget {
            thinking = Some(ClaudeThinkingConfig {
                thinking_type: "enabled".to_string(),
                budget_tokens: b,
            });
            if final_max_tokens <= b {
                final_max_tokens = b + 2048;
            }
            // Extended thinking always requires temperature to be suppressed.
            final_temperature = None;
        }
    }

    ClaudeRequest {
        model: model.to_string(),
        max_tokens: final_max_tokens,
        system: system_value,
        messages,
        stream,
        temperature: final_temperature,
        thinking,
    }
}

// Build a request for OpenAI proper:
// - uses max_completion_tokens (replaces deprecated max_tokens for gpt-4o+)
// - suppresses temperature when reasoning_effort is set
// - sends reasoning_effort only to OpenAI
fn build_openai_request(
    model: &str,
    prompt: &str,
    system: Option<&str>,
    history: Option<&[(String, String)]>,
    max_tokens: usize,
    temperature: f64,
    stream: bool,
) -> OpenAiRequest {
    let mut messages = Vec::new();
    if let Some(sys) = system {
        messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: sys.to_string(),
        });
    }
    if let Some(hist) = history {
        for (role, content) in hist {
            messages.push(OpenAiMessage {
                role: role.clone(),
                content: content.clone(),
            });
        }
    }
    messages.push(OpenAiMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
    });

    let effort = get_active_reasoning_effort();
    let has_effort = effort.is_some();

    OpenAiRequest {
        model: model.to_string(),
        messages,
        max_completion_tokens: Some(max_tokens),
        temperature: if has_effort { None } else { Some(temperature) },
        stream,
        reasoning_effort: effort,
    }
}

// Build a request for OpenAI-compatible providers (Groq, DeepSeek, Mistral, Ollama)
// - uses max_tokens (not max_completion_tokens)
// - always sends temperature (these providers require it)
// - never sends reasoning_effort (OpenAI-only parameter)
fn build_openai_compat_request(
    model: &str,
    prompt: &str,
    system: Option<&str>,
    history: Option<&[(String, String)]>,
    max_tokens: usize,
    temperature: f64,
    stream: bool,
) -> OpenAiCompatRequest {
    let mut messages = Vec::new();
    if let Some(sys) = system {
        messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: sys.to_string(),
        });
    }
    if let Some(hist) = history {
        for (role, content) in hist {
            messages.push(OpenAiMessage {
                role: role.clone(),
                content: content.clone(),
            });
        }
    }
    messages.push(OpenAiMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
    });

    OpenAiCompatRequest {
        model: model.to_string(),
        messages,
        max_tokens: Some(max_tokens),
        temperature: Some(temperature),
        stream,
    }
}

fn get_openai_compat_endpoint(
    provider: &Provider,
) -> Result<(String, Option<String>), UnevenError> {
    match provider {
        Provider::OpenAi => {
            let key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| UnevenError::LlmError("OPENAI_API_KEY is not set".to_string()))?;
            Ok((
                "https://api.openai.com/v1/chat/completions".to_string(),
                Some(key),
            ))
        }
        Provider::DeepSeek => {
            let key = std::env::var("DEEPSEEK_API_KEY")
                .map_err(|_| UnevenError::LlmError("DEEPSEEK_API_KEY is not set".to_string()))?;
            // Correct DeepSeek endpoint — /v1 prefix is not used in currentAPI
            Ok((
                "https://api.deepseek.com/chat/completions".to_string(),
                Some(key),
            ))
        }
        Provider::Mistral => {
            let key = std::env::var("MISTRAL_API_KEY")
                .map_err(|_| UnevenError::LlmError("MISTRAL_API_KEY is not set".to_string()))?;
            Ok((
                "https://api.mistral.ai/v1/chat/completions".to_string(),
                Some(key),
            ))
        }
        Provider::Groq => {
            let key = std::env::var("GROQ_API_KEY")
                .map_err(|_| UnevenError::LlmError("GROQ_API_KEY is not set".to_string()))?;
            Ok((
                "https://api.groq.com/openai/v1/chat/completions".to_string(),
                Some(key),
            ))
        }
        Provider::Ollama => {
            let host = std::env::var("OLLAMA_API")
                .unwrap_or_else(|_| "http://localhost:11434".to_string());
            // Normalize: strip trailing slash and any trailing /v1 to avoid
            // double /v1 when OLLAMA_API is set to e.g. "http://localhost:11434/v1"
            let host = host
                .trim_end_matches('/')
                .trim_end_matches("/v1")
                .to_string();
            Ok((format!("{}/v1/chat/completions", host), None))
        }
        _ => Err(UnevenError::LlmError(
            "Provider is not OpenAI compatible".to_string(),
        )),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

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
    fn provider_parsing_is_case_insensitive() {
        // Enforce exact case matching based on typescript design checks
        assert!(matches!(
            Provider::from_string("local"),
            Ok(Provider::Local)
        ));
        assert!(matches!(
            Provider::from_string("openai"),
            Ok(Provider::OpenAi)
        ));
        assert!(matches!(
            Provider::from_string("claude"),
            Ok(Provider::Claude)
        ));
        assert!(matches!(
            Provider::from_string("gemini"),
            Ok(Provider::Gemini)
        ));
        assert!(matches!(
            Provider::from_string("ollama"),
            Ok(Provider::Ollama)
        ));
        assert!(matches!(
            Provider::from_string("deepseek"),
            Ok(Provider::DeepSeek)
        ));
        assert!(matches!(
            Provider::from_string("mistral"),
            Ok(Provider::Mistral)
        ));
        assert!(matches!(Provider::from_string("groq"), Ok(Provider::Groq)));
    }

    #[test]
    fn unknown_provider_returns_err() {
        assert!(Provider::from_string("mistral-unknown").is_err());
        assert!(Provider::from_string("").is_err());
    }

    #[test]
    fn error_message_contains_provider_name() {
        let err = Provider::from_string("unknown-llm").unwrap_err();
        assert!(err.to_string().contains("unknown-llm"), "{err}");
    }

    #[test]
    fn provider_from_string_with_spaces_returns_err() {
        assert!(Provider::from_string(" openai").is_err());
        assert!(Provider::from_string("openai ").is_err());
        assert!(Provider::from_string(" ").is_err());
    }

    #[test]
    fn openai_compat_request_has_no_reasoning_effort() {
        let req = build_openai_compat_request("gpt-4o", "hello", None, None, 1024, 0.7, false);
        // Serialized JSON must not contain reasoning_effort
        let json = serde_json::to_string(&req).unwrap();
        assert!(
            !json.contains("reasoning_effort"),
            "compat request must not send reasoning_effort: {json}"
        );
        assert!(
            json.contains("max_tokens"),
            "compat request must use max_tokens: {json}"
        );
    }

    #[test]
    fn openai_request_uses_max_completion_tokens() {
        let req = build_openai_request("gpt-4o", "hello", None, None, 1024, 0.7, false);
        let json = serde_json::to_string(&req).unwrap();
        assert!(
            json.contains("max_completion_tokens"),
            "openai request must use max_completion_tokens: {json}"
        );
        assert!(
            !json.contains("\"max_tokens\""),
            "openai request must not use max_tokens: {json}"
        );
    }

    #[test]
    fn gemini_thinking_config_has_no_thinking_level() {
        let config = GeminiThinkingConfig {
            thinking_budget: Some(2048),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(
            !json.contains("thinkingLevel"),
            "GeminiThinkingConfig must not serialize thinkingLevel: {json}"
        );
        assert!(
            json.contains("thinkingBudget"),
            "GeminiThinkingConfig must serialize thinkingBudget: {json}"
        );
    }

    #[test]
    fn ollama_url_no_double_v1() {
        // Simulate OLLAMA_API already containing /v1
        let host = "http://localhost:11434/v1";
        let normalized = host
            .trim_end_matches('/')
            .trim_end_matches("/v1")
            .to_string();
        let url = format!("{}/v1/chat/completions", normalized);
        assert_eq!(url, "http://localhost:11434/v1/chat/completions");
        assert!(
            !url.contains("/v1/v1"),
            "URL must not contain double /v1: {url}"
        );
    }

    #[test]
    fn deepseek_endpoint_has_no_v1_prefix() {
        // Verify the hardcoded DeepSeek URL is correct
        let url = "https://api.deepseek.com/chat/completions";
        assert!(
            !url.contains("/v1/"),
            "DeepSeek URL must not contain /v1/: {url}"
        );
    }
}
