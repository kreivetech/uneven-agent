use crate::UnevenError;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};

pub mod fallback;
pub mod candle;

pub use fallback::{EMBEDDING_DIM, hash_embed};



#[cfg(feature = "llm")]
static BERT: Lazy<std::sync::RwLock<Option<candle::BertState>>> =
    Lazy::new(|| std::sync::RwLock::new(None));

static MODEL_INIT_DONE: Lazy<AtomicBool> =
    Lazy::new(|| AtomicBool::new(false));

#[cfg(feature = "llm")]
fn read_bert() -> std::sync::RwLockReadGuard<'static, Option<candle::BertState>> {
    BERT.read().unwrap_or_else(|e| {
        tracing::warn!("Embeddings: BERT read-lock poisoned — recovering");
        e.into_inner()
    })
}

#[cfg(feature = "llm")]
fn write_bert() -> std::sync::RwLockWriteGuard<'static, Option<candle::BertState>> {
    BERT.write().unwrap_or_else(|e| {
        tracing::warn!("Embeddings: BERT write-lock poisoned — recovering");
        e.into_inner()
    })
}

pub async fn init_embeddings() -> Result<(), UnevenError> {
    // Skip entirely if BERT is already loaded — handles re-init calls cleanly.
    #[cfg(feature = "llm")]
    if read_bert().is_some() {
        return Ok(());
    }

    // Gate concurrent inits; reset on failure so retry is possible after model download.
    if MODEL_INIT_DONE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(());
    }

    #[cfg(feature = "llm")]
    {
        let models_dir = crate::infrastructure::utils::paths::global_models_dir();
        let model_path = models_dir.join("embeddings").join("model.safetensors");
        let tokenizer_path = models_dir.join("embeddings").join("tokenizer.json");
        let config_path = models_dir.join("embeddings").join("config.json");

        let all_exist = model_path.exists()
            && tokenizer_path.exists()
            && config_path.exists();

        if all_exist {
            tracing::info!("Embeddings: loading Candle BERT");
            let m_str = model_path.to_string_lossy();
            let t_str = tokenizer_path.to_string_lossy();
            let c_str = config_path.to_string_lossy();
            let load_res = tokio::task::block_in_place(|| {
                crate::infrastructure::llm::run_in_llm_pool(|| {
                    candle::try_load(&m_str, &t_str, &c_str)
                })
            });
            match load_res {
                Ok(state) => {
                    *write_bert() = Some(state);
                    tracing::info!("Embeddings: BERT model loaded");
                }
                Err(e) => {
                    MODEL_INIT_DONE.store(false, Ordering::Release);
                    tracing::warn!("Embeddings: BERT load failed — using fallback ({e})");
                }
            }
        } else {
            // Files not yet downloaded; reset so next init call retries after download.
            MODEL_INIT_DONE.store(false, Ordering::Release);
        }
    }
    #[cfg(not(feature = "llm"))]
    MODEL_INIT_DONE.store(false, Ordering::Release);

    Ok(())
}

pub async fn generate_embedding(text: &str) -> Result<Vec<f32>, UnevenError> {
    if text.is_empty() { return Err(UnevenError::LlmError("Empty text".into())); }

    #[cfg(feature = "llm")]
    {
        let result = tokio::task::block_in_place(|| {
            crate::infrastructure::llm::run_in_llm_pool(|| {
                let guard = read_bert();
                guard.as_ref().map(|state| candle::embed_text(text, state))
            })
        });
        if let Some(embed_result) = result {
            return embed_result.map_err(|e| UnevenError::LlmError(format!("BERT failed: {e}")));
        }
    }
    Ok(hash_embed(text))
}

pub async fn batch_embed(texts: Vec<&str>) -> Result<Vec<Vec<f32>>, UnevenError> {
    use rayon::prelude::*;
    tokio::task::block_in_place(|| {
        crate::infrastructure::llm::run_in_llm_pool(|| {
            texts.par_iter().map(|t| embed_sync(t)).collect()
        })
    })
}

fn embed_sync(text: &str) -> Result<Vec<f32>, UnevenError> {
    if text.is_empty() { return Err(UnevenError::LlmError("Empty text".into())); }
    #[cfg(feature = "llm")]
    {
        let guard = read_bert();
        if let Some(state) = guard.as_ref() {
            return candle::embed_text(text, state).map_err(|e| UnevenError::LlmError(format!("BERT failed: {e}")));
        }
    }
    Ok(hash_embed(text))
}

pub fn is_real_model_loaded() -> bool {
    #[cfg(feature = "llm")]
    { read_bert().is_some() }
    #[cfg(not(feature = "llm"))]
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::fallback::{hash_embed, normalize_l2, EMBEDDING_DIM};

    #[test]
    fn hash_embed_returns_correct_dimension() {
        let emb = hash_embed("hello world");
        assert_eq!(emb.len(), EMBEDDING_DIM);
    }

    #[test]
    fn hash_embed_is_deterministic() {
        let a = hash_embed("same input");
        let b = hash_embed("same input");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_embed_different_inputs_produce_different_outputs() {
        let a = hash_embed("input A");
        let b = hash_embed("input B");
        assert_ne!(a, b);
    }

    #[test]
    fn hash_embed_output_is_unit_normalized() {
        let emb = hash_embed("normalize me");
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm was {norm}");
    }

    #[test]
    fn normalize_l2_zero_vector_returns_error() {
        let mut v = vec![0.0f32; 8];
        let result = normalize_l2(&mut v);
        assert!(result.is_err());
    }

    #[test]
    fn normalize_l2_unit_vector_unchanged() {
        let mut v = vec![1.0f32, 0.0, 0.0];
        normalize_l2(&mut v).unwrap();
        assert!((v[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn is_real_model_loaded_false_without_llm_feature() {
        // Without the llm feature compiled in, BERT is never loaded.
        #[cfg(not(feature = "llm"))]
        assert!(!is_real_model_loaded());
        // With the llm feature, the model is only loaded after a successful init;
        // in a unit-test environment no model files exist so we still expect false.
        #[cfg(feature = "llm")]
        assert!(!is_real_model_loaded());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn generate_embedding_empty_text_returns_error() {
        let result = generate_embedding("").await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn generate_embedding_non_empty_returns_correct_dim() {
        let result = generate_embedding("hello").await.unwrap();
        assert_eq!(result.len(), EMBEDDING_DIM);
    }
}
