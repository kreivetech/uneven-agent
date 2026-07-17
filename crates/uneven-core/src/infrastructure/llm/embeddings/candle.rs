#[cfg(feature = "llm")]
use candle_core::{DType, Device, Tensor};
#[cfg(feature = "llm")]
use candle_nn::VarBuilder;
#[cfg(feature = "llm")]
use candle_transformers::models::bert::{BertModel, Config};
#[cfg(feature = "llm")]
use tokenizers::Tokenizer;
#[cfg(feature = "llm")]
use super::fallback::normalize_l2;

#[cfg(feature = "llm")]
pub struct BertState {
    pub model: BertModel,
    pub tokenizer: Tokenizer,
    pub device: Device,
}

#[cfg(feature = "llm")]
fn select_device() -> Device {
    #[cfg(feature = "cuda")]
    if let Ok(d) = Device::new_cuda(0) {
        tracing::info!("Embeddings: selected CUDA device 0");
        return d;
    }
    #[cfg(feature = "metal")]
    if let Ok(d) = Device::new_metal(0) {
        tracing::info!("Embeddings: selected Metal device 0");
        return d;
    }
    tracing::info!("Embeddings: selected CPU");
    Device::Cpu
}

#[cfg(feature = "llm")]
pub fn try_load(model_path: &str, tokenizer_path: &str, config_path: &str) -> anyhow::Result<BertState> {
    let device = select_device();
    // F16 on GPU (native hardware support); F32 on CPU (no native F16 math).
    let dtype  = if matches!(device, Device::Cpu) { DType::F32 } else { DType::F16 };
    let tokenizer = Tokenizer::from_file(tokenizer_path)
        .map_err(|e| anyhow::anyhow!("Tokenizer load failed: {e}"))?;

    let config_str = std::fs::read_to_string(config_path)
        .map_err(|e| anyhow::anyhow!("Config read failed: {e}"))?;
    let config: Config = serde_json::from_str(&config_str)
        .map_err(|e| anyhow::anyhow!("Config parse failed: {e}"))?;

    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[model_path], dtype, &device)?
    };

    let model = BertModel::load(vb, &config)?;
    Ok(BertState { model, tokenizer, device })
}

#[cfg(feature = "llm")]
pub fn embed_text(text: &str, state: &BertState) -> anyhow::Result<Vec<f32>> {
    let encoding = state.tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("Tokenize: {e}"))?;

    let ids:      Vec<u32> = encoding.get_ids().to_vec();
    let type_ids: Vec<u32> = vec![0u32; ids.len()];
    let mask:     Vec<u32> = encoding.get_attention_mask().to_vec();

    let dev = &state.device;
    let input    = Tensor::new(&ids[..],      dev)?.unsqueeze(0)?;
    let type_t   = Tensor::new(&type_ids[..], dev)?.unsqueeze(0)?;
    let mask_t   = Tensor::new(&mask[..],     dev)?.unsqueeze(0)?;

    let hidden = state.model.forward(&input, &type_t, Some(&mask_t))?;
    let hidden   = hidden.squeeze(0)?;
    let seq_len  = hidden.dim(0)? as f64;
    let pooled   = (hidden.sum(0)? / seq_len)?;

    let mut vec = pooled.to_dtype(DType::F32)?.to_vec1::<f32>()?;
    let _ = normalize_l2(&mut vec);
    Ok(vec)
}
