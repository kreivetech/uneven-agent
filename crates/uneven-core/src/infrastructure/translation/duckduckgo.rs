use once_cell::sync::Lazy;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

static DDG_TRANSLATOR: Lazy<DuckDuckGoTranslator> = Lazy::new(DuckDuckGoTranslator::new);

/// Strict maximum characters allowed for an interactive user prompt.
/// This prevents payload overflow, translation rate-limits, and encourages
/// attaching files via the workspace picker rather than pasting code dumps.
pub const MAX_ALLOWED_PROMPT_CHARS: usize = 2000;

/// Maximum characters per individual DuckDuckGo translation request (safety margin under 1,000).
pub const MAX_CHUNK_CHARS: usize = 900;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TranslationResponse {
    #[serde(default)]
    translated: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

pub struct DuckDuckGoTranslator {
    client: reqwest::Client,
    cached_vqd: Arc<RwLock<Option<String>>>,
}

impl Default for DuckDuckGoTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl DuckDuckGoTranslator {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .build()
            .unwrap_or_default();

        Self {
            client,
            cached_vqd: Arc::new(RwLock::new(None)),
        }
    }

    /// Fetches a fresh session `vqd` token from DuckDuckGo.
    async fn fetch_vqd(&self) -> Option<String> {
        let resp = self
            .client
            .get("https://duckduckgo.com/?q=translate")
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .send()
            .await
            .ok()?;

        // 1. Try header
        if let Some(v) = resp.headers().get("x-vqd-4").and_then(|h| h.to_str().ok()) {
            return Some(v.to_string());
        }

        // 2. Try HTML body regex
        let html = resp.text().await.ok()?;
        static RE_VQD: Lazy<regex::Regex> =
            Lazy::new(|| regex::Regex::new(r#"vqd="([^"]+)""#).unwrap());

        RE_VQD.captures(&html).map(|c| c.get(1).unwrap().as_str().to_string())
    }

    /// Translates a single text chunk (strictly <= 950 chars) using DuckDuckGo.
    async fn translate_single_chunk(&self, chunk: &str) -> String {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            return chunk.to_string();
        }

        // Quick heuristic: If purely ASCII and no common non-English keywords, avoid redundant call
        let is_ascii = trimmed.is_ascii();
        let has_pt_markers = trimmed.split_whitespace().any(|w| {
            matches!(
                w.to_lowercase().as_str(),
                "crie"
                    | "criar"
                    | "fazer"
                    | "faça"
                    | "para"
                    | "com"
                    | "não"
                    | "uma"
                    | "este"
                    | "está"
                    | "ajuste"
                    | "adicione"
                    | "mude"
                    | "gere"
                    | "arrume"
                    | "quando"
                    | "onde"
                    | "como"
            )
        });

        if is_ascii && !has_pt_markers {
            return chunk.to_string();
        }

        // Acquire or refresh vqd token
        let mut vqd = {
            let r = self.cached_vqd.read().await;
            r.clone()
        };

        if vqd.is_none()
            && let Some(new_vqd) = self.fetch_vqd().await
        {
            *self.cached_vqd.write().await = Some(new_vqd.clone());
            vqd = Some(new_vqd);
        }

        let Some(current_vqd) = vqd else {
            return chunk.to_string();
        };

        let url = format!(
            "https://duckduckgo.com/translation.js?query=translate&vqd={}&from=pt&to=en",
            current_vqd
        );

        let req = self
            .client
            .post(&url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .header("Content-Type", "text/plain;charset=UTF-8")
            .header("Referer", "https://duckduckgo.com/")
            .body(trimmed.to_string());

        if let Ok(resp) = req.send().await {
            if resp.status().is_success() {
                if let Ok(payload) = resp.json::<TranslationResponse>().await
                    && let Some(translated) = payload.translated
                    && !translated.trim().is_empty()
                {
                    return translated;
                }
            } else {
                *self.cached_vqd.write().await = None;
            }
        }

        chunk.to_string()
    }

    /// Translates input text into English with smart paragraph chunking and code fence preservation.
    /// Strictly capped at MAX_ALLOWED_PROMPT_CHARS.
    pub async fn translate_to_english(&self, text: &str) -> String {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return text.to_string();
        }

        // If fits within a single chunk, translate directly
        if trimmed.len() <= MAX_CHUNK_CHARS {
            return self.translate_single_chunk(trimmed).await;
        }

        // Smart chunking: split by paragraphs or newlines
        let lines: Vec<&str> = trimmed.split('\n').collect();
        let mut chunks: Vec<String> = Vec::new();
        let mut current_chunk = String::new();

        for line in lines {
            if current_chunk.len() + line.len() + 1 > MAX_CHUNK_CHARS {
                if !current_chunk.trim().is_empty() {
                    chunks.push(current_chunk.trim().to_string());
                }
                current_chunk = line.to_string();
            } else {
                if !current_chunk.is_empty() {
                    current_chunk.push('\n');
                }
                current_chunk.push_str(line);
            }
        }
        if !current_chunk.trim().is_empty() {
            chunks.push(current_chunk.trim().to_string());
        }

        // Translate each chunk sequentially with the shared session vqd
        let mut translated_pieces: Vec<String> = Vec::new();
        for chunk in chunks {
            // If the chunk is a code fence, preserve it untouched
            if chunk.trim().starts_with("```") && chunk.trim().ends_with("```") {
                translated_pieces.push(chunk);
            } else {
                let tr = self.translate_single_chunk(&chunk).await;
                translated_pieces.push(tr);
            }
        }

        translated_pieces.join("\n\n")
    }
}

/// Convenience global function to translate a user prompt to English.
pub async fn translate_prompt_to_english(text: &str) -> String {
    DDG_TRANSLATOR.translate_to_english(text).await
}
