//! HuggingFace GGUF Provider - LlamaCpp wrapper with GGUF model downloading.
//!
//! This module provides a thin wrapper around [`LlamaBackends`] that adds
//! automatic GGUF model downloading from HuggingFace Hub.

use std::path::PathBuf;

use foundation_deployment::providers::huggingface::{
    HFClient, HFRepository, RepoDownloadFileParams,
};

use crate::backends::llamacpp::{LlamaBackendConfig, LlamaBackends, LlamaModels};
use crate::errors::{ModelProviderErrors, ModelProviderResult};
use crate::types::{ModelId, ModelProvider, ModelSpec};
use foundation_deployment::providers::huggingface::repository;

/// HuggingFace Hub model provider.
///
/// Wraps `LlamaBackends` with automatic model downloading from HuggingFace Hub.
///
/// # Fields
///
/// * `hf_client` - Client for HuggingFace Hub API
/// * `llama_backend` - Which llama.cpp backend variant to use (CPU/GPU/Metal)
/// * `cache_dir` - Local directory for cached GGUF files
/// * `default_quantization` - Default quantization when not specified in ModelId
#[derive(Clone)]
pub struct HuggingFaceGGUFProvider {
    hf_client: HFClient,
    llama_backend: LlamaBackends,
    cache_dir: PathBuf,
    default_quantization: Option<String>,
}

/// Configuration for HuggingFace provider.
///
/// # Example
///
/// ```rust
/// use foundation_ai::backends::huggingface_provider::HuggingFaceGGUFConfig;
/// use foundation_ai::backends::llamacpp::LlamaBackends;
///
/// let config = HuggingFaceGGUFConfig::builder()
///     .token("hf_...")
///     .cache_dir("~/.cache/huggingface")
///     .llama_backend(LlamaBackends::LLamaGPU)  // Use GPU
///     .n_gpu_layers(32)
///     .build();
/// ```
#[derive(Debug)]
pub struct HuggingFaceGGUFConfig {
    /// HuggingFace API token (optional for public models).
    pub auth: Option<foundation_auth::AuthCredential>,
    /// Local cache directory for downloaded GGUF files.
    pub cache_dir: PathBuf,
    /// Default quantization when not specified in ModelId.
    pub default_quantization: Option<String>,
    /// llama.cpp backend configuration.
    pub llama_config: LlamaBackendConfig,
    /// Which llama.cpp backend variant to use.
    pub llama_backend: LlamaBackends,
}

impl Default for HuggingFaceGGUFConfig {
    fn default() -> Self {
        Self {
            auth: None,
            cache_dir: default_cache_dir(),
            default_quantization: Some("q4_k_m".to_string()),
            llama_config: LlamaBackendConfig::default(),
            llama_backend: LlamaBackends::LLamaCPU,
        }
    }
}

impl Clone for HuggingFaceGGUFConfig {
    fn clone(&self) -> Self {
        Self {
            auth: None,
            cache_dir: self.cache_dir.clone(),
            default_quantization: self.default_quantization.clone(),
            llama_config: self.llama_config.clone(),
            llama_backend: self.llama_backend,
        }
    }
}

impl crate::types::AuthProvider for HuggingFaceGGUFConfig {
    fn auth(&self) -> Option<&foundation_auth::AuthCredential> {
        self.auth.as_ref()
    }
}

impl HuggingFaceGGUFConfig {
    /// Create a new config builder with default values.
    #[must_use]
    pub fn builder() -> HuggingFaceGGUFConfigBuilder {
        HuggingFaceGGUFConfigBuilder::new()
    }
}

/// Get default cache directory: `~/.cache/huggingface`
fn default_cache_dir() -> PathBuf {
    // Use huggingface_hub's default cache location
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

/// Builder for [`HuggingFaceGGUFConfig`].
#[derive(Debug, Clone)]
pub struct HuggingFaceGGUFConfigBuilder {
    config: HuggingFaceGGUFConfig,
}

impl Default for HuggingFaceGGUFConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HuggingFaceGGUFConfigBuilder {
    /// Create a new builder with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: HuggingFaceGGUFConfig::default(),
        }
    }

    /// Set the HuggingFace API token.
    #[must_use]
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.config.auth = Some(foundation_auth::AuthCredential::SecretOnly(
            foundation_auth::ConfidentialText::new(token.into()),
        ));
        self
    }

    /// Set the cache directory for downloaded models.
    #[must_use]
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.cache_dir = path.into();
        self
    }

    /// Set the default quantization (e.g., "q4_k_m", "q5_k_m").
    #[must_use]
    pub fn default_quantization(mut self, quant: impl Into<String>) -> Self {
        self.config.default_quantization = Some(quant.into());
        self
    }

    /// Set llama.cpp backend configuration.
    #[must_use]
    pub fn llama_config(mut self, config: LlamaBackendConfig) -> Self {
        self.config.llama_config = config;
        self
    }

    /// Set the number of GPU layers to offload.
    #[must_use]
    pub fn n_gpu_layers(mut self, n: u32) -> Self {
        self.config.llama_config.n_gpu_layers = n;
        self
    }

    /// Set the number of CPU threads to use.
    #[must_use]
    pub fn n_threads(mut self, n: usize) -> Self {
        self.config.llama_config.n_threads = n;
        self
    }

    /// Set the context length.
    #[must_use]
    pub fn context_length(mut self, n: usize) -> Self {
        self.config.llama_config.context_length = n;
        self
    }

    /// Set the llama.cpp backend variant.
    #[must_use]
    pub fn llama_backend(mut self, backend: LlamaBackends) -> Self {
        self.config.llama_backend = backend;
        self
    }

    /// Build the final config.
    #[must_use]
    pub fn build(self) -> HuggingFaceGGUFConfig {
        self.config
    }
}

/// Parsed HuggingFace model identifier.
#[derive(Debug, Clone)]
pub struct ParsedModelId {
    /// Repository ID (e.g., "TheBloke/Llama-2-7B-GGUF").
    pub repo_id: String,
    /// Quantization name (e.g., "q4_k_m"), if specified.
    pub quantization: Option<String>,
    /// Revision (branch/tag/commit), defaults to "main".
    pub revision: String,
}

impl HuggingFaceGGUFProvider {
    /// Create a new provider with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the HFClient cannot be initialized.
    pub fn new(config: HuggingFaceGGUFConfig) -> ModelProviderResult<Self> {
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

        // Ensure cache directory exists
        std::fs::create_dir_all(&config.cache_dir).map_err(|e| {
            ModelProviderErrors::FailedFetching(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create cache directory: {e}"),
            )))
        })?;

        Ok(Self {
            hf_client,
            llama_backend: config.llama_backend,
            cache_dir: config.cache_dir,
            default_quantization: config.default_quantization,
        })
    }

    /// Parse a ModelId into HuggingFace repo_id, quantization, and revision.
    ///
    /// When ModelId::Name contains a Quantization variant, it's converted to the
    /// corresponding GGUF filename pattern (e.g., Quantization::Q2K -> "Q2_K").
    ///
    /// # Examples
    ///
    /// ```
    /// # use foundation_ai::backends::huggingface_provider::HuggingFaceGGUFProvider;
    /// // ModelId::Name("repo".to_string(), Some(Quantization::Q2K)) -> quant="Q2_K"
    /// // ModelId::Name("repo".to_string(), Some(Quantization::Q4_KM)) -> quant="Q4_K_M"
    /// ```
    #[must_use]
    pub fn parse_model_id(&self, model_id: &ModelId) -> Option<ParsedModelId> {
        let (name, provided_quant) = match model_id {
            ModelId::Name(name, quant) => (name.as_str(), quant.as_ref()),
            _ => return None,
        };

        // Split by colon to extract revision and quantization from string
        let parts: Vec<&str> = name.split(':').collect();

        let (repo_id, revision, string_quantization) = match parts.as_slice() {
            [repo] => {
                // Just repo ID, use default quantization
                (repo.to_string(), "main".to_string(), None)
            }
            [repo, quant_or_rev] => {
                // Could be repo:quant or repo:revision
                // Heuristic: if second part looks like a quantization, use it
                if looks_like_quantization(quant_or_rev) {
                    (
                        repo.to_string(),
                        "main".to_string(),
                        Some(quant_or_rev.to_string()),
                    )
                } else {
                    // Treat as revision, use default quantization
                    (repo.to_string(), quant_or_rev.to_string(), None)
                }
            }
            [repo, rev, quant] => {
                // Full: repo:revision:quantization
                (repo.to_string(), rev.to_string(), Some(quant.to_string()))
            }
            _ => return None,
        };

        // Priority: ModelId Quantization > string quantization > default
        let final_quant = if let Some(quant) = provided_quant {
            // Convert Quantization enum to GGUF filename format
            Some(quant.to_filename_format())
        } else if let Some(q) = string_quantization {
            Some(q)
        } else {
            self.default_quantization.clone()
        };

        Some(ParsedModelId {
            repo_id,
            revision,
            quantization: final_quant,
        })
    }

    /// Construct GGUF filename pattern from quantization string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use foundation_ai::backends::huggingface_provider::HuggingFaceGGUFProvider;
    /// // "q4_k_m" -> "*Q4_K_M.gguf"
    /// // "q2_k" -> "*Q2_K.gguf"
    /// ```
    #[must_use]
    pub fn quantization_to_filename_pattern(quantization: &str) -> String {
        // Convert to uppercase and add wildcards for matching
        // e.g., "q4_k_m" -> "*Q4_K_M.gguf", "q2_k" -> "*Q2_K.gguf"
        format!("*{}.gguf", quantization.to_uppercase())
    }

    /// Check if a GGUF file exists in cache for the given repo and quantization.
    fn find_cached_file(&self, repo_id: &str, quantization: Option<&str>) -> Option<PathBuf> {
        let repo_path = self.cache_dir.join(repo_id.replace('/', "--"));

        if !repo_path.exists() {
            return None;
        }

        // Search for matching GGUF file
        let pattern = quantization
            .map(Self::quantization_to_filename_pattern)
            .unwrap_or_else(|| "*.gguf".to_string());

        if let Ok(entries) = std::fs::read_dir(&repo_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "gguf") {
                    let filename = path.file_name()?.to_str()?;
                    if pattern_matches(&pattern, filename) {
                        return Some(path);
                    }
                }
            }
        }

        None
    }

    /// Download a GGUF file from HuggingFace Hub.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails or the repository/file is not found.
    pub fn download_model(&self, parsed: &ParsedModelId) -> ModelProviderResult<PathBuf> {
        // Check cache first
        if let Some(cached_path) =
            self.find_cached_file(&parsed.repo_id, parsed.quantization.as_deref())
        {
            tracing::debug!("Found cached model: {:?}", cached_path);
            return Ok(cached_path);
        }

        tracing::trace!(
            "---------------Downloading model {} (revision: {}, quantization: {:?})",
            parsed.repo_id,
            parsed.revision,
            parsed.quantization
        );

        // Create repository handle
        let repo = self.hf_client.model(
            parsed.repo_id.split('/').next().unwrap_or("").to_string(),
            parsed
                .repo_id
                .split('/')
                .skip(1)
                .collect::<Vec<_>>()
                .join("/"),
        );

        // Determine filename to download
        let filename = if let Some(quant) = &parsed.quantization {
            // Try to find the exact quantization file
            tracing::trace!("------------get model revision for quantization");
            find_gguf_file_in_repo(&repo, &parsed.revision, quant)?
        } else {
            // Use default pattern
            "*.gguf".to_string()
        };

        // Create destination directory
        let dest_dir = self.cache_dir.join(parsed.repo_id.replace('/', "--"));
        std::fs::create_dir_all(&dest_dir).map_err(|e| {
            ModelProviderErrors::FailedFetching(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create destination directory: {e}"),
            )))
        })?;

        // Download the file
        let params = RepoDownloadFileParams {
            revision: Some(parsed.revision.clone()),
            filename: filename.clone(),
            directory: dest_dir.clone(),
        };

        tracing::trace!("------------download model file");
        let downloaded_path = repository::repo_download_file(&repo, &params).map_err(|e| {
            ModelProviderErrors::FailedFetching(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to download model: {e}"),
            )))
        })?;

        tracing::info!("Downloaded model to: {:?}", downloaded_path);
        Ok(downloaded_path)
    }
}

impl ModelProvider for HuggingFaceGGUFProvider {
    type Config = HuggingFaceGGUFConfig;
    type Model = LlamaModels;

    fn create(
        self,
        config: Option<Self::Config>,
    ) -> ModelProviderResult<Self>
    where
        Self: Sized,
    {
        // If config is provided, recreate with new config
        if let Some(config) = config {
            HuggingFaceGGUFProvider::new(config)
        } else {
            Ok(self)
        }
    }

    fn describe(&self) -> ModelProviderResult<crate::types::ModelProviderDescriptor> {
        let descriptor = crate::types::ModelProviderDescriptor {
            id: "huggingface".to_string(),
            name: "HuggingFace Hub (llama.cpp)".to_string(),
            reasoning: false,
            api: crate::types::ModelAPI::Custom("huggingface-hub".to_string()),
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
        };
        Ok(descriptor)
    }

    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model> {
        // Parse the ModelId
        let parsed = self.parse_model_id(&model_id).ok_or_else(|| {
            ModelProviderErrors::NotFound(format!(
                "Invalid HuggingFace ModelId: {model_id:?}. Expected format: 'owner/repo[:quantization]'"
            ))
        })?;

        // Download or find cached model
        let model_path = self.download_model(&parsed)?;

        // Create ModelSpec for llama.cpp
        let model_spec = ModelSpec {
            name: parsed.repo_id.clone(),
            id: model_id.clone(),
            devices: None,
            model_location: Some(model_path),
            lora_location: None,
        };

        // Delegate to LlamaBackends for actual model loading
        self.llama_backend.get_model_by_spec(model_spec)
    }

    fn get_model_by_spec(&self, _model_spec: ModelSpec) -> ModelProviderResult<Self::Model> {
        // For HuggingFace provider, we always use get_model with ModelId
        // This method is not directly supported
        Err(ModelProviderErrors::NotFound(
            "Use get_model with HuggingFace ModelId syntax instead".to_string(),
        ))
    }

    fn get_one(&self, _model_id: ModelId) -> ModelProviderResult<crate::types::ModelSpec> {
        Err(ModelProviderErrors::NotFound(
            "Model catalog not implemented".to_string(),
        ))
    }

    fn get_all(&self, _model_id: ModelId) -> ModelProviderResult<Vec<crate::types::ModelSpec>> {
        Err(ModelProviderErrors::NotFound(
            "Model catalog not implemented".to_string(),
        ))
    }
}

/// Check if a string looks like a quantization identifier.
fn looks_like_quantization(s: &str) -> bool {
    // Common quantization patterns: q4_k_m, q5_k_s, q8_0, etc.
    let s_lower = s.to_lowercase();
    s_lower.starts_with('q') && (s_lower.contains("_k_") || s_lower.contains('_'))
}

/// Check if a filename matches a quantization pattern.
/// Does partial matching - extracts the quantization part from both pattern and filename.
fn pattern_matches(pattern: &str, filename: &str) -> bool {
    // Extract just the filename from the path if it contains slashes
    let basename = filename.rsplit('/').next().unwrap_or(filename);

    // Remove .gguf extension from both for comparison
    let pattern_no_ext = pattern.strip_suffix(".gguf").unwrap_or(pattern);
    let basename_no_ext = basename.strip_suffix(".gguf").unwrap_or(basename);

    // Extract the quantization pattern (e.g., "*Q2_K" -> "Q2_K")
    let pattern_quant = pattern_no_ext.strip_prefix('*').unwrap_or(pattern_no_ext);

    // Check if the basename contains the quantization pattern
    basename_no_ext.contains(pattern_quant)
}

/// Find a GGUF file in a repository matching the quantization.
fn find_gguf_file_in_repo(
    repo: &HFRepository,
    revision: &str,
    quantization: &str,
) -> ModelProviderResult<String> {
    use foundation_core::valtron::Stream;
    use foundation_deployment::providers::huggingface::RepoListTreeParams;

    let params = RepoListTreeParams {
        revision: Some(revision.to_string()),
        recursive: Some(true),
        limit: None,
    };

    // List files in the repository
    let tree =
        foundation_deployment::providers::huggingface::repository::repo_list_tree(repo, &params)
            .map_err(|e| {
                ModelProviderErrors::FailedFetching(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to list repository files: {e}"),
                )))
            })?;

    // Collect all entries from the stream
    let entries: Vec<_> = tree
        .filter_map(|s| match s {
            Stream::Next(Ok(entry)) => Some(entry),
            Stream::Next(Err(_)) => None,
            _ => None,
        })
        .collect();

    // Find matching GGUF file
    let pattern = HuggingFaceGGUFProvider::quantization_to_filename_pattern(quantization);

    for entry in &entries {
        if let foundation_deployment::providers::huggingface::RepoTreeEntry::File { path, .. } =
            entry
        {
            if path.ends_with(".gguf") && pattern_matches(&pattern, path) {
                tracing::info!(
                    "Found relevant GGUF model for pattern: {} to be: {}",
                    &pattern,
                    &path,
                );
                return Ok(path.clone());
            }
        }
    }

    // If no exact match, try to find any GGUF file
    for entry in &entries {
        if let foundation_deployment::providers::huggingface::RepoTreeEntry::File { path, .. } =
            entry
        {
            if path.ends_with(".gguf") {
                tracing::warn!(
                    "Exact quantization {} not found, using: {}",
                    quantization,
                    path
                );
                return Ok(path.clone());
            }
        }
    }

    Err(ModelProviderErrors::NotFound(format!(
        "No GGUF file matching quantization '{}' found in repository",
        quantization
    )))
}
