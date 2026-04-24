//! HuggingFace Candle Provider — safetensors model downloading + Candle inference.
//!
//! Wraps [`CandleBackend`] with automatic safetensors model downloading from
//! HuggingFace Hub, mirroring the [`HuggingFaceGGUFProvider`] pattern for GGUF models.

use std::path::PathBuf;

use crate::backends::candle::{
    CandleArchitecture, CandleBackend, CandleBackendConfig, CandleDType, CandleModels,
};
use crate::errors::{ModelErrors, ModelProviderErrors, ModelProviderResult};
use crate::types::{ModelId, ModelProvider, ModelSpec};
use foundation_deployment::providers::huggingface::{
    HFClient, RepoDownloadFileParams, RepoListTreeParams, RepoTreeEntry,
};
use foundation_deployment::providers::huggingface::repository;
use foundation_core::valtron::Stream;

/// HuggingFace provider for safetensors models via the Candle inference backend.
///
/// Downloads models from HuggingFace Hub and loads them using [`CandleBackend`].
pub struct HuggingFaceCandleProvider {
    hf_client: HFClient,
    backend: CandleBackend,
    cache_dir: PathBuf,
    architecture: CandleArchitecture,
}

impl core::fmt::Debug for HuggingFaceCandleProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HuggingFaceCandleProvider")
            .field("cache_dir", &self.cache_dir)
            .field("architecture", &self.architecture)
            .finish_non_exhaustive()
    }
}

impl Clone for HuggingFaceCandleProvider {
    fn clone(&self) -> Self {
        Self {
            hf_client: self.hf_client.clone(),
            backend: self.backend.clone(),
            cache_dir: self.cache_dir.clone(),
            architecture: self.architecture.clone(),
        }
    }
}

/// Configuration for [`HuggingFaceCandleProvider`].
#[derive(Debug)]
pub struct HuggingFaceCandleConfig {
    pub auth: Option<foundation_auth::AuthCredential>,
    pub cache_dir: PathBuf,
    pub context_length: usize,
    pub dtype: CandleDType,
    pub architecture: CandleArchitecture,
}

impl Default for HuggingFaceCandleConfig {
    fn default() -> Self {
        Self {
            auth: None,
            cache_dir: default_cache_dir(),
            context_length: 4096,
            dtype: CandleDType::F32,
            architecture: CandleArchitecture::Llama,
        }
    }
}

impl Clone for HuggingFaceCandleConfig {
    fn clone(&self) -> Self {
        Self {
            auth: None,
            cache_dir: self.cache_dir.clone(),
            context_length: self.context_length,
            dtype: self.dtype,
            architecture: self.architecture.clone(),
        }
    }
}

impl crate::types::AuthProvider for HuggingFaceCandleConfig {
    fn auth(&self) -> Option<&foundation_auth::AuthCredential> {
        self.auth.as_ref()
    }
}

impl HuggingFaceCandleConfig {
    #[must_use]
    pub fn builder() -> HuggingFaceCandleConfigBuilder {
        HuggingFaceCandleConfigBuilder::new()
    }
}

fn default_cache_dir() -> PathBuf {
    std::env::var("HF_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("XDG_CACHE_HOME")
                .ok()
                .map(|p| PathBuf::from(p).join("huggingface"))
        })
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|p| PathBuf::from(p).join(".cache").join("huggingface"))
        })
        .unwrap_or_else(|| PathBuf::from("./huggingface_cache"))
}

/// Builder for [`HuggingFaceCandleConfig`].
#[derive(Debug, Clone)]
pub struct HuggingFaceCandleConfigBuilder {
    config: HuggingFaceCandleConfig,
}

impl Default for HuggingFaceCandleConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HuggingFaceCandleConfigBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: HuggingFaceCandleConfig::default(),
        }
    }

    #[must_use]
    pub fn hf_token(mut self, token: impl Into<String>) -> Self {
        self.config.auth = Some(foundation_auth::AuthCredential::SecretOnly(
            foundation_auth::ConfidentialText::new(token.into()),
        ));
        self
    }

    #[must_use]
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.cache_dir = path.into();
        self
    }

    #[must_use]
    pub fn context_length(mut self, n: usize) -> Self {
        self.config.context_length = n;
        self
    }

    #[must_use]
    pub fn dtype(mut self, dtype: CandleDType) -> Self {
        self.config.dtype = dtype;
        self
    }

    #[must_use]
    pub fn architecture(mut self, arch: CandleArchitecture) -> Self {
        self.config.architecture = arch;
        self
    }

    #[must_use]
    pub fn build(self) -> HuggingFaceCandleConfig {
        self.config
    }
}

impl HuggingFaceCandleProvider {
    /// Create a new provider from configuration.
    ///
    /// Initialises the HF client and underlying [`CandleBackend`] (CPU).
    pub fn new(config: HuggingFaceCandleConfig) -> ModelProviderResult<Self> {
        let hf_client = HFClient::builder()
            .token(
                config
                    .auth
                    .as_ref()
                    .map(|a| match a {
                        foundation_auth::AuthCredential::SecretOnly(t) => t.get(),
                        _ => String::new(),
                    })
                    .unwrap_or_default(),
            )
            .build()
            .map_err(|e| {
                ModelProviderErrors::FailedFetching(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create HFClient: {e}"),
                )))
            })?;

        std::fs::create_dir_all(&config.cache_dir).map_err(|e| {
            ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
                "Failed to create cache directory: {e}"
            )))
        })?;

        let architecture = config.architecture.clone();
        let backend_config = CandleBackendConfig::builder()
            .context_length(config.context_length)
            .dtype(config.dtype)
            .architecture(config.architecture.clone())
            .cache_dir(&config.cache_dir)
            .build();

        let backend = CandleBackend::cpu()
            .create(Some(backend_config))?;

        Ok(Self {
            hf_client,
            backend,
            cache_dir: config.cache_dir,
            architecture,
        })
    }

    /// Parse a [`ModelId`] into a HuggingFace repository id.
    ///
    /// Expects `ModelId::Name("owner/repo", _)`.
    #[must_use]
    pub fn parse_repo_id(model_id: &ModelId) -> Option<String> {
        match model_id {
            ModelId::Name(name, _) => {
                let repo_id = name.split(':').next().unwrap_or(name);
                if repo_id.contains('/') {
                    Some(repo_id.to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Download model files (config.json, tokenizer.json, safetensors) from
    /// HuggingFace Hub into the local cache directory.
    ///
    /// Returns the local directory path containing the downloaded files.
    pub fn download_model(&self, repo_id: &str) -> ModelProviderResult<PathBuf> {
        let dest_dir = self.cache_dir.join(repo_id.replace('/', "--"));

        // Check if already cached
        if dest_dir.join("config.json").exists() && has_safetensors(&dest_dir) {
            tracing::debug!("Found cached model: {:?}", dest_dir);
            return Ok(dest_dir);
        }

        std::fs::create_dir_all(&dest_dir).map_err(|e| {
            ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
                "Failed to create model directory: {e}"
            )))
        })?;

        tracing::info!("Downloading safetensors model {} to {:?}", repo_id, dest_dir);

        let repo = self.hf_client.model(
            repo_id.split('/').next().unwrap_or("").to_string(),
            repo_id.split('/').skip(1).collect::<Vec<_>>().join("/"),
        );

        // Download config.json and tokenizer.json
        for filename in &["config.json", "tokenizer.json"] {
            let params = RepoDownloadFileParams {
                filename: filename.to_string(),
                revision: Some("main".to_string()),
                directory: dest_dir.clone(),
            };
            repository::repo_download_file(&repo, &params).map_err(|e| {
                ModelProviderErrors::FailedFetching(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to download {filename}: {e}"),
                )))
            })?;
        }

        // Download safetensors — try single file first, then sharded
        let single_ok = {
            let params = RepoDownloadFileParams {
                filename: "model.safetensors".to_string(),
                revision: Some("main".to_string()),
                directory: dest_dir.clone(),
            };
            repository::repo_download_file(&repo, &params).is_ok()
        };

        if single_ok {
            tracing::info!("Downloaded model.safetensors to {:?}", dest_dir);
        } else {
            // Try sharded via index
            let index_params = RepoDownloadFileParams {
                filename: "model.safetensors.index.json".to_string(),
                revision: Some("main".to_string()),
                directory: dest_dir.clone(),
            };
            let index_path = repository::repo_download_file(&repo, &index_params).map_err(|e| {
                ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
                    "No model.safetensors or index found in {repo_id}: {e}"
                )))
            })?;

            let index_content = std::fs::read_to_string(&index_path).map_err(|e| {
                ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
                    "Failed to read index: {e}"
                )))
            })?;
            let index: serde_json::Value =
                serde_json::from_str(&index_content).map_err(|e| {
                    ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
                        "Failed to parse index: {e}"
                    )))
                })?;

            if let Some(weight_map) = index.get("weight_map").and_then(|w| w.as_object()) {
                let mut filenames: Vec<String> = weight_map
                    .values()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                filenames.sort();
                filenames.dedup();

                for filename in &filenames {
                    let params = RepoDownloadFileParams {
                        filename: filename.clone(),
                        revision: Some("main".to_string()),
                        directory: dest_dir.clone(),
                    };
                    repository::repo_download_file(&repo, &params).map_err(|e| {
                        ModelProviderErrors::FailedFetching(Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to download shard {filename}: {e}"),
                        )))
                    })?;
                }
            }
        }

        tracing::info!("Model downloaded to {:?}", dest_dir);
        Ok(dest_dir)
    }

    /// List available safetensors files in a HuggingFace repository.
    ///
    /// Useful for discovering what models/shards are available before downloading.
    pub fn list_model_files(&self, repo_id: &str) -> ModelProviderResult<Vec<String>> {
        let repo = self.hf_client.model(
            repo_id.split('/').next().unwrap_or("").to_string(),
            repo_id.split('/').skip(1).collect::<Vec<_>>().join("/"),
        );

        let params = RepoListTreeParams {
            revision: Some("main".to_string()),
            recursive: Some(true),
            limit: None,
        };

        let tree = repository::repo_list_tree(&repo, &params).map_err(|e| {
            ModelProviderErrors::FailedFetching(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to list repository files: {e}"),
            )))
        })?;

        let entries: Vec<_> = tree
            .filter_map(|s| match s {
                Stream::Next(Ok(entry)) => Some(entry),
                Stream::Next(Err(_)) => None,
                _ => None,
            })
            .collect();

        let mut safetensors_files: Vec<String> = entries
            .iter()
            .filter_map(|entry| {
                if let RepoTreeEntry::File { path, .. } = entry {
                    if path.ends_with(".safetensors") || path.ends_with(".safetensors.index.json") {
                        Some(path.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        safetensors_files.sort();

        Ok(safetensors_files)
    }
}

impl ModelProvider for HuggingFaceCandleProvider {
    type Config = HuggingFaceCandleConfig;
    type Model = CandleModels;

    fn create(
        self,
        config: Option<Self::Config>,
    ) -> ModelProviderResult<Self>
    where
        Self: Sized,
    {
        if let Some(config) = config {
            HuggingFaceCandleProvider::new(config)
        } else {
            Ok(self)
        }
    }

    fn describe(&self) -> ModelProviderResult<crate::types::ModelProviderDescriptor> {
        Ok(crate::types::ModelProviderDescriptor {
            id: "huggingface-candle".to_string(),
            name: "HuggingFace Hub (Candle)".to_string(),
            reasoning: false,
            api: crate::types::ModelAPI::Custom("huggingface-candle".to_string()),
            provider: crate::types::ModelProviders::HUGGINGFACE,
            base_url: Some("https://huggingface.co".to_string()),
            inputs: crate::types::MessageType::Text,
            cost: crate::types::ModelUsageCosting {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 4096,
            max_tokens: 2048,
        })
    }

    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model> {
        let repo_id = Self::parse_repo_id(&model_id).ok_or_else(|| {
            ModelProviderErrors::NotFound(format!(
                "Invalid ModelId for Candle: {model_id:?}. Expected 'owner/repo'."
            ))
        })?;

        let model_dir = self.download_model(&repo_id)?;

        let spec = ModelSpec {
            name: repo_id,
            id: model_id,
            devices: None,
            model_location: Some(model_dir),
            lora_location: None,
        };

        self.backend.get_model_by_spec(spec)
    }

    fn get_model_by_spec(&self, model_spec: ModelSpec) -> ModelProviderResult<Self::Model> {
        if model_spec.model_location.is_some() {
            return self.backend.get_model_by_spec(model_spec);
        }

        let repo_id = Self::parse_repo_id(&model_spec.id).ok_or_else(|| {
            ModelProviderErrors::NotFound(
                "No model_location and ModelId is not a HuggingFace repo id".to_string(),
            )
        })?;

        let model_dir = self.download_model(&repo_id)?;

        let spec = ModelSpec {
            model_location: Some(model_dir),
            ..model_spec
        };

        self.backend.get_model_by_spec(spec)
    }

    fn get_one(&self, _model_id: ModelId) -> ModelProviderResult<ModelSpec> {
        Err(ModelProviderErrors::NotFound(
            "Model catalog not implemented for Candle provider".to_string(),
        ))
    }

    fn get_all(&self, _model_id: ModelId) -> ModelProviderResult<Vec<ModelSpec>> {
        Err(ModelProviderErrors::NotFound(
            "Model catalog not implemented for Candle provider".to_string(),
        ))
    }
}

fn has_safetensors(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| {
                    e.path()
                        .extension()
                        .map_or(false, |ext| ext == "safetensors")
                })
        })
        .unwrap_or(false)
}
