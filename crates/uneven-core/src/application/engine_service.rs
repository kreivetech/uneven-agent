use crate::domain::{UnevenConfig, UnevenError};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

/// Main Uneven engine service (Application Layer)
pub struct UnevenEngine {
    config: RwLock<UnevenConfig>,
    initialized: AtomicBool,
}

impl UnevenEngine {
    pub fn new(config: UnevenConfig) -> Self {
        let _ = crate::domain::PROJECT_ROOT.set(config.project_root.clone());
        UnevenEngine {
            config: RwLock::new(config),
            initialized: AtomicBool::new(false),
        }
    }

    /// Validates config and marks engine as ready. Idempotent — safe to call multiple times.
    pub async fn initialize(&self) -> Result<(), UnevenError> {
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        let config = self.config.read().await;
        if config.brain.provider.is_empty() {
            return Err(UnevenError::InitFailed("brain.provider is empty".to_string()));
        }
        if config.brain.model.is_empty() {
            return Err(UnevenError::InitFailed("brain.model is empty".to_string()));
        }
        drop(config);
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }

    pub async fn get_config(&self) -> UnevenConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, config: UnevenConfig) -> Result<(), UnevenError> {
        if config.brain.provider.is_empty() {
            return Err(UnevenError::ConfigError("brain.provider is empty".to_string()));
        }
        if config.brain.model.is_empty() {
            return Err(UnevenError::ConfigError("brain.model is empty".to_string()));
        }
        *self.config.write().await = config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BrainConfig;

    fn mock_config() -> UnevenConfig {
        UnevenConfig {
            project_root: ".".to_string(),
            brain: BrainConfig {
                provider: "local".to_string(),
                model: "test-model".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_engine_init_lifecycle() {
        let engine = UnevenEngine::new(mock_config());
        assert!(!engine.is_initialized());
        
        engine.initialize().await.unwrap();
        assert!(engine.is_initialized());
        
        // Idempotent
        assert!(engine.initialize().await.is_ok());
    }

    #[tokio::test]
    async fn test_engine_init_fails_on_empty_config() {
        let mut config = mock_config();
        config.brain.provider = "".to_string();
        let engine = UnevenEngine::new(config);
        assert!(engine.initialize().await.is_err());
        
        let mut config2 = mock_config();
        config2.brain.model = "".to_string();
        let engine2 = UnevenEngine::new(config2);
        assert!(engine2.initialize().await.is_err());
    }

    #[tokio::test]
    async fn test_engine_config_update() {
        let engine = UnevenEngine::new(mock_config());
        let mut new_config = mock_config();
        new_config.brain.model = "new-model".to_string();
        
        engine.update_config(new_config).await.unwrap();
        assert_eq!(engine.get_config().await.brain.model, "new-model");
    }

    #[tokio::test]
    async fn test_engine_config_update_validation() {
        let engine = UnevenEngine::new(mock_config());
        let mut bad_config = mock_config();
        bad_config.brain.provider = "".to_string();
        
        assert!(engine.update_config(bad_config).await.is_err());
    }
}
