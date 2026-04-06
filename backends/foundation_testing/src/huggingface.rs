//! HuggingFace integration for model downloading.
//!
//! WHY: Testing AI backends (e.g., llama.cpp) requires GGUF model files.
//! This module provides a test harness to download models from HuggingFace Hub.
//!
//! WHAT: `TestHarness` struct manages downloading and caching models from HuggingFace.
//!
//! HOW: Uses the `hf-hub` crate with tokio runtime for async downloads.
//!
//! **Note**: This module is test-only. Tokio is used exclusively for the hf-hub
//! async runtime during model downloads.

use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, debug};

/// Default artifacts directory name (relative to project root).
pub const DEFAULT_ARTIFACTS_DIR: &str = ".artifacts";

/// TestHarness for downloading and managing test models from HuggingFace.
///
/// # Usage
///
/// ```rust,no_run
/// use foundation_testing::huggingface::TestHarness;
///
/// let harness = TestHarness::new(project_root);
/// let model_path = harness.get_model("unsloth/SmolLM2-360M-Instruct-GGUF", "SmolLM2-360M-Instruct-Q2_K.gguf").await;
/// ```
pub struct TestHarness {
    /// Root directory for storing downloaded models.
    artifacts_dir: PathBuf,
}

impl TestHarness {
    /// Create a new TestHarness with the given project root.
    ///
    /// # Arguments
    ///
    /// * `project_root` - Root directory of the project (models stored in `.artifacts/`)
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        let artifacts_dir = project_root.join(DEFAULT_ARTIFACTS_DIR);
        Self { artifacts_dir }
    }

    /// Get or download a model from HuggingFace Hub.
    ///
    /// Checks if the model exists in the artifacts directory. If not,
    /// downloads it from HuggingFace Hub.
    ///
    /// # Arguments
    ///
    /// * `repo_id` - HuggingFace repository ID (e.g., "unsloth/SmolLM2-360M-Instruct-GGUF")
    /// * `filename` - Model filename (e.g., "SmolLM2-360M-Instruct-Q2_K.gguf")
    ///
    /// # Returns
    ///
    /// Path to the downloaded (or existing) model file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The artifacts directory cannot be created
    /// - The download fails
    /// - The model file is not found after download
    pub async fn get_model(&self, repo_id: &str, filename: &str) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let model_path = self.artifacts_dir.join(filename);

        // Check if model already exists
        if model_path.exists() {
            info!("Model already exists at: {}", model_path.display());
            return Ok(model_path);
        }

        // Create artifacts directory if it doesn't exist
        if !self.artifacts_dir.exists() {
            fs::create_dir_all(&self.artifacts_dir)?;
            info!("Created artifacts directory: {}", self.artifacts_dir.display());
        }

        // Download from HuggingFace Hub
        info!("Downloading model {filename} from {repo_id}...");
        debug!("This may take a while for large models...");

        let api = hf_hub::api::tokio::ApiBuilder::new()
            .with_progress(true)
            .build()?;

        let repo = api.repo(hf_hub::Repo::model(repo_id.to_string()));
        let downloaded_path = repo.get(filename).await?;

        // Move/copy the file to our artifacts directory
        fs::copy(&downloaded_path, &model_path)?;

        info!("Model downloaded to: {}", model_path.display());
        Ok(model_path)
    }

    /// Get the default SmolLM2 model for testing.
    ///
    /// Convenience wrapper using the default SmolLM2 model repository.
    ///
    /// # Returns
    ///
    /// Path to the downloaded (or existing) model file.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails.
    pub async fn get_smollm_model(&self) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        self.get_model(
            "unsloth/SmolLM2-360M-Instruct-GGUF",
            "SmolLM2-360M-Instruct-Q2_K.gguf",
        ).await
    }

    /// Get a model by repo and filename without caching.
    ///
    /// Similar to `get_model()` but downloads directly without copying to artifacts.
    /// Useful for one-off downloads or when you want to manage the file location yourself.
    ///
    /// # Arguments
    ///
    /// * `repo_id` - HuggingFace repository ID
    /// * `filename` - Model filename
    ///
    /// # Returns
    ///
    /// Path to the downloaded file in the HuggingFace cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails.
    pub async fn download_model(&self, repo_id: &str, filename: &str) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        info!("Downloading model {filename} from {repo_id}...");

        let api = hf_hub::api::tokio::ApiBuilder::new()
            .with_progress(true)
            .build()?;

        let repo = api.repo(hf_hub::Repo::model(repo_id.to_string()));
        let downloaded_path = repo.get(filename).await?;

        info!("Model cached at: {}", downloaded_path.display());
        Ok(downloaded_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifacts_dir_constant() {
        assert!(!DEFAULT_ARTIFACTS_DIR.is_empty());
    }

    #[test]
    fn test_harness_creation() {
        let temp_dir = std::env::temp_dir();
        let harness = TestHarness::new(&temp_dir);
        assert!(harness.artifacts_dir.ends_with(DEFAULT_ARTIFACTS_DIR));
    }
}
