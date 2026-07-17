//! Fix generation and application module
//!
//! Reads files, generates patches using the similar crate for diffs,
//! and applies changes surgically.

use std::path::{Component, Path};
use crate::UnevenError;

pub fn generate_patch(
    file_path: &str,
    original: &str,
    fixed: &str,
) -> Result<String, UnevenError> {
    tracing::debug!("Generating patch for: {}", file_path);
    let diff = similar::TextDiff::from_lines(original, fixed);
    let old_header = format!("a/{}", file_path);
    let new_header = format!("b/{}", file_path);
    Ok(format!("{}", diff.unified_diff().header(&old_header, &new_header)))
}

pub async fn apply_patch(
    file_path: &str,
    original: &str,
    fixed: &str,
    top_of_file: bool,
    append_eof: bool,
) -> Result<(), UnevenError> {
    tracing::info!("Applying patch to: {}", file_path);

    if Path::new(file_path).components().any(|c| c == Component::ParentDir) {
        return Err(UnevenError::AgentError(format!(
            "Refusing to write to path with traversal component: {}",
            file_path
        )));
    }

    if original == fixed && !top_of_file && !append_eof {
        return Ok(());
    }

    let current = tokio::fs::read_to_string(file_path)
        .await
        .map_err(|e| UnevenError::AgentError(format!("Failed to read file: {}", e)))?;

    let new_content = if top_of_file {
        format!("{}{}", fixed, current)
    } else if append_eof {
        format!("{}{}", current, fixed)
    } else if current == original {
        // Mode 1: Full Replacement
        fixed.to_string()
    } else if !original.is_empty() && current.contains(original.trim()) {
        // Mode 2: Block Replacement (resilient matching)
        tracing::debug!("FixEngine: Block match found using trimmed search");
        current.replace(original.trim(), fixed)
    } else if original.is_empty() {
        // Mode 3: Append or Full Overwrite
        fixed.to_string()
    } else {
        // TEMPORARY DIAGNOSTIC LOG
        tracing::warn!("FixEngine: Block mismatch for {}", file_path);
        tracing::debug!("FixEngine: Current file sample: {:?}", &current.get(0..100).unwrap_or(&current));
        tracing::debug!("FixEngine: Expected block sample: {:?}", &original.get(0..100).unwrap_or(original));

        return Err(UnevenError::AgentError(format!(
            "[UE-06000011] File '{}' state has changed — skipping fix application",
            file_path
        )));
    };

    let tmp_path = format!("{}.tmp", file_path);
    tokio::fs::write(&tmp_path, new_content)
        .await
        .map_err(|e| UnevenError::AgentError(format!("Failed to write temp file: {}", e)))?;

    tokio::fs::rename(&tmp_path, file_path)
        .await
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            UnevenError::AgentError(format!("Failed to rename temp file: {}", e))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_patch_basic() {
        let original = "fn main() {\n    println!(\"hello\");\n}\n";
        let fixed = "fn main() {\n    println!(\"world\");\n}\n";
        let patch = generate_patch("main.rs", original, fixed).unwrap();
        assert!(patch.contains("-    println!(\"hello\");"));
        assert!(patch.contains("+    println!(\"world\");"));
        assert!(patch.contains("--- a/main.rs"));
        assert!(patch.contains("+++ b/main.rs"));
    }

    #[tokio::test]
    async fn test_apply_patch_lifecycle() {
        let tmp_file = std::env::temp_dir().join(format!("un-fix-{}", std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()));
        let path = tmp_file.to_str().unwrap();
        
        let original = "line1\nline2\n";
        let fixed = "line1\nfixed2\n";
        
        tokio::fs::write(path, original).await.unwrap();
        
        // Success
        apply_patch(path, original, fixed, false, false).await.unwrap();
        let current = tokio::fs::read_to_string(path).await.unwrap();
        assert_eq!(current, fixed);
        
        // No-op (original == fixed)
        apply_patch(path, fixed, fixed, false, false).await.unwrap();
        
        // Divergence detection
        let err = apply_patch(path, "stale content", "new", false, false).await.unwrap_err();
        assert!(err.to_string().contains("state has changed"));
        
        // Path traversal denial
        let err_traversal = apply_patch("../../etc/passwd", "original", "fixed", false, false).await.unwrap_err();
        assert!(err_traversal.to_string().contains("traversal"));
        
        tokio::fs::remove_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn test_apply_patch_block_replacement() {
        let tmp_file = std::env::temp_dir().join(format!("un-fix-block-{}", std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()));
        let path = tmp_file.to_str().unwrap();
        
        let original_full = "line1\nTARGET_BLOCK\nline3\n";
        let target_block = "TARGET_BLOCK";
        let fixed_block = "REPLACED_BLOCK";
        
        tokio::fs::write(path, original_full).await.unwrap();
        
        apply_patch(path, target_block, fixed_block, false, false).await.unwrap();
        let current = tokio::fs::read_to_string(path).await.unwrap();
        assert_eq!(current, "line1\nREPLACED_BLOCK\nline3\n");
        
        tokio::fs::remove_file(path).await.unwrap();
    }
}
