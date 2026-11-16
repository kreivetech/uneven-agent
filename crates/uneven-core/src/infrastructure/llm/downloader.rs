use crate::domain::consts::models::SUPPORTED_LOCAL_MODELS;
use crate::UnevenError;
use std::path::Path;

/// Checks if both the GGUF model and tokenizer files exist locally in global models dir.
pub fn is_local_model_downloaded(model: &str) -> bool {
    let global_models_dir = crate::infrastructure::utils::paths::global_models_dir();
    let gguf_path = global_models_dir.join(format!("{}.gguf", model));
    let tokenizer_path = global_models_dir.join(format!("{}-tokenizer.json", model));
    gguf_path.exists() && tokenizer_path.exists()
}

/// Downloads a single file from a URL to a local destination path via HTTP streaming.
pub async fn download_file(url: &str, dest_path: &Path) -> Result<(), UnevenError> {
    if let Some(parent) = dest_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let client = reqwest::Client::new();
    let mut res = client
        .get(url)
        .send()
        .await
        .map_err(|e| UnevenError::LlmError(format!("Failed to connect to {}: {}", url, e)))?;

    if !res.status().is_success() {
        return Err(UnevenError::LlmError(format!(
            "Failed download response status {} for {}",
            res.status(),
            url
        )));
    }

    let mut file = std::fs::File::create(dest_path)
        .map_err(|e| UnevenError::LlmError(format!("Failed to create destination file: {}", e)))?;

    use std::io::Write;
    while let Some(chunk) = res
        .chunk()
        .await
        .map_err(|e| UnevenError::LlmError(format!("Stream read error: {}", e)))?
    {
        file.write_all(&chunk)
            .map_err(|e| UnevenError::LlmError(format!("Failed writing to disk: {}", e)))?;
    }

    Ok(())
}

/// Ensures all necessary files for a local GGUF model are present, downloading them if missing.
pub async fn ensure_local_model_files(model: &str) -> Result<(), UnevenError> {
    if is_local_model_downloaded(model) {
        return Ok(());
    }

    let catalog_entry = SUPPORTED_LOCAL_MODELS
        .iter()
        .find(|m| m.id == model)
        .ok_or_else(|| {
            UnevenError::LlmError(format!(
                "Model '{}' is not in the supported local catalog for auto-download.",
                model
            ))
        })?;

    let global_models_dir = crate::infrastructure::utils::paths::global_models_dir();
    for file_entry in catalog_entry.files {
        let suffix = file_entry
            .dest
            .strip_prefix(".uneven/models/")
            .unwrap_or(file_entry.dest);
        let dest = global_models_dir.join(suffix);
        if dest.exists() {
            continue;
        }
        download_file(file_entry.url, &dest).await?;
    }

    Ok(())
}
