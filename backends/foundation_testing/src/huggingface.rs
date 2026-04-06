//! HuggingFace integration for model downloading.
//!
//! WHY: Testing AI backends (e.g., llama.cpp) requires GGUF model files.
//! This module provides a test harness to download models from HuggingFace Hub.
//!
//! WHAT: `TestHarness` struct manages downloading and caching models from HuggingFace.
//!
//! HOW: Uses the new `foundation_deployment::providers::huggingface` client with
//! simple_http (no tokio/async required).

use foundation_core::valtron;
use foundation_deployment::providers::huggingface::{
    client, repository, HFClientBuilder, RepoDownloadFileParams,
};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Default artifacts directory name (relative to project root).
pub const DEFAULT_ARTIFACTS_DIR: &str = ".artifacts";

/// Default models subdirectory name.
pub const MODELS_SUBDIR: &str = "models";

/// TestHarness for downloading and managing test models from HuggingFace.
///
/// # Usage
///
/// ```rust,no_run
/// use foundation_testing::huggingface::TestHarness;
///
/// let harness = TestHarness::new(project_root);
/// let model_path = harness.get_model("unsloth/SmolLM2-360M-Instruct-GGUF", "SmolLM2-360M-Instruct-Q2_K.gguf");
/// ```
pub struct TestHarness {
    /// Root directory for storing downloaded models (artifacts/models/).
    artifacts_dir: PathBuf,
}

impl TestHarness {
    /// Create a new TestHarness with the given project root.
    ///
    /// # Arguments
    ///
    /// * `project_root` - Root directory of the project (models stored in `.artifacts/models/`)
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        let artifacts_dir = project_root.join(DEFAULT_ARTIFACTS_DIR).join(MODELS_SUBDIR);
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
    pub fn get_model(
        &self,
        repo_id: &str,
        filename: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let model_path = self.artifacts_dir.join(filename);

        // Check if model already exists
        if model_path.exists() {
            info!("Model already exists at: {}", model_path.display());
            return Ok(model_path);
        }

        // Create artifacts directory if it doesn't exist
        if !self.artifacts_dir.exists() {
            fs::create_dir_all(&self.artifacts_dir)?;
            info!(
                "Created artifacts directory: {}",
                self.artifacts_dir.display()
            );
        }

        // Download from HuggingFace Hub using our new provider
        info!("Downloading model {filename} from {repo_id}...");
        debug!("This may take a while for large models...");

        repository::repo_download_file(
            &self.build_repo(repo_id)?,
            &RepoDownloadFileParams {
                filename: filename.to_string(),
                revision: None,
                directory: self.artifacts_dir.clone(),
            },
        )?;

        info!("Model downloaded to: {}", model_path.display());
        Ok(model_path)
    }

    /// Build a repository handle from a repo_id string.
    fn build_repo(&self, repo_id: &str) -> Result<repository::HFRepository, Box<dyn std::error::Error + Send + Sync>> {
        // Initialize valtron pool for blocking execution
        let _guard = valtron::initialize_pool(42, Some(4));

        // Build client with token from environment
        let token = std::env::var("HF_TOKEN").ok();
        let mut builder = HFClientBuilder::new();
        if let Some(token) = token {
            builder = builder.token(token);
        }
        let client = builder.build()?;

        // Parse repo_id into owner and name
        let parts: Vec<&str> = repo_id.split('/').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid repo_id: {}. Expected format: 'owner/name'",
                repo_id
            )
            .into());
        }
        let owner = parts[0].to_string();
        let name = parts[1].to_string();

        // Get repository handle
        Ok(client.model(owner, name))
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
    pub fn get_smollm_model(&self) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        self.get_model(
            "unsloth/SmolLM2-360M-Instruct-GGUF",
            "SmolLM2-360M-Instruct-Q2_K.gguf",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifacts_dir_constant() {
        assert!(!DEFAULT_ARTIFACTS_DIR.is_empty());
        assert_eq!(MODELS_SUBDIR, "models");
    }

    #[test]
    fn test_harness_creation() {
        let temp_dir = std::env::temp_dir();
        let harness = TestHarness::new(&temp_dir);
        assert!(harness.artifacts_dir.ends_with(DEFAULT_ARTIFACTS_DIR));
        assert!(harness.artifacts_dir.ends_with(MODELS_SUBDIR));
    }
}
