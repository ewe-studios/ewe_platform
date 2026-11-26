//! The catalog of remote (API) models.
//!
//! WHY: a Worker and a native host can serve models that run at a remote
//! provider. Nothing loads, so the catalog returns a stored handle.
//!
//! WHAT: [`ApiModelConfig`] (one model from configuration) and
//! [`RemoteCatalog`] (the handles, ready to serve).
//!
//! HOW: each provider becomes a [`ProviderRouter`] with one provider. The
//! handle holds that router. A turn then reaches the provider through the
//! router, and no model is loaded on this host.

use std::sync::Arc;

use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::backends::workers_ai::{WorkersAiConfig, WorkersAiProvider};
use foundation_ai::types::{ModelId, ModelProvider, ProviderRouter};
use foundation_context::Ctx;
use foundation_netio::DynNetClient;
use serde::Deserialize;

use crate::core::model::catalog::{
    AgentRpcFuture, CatalogEntry, KindSupport, LoadState, ModelCapabilities, ModelCatalog,
    ModelDescriptor,
};
use crate::core::model::{Device, ModelHandle, ModelKind, ModelRequest};
use crate::error::{AgentRpcError, AgentRpcResult};

/// One remote model, from configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiModelConfig {
    /// The name that clients use.
    pub name: String,
    /// "openai", "openrouter", "anthropic" or "workers-ai".
    pub provider: String,
    /// The model ID at the provider, for example
    /// "@cf/meta/llama-3.1-8b-instruct".
    pub model_id: String,
    /// The provider base URL, when it is not the provider default.
    #[serde(default)]
    pub base_url: Option<String>,
    /// The name of the secret that holds the API key (never the key itself).
    pub api_key_secret: String,
    /// The Cloudflare account ID. Required for "workers-ai".
    #[serde(default)]
    pub account_id: Option<String>,
}

/// Remote models only. Nothing loads; `resolve` returns a stored handle.
pub struct RemoteCatalog {
    entries: Vec<(CatalogEntry, ModelHandle)>,
}

impl std::fmt::Debug for RemoteCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteCatalog")
            .field(
                "models",
                &self
                    .entries
                    .iter()
                    .map(|(entry, _)| &entry.name)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl RemoteCatalog {
    /// Build one [`ProviderRouter`] per model over `http`.
    ///
    /// `secrets` reads a secret by name. It never returns a key that the
    /// caller did not name in the configuration.
    ///
    /// # Errors
    /// `AgentRpcError::Config` for an unknown provider or a missing secret.
    pub fn from_config(
        models: &[ApiModelConfig],
        secrets: &dyn Fn(&str) -> Option<String>,
        http: &DynNetClient,
        ctx: &Ctx,
    ) -> AgentRpcResult<Self> {
        let mut entries = Vec::with_capacity(models.len());
        for model in models {
            let key = secrets(&model.api_key_secret).ok_or_else(|| {
                AgentRpcError::Config(format!(
                    "model {}: the secret {} is not set",
                    model.name, model.api_key_secret
                ))
            })?;
            let router = router_for(model, &key, Arc::clone(http), ctx)?;
            let entry = CatalogEntry {
                name: model.name.clone(),
                kind: ModelKind::Api,
                device: Device::Remote,
            };
            let handle = ModelHandle::from_router(
                model.name.clone(),
                ModelKind::Api,
                Device::Remote,
                ModelId::from(model.model_id.as_str()),
                router,
            );
            entries.push((entry, handle));
        }
        Ok(Self { entries })
    }

    /// The configured entries, in configuration order.
    #[must_use]
    pub fn entries(&self) -> &[(CatalogEntry, ModelHandle)] {
        &self.entries
    }
}

impl ModelCatalog for RemoteCatalog {
    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            kinds: vec![KindSupport {
                kind: ModelKind::Api,
                devices: vec![Device::Remote],
            }],
            models: self
                .entries
                .iter()
                .map(|(entry, _)| ModelDescriptor {
                    name: entry.name.clone(),
                    kind: entry.kind,
                    device: entry.device,
                    state: LoadState::Remote,
                })
                .collect(),
        }
    }

    fn resolve<'a>(&'a self, request: &'a ModelRequest) -> AgentRpcFuture<'a, ModelHandle> {
        Box::pin(async move {
            let entries: Vec<CatalogEntry> = self
                .entries
                .iter()
                .map(|(entry, _)| entry.clone())
                .collect();
            let capabilities = self.capabilities();
            let chosen =
                super::catalog::select(&entries, &capabilities.kinds, request).map_err(|code| {
                    AgentRpcError::ModelSelection {
                        code,
                        detail: code.message().to_string(),
                    }
                })?;
            self.entries
                .iter()
                .find(|(entry, _)| entry.name == chosen.name)
                .map(|(_, handle)| handle.clone())
                .ok_or_else(|| {
                    AgentRpcError::Config(format!(
                        "the catalog lost model {} between select and lookup",
                        chosen.name
                    ))
                    .into()
                })
        })
    }
}

/// One provider router for one configured remote model.
fn router_for(
    model: &ApiModelConfig,
    key: &str,
    http: DynNetClient,
    ctx: &Ctx,
) -> AgentRpcResult<ProviderRouter> {
    let provider = match model.provider.as_str() {
        "openai" => {
            let mut config = OpenAIConfig::api_key(key);
            if let Some(base_url) = &model.base_url {
                config = config.with_base_url(base_url);
            }
            let provider = OpenAIProvider::with_http_client(http, ctx.clone())
                .create(Some(config))
                .map_err(provider_error(&model.name))?;
            ProviderRouter::builder().provider(provider).build()
        }
        "openrouter" => {
            let mut config = OpenAIConfig::openrouter(key);
            if let Some(base_url) = &model.base_url {
                config = config.with_base_url(base_url);
            }
            let provider = OpenAIProvider::with_http_client(http, ctx.clone())
                .create(Some(config))
                .map_err(provider_error(&model.name))?;
            ProviderRouter::builder().provider(provider).build()
        }
        "anthropic" => {
            let mut config = AnthropicConfig::api_key(key);
            if let Some(base_url) = &model.base_url {
                config = config.with_base_url(base_url);
            }
            let provider = AnthropicMessagesProvider::with_http_client(http, ctx.clone())
                .create(Some(config))
                .map_err(provider_error(&model.name))?;
            ProviderRouter::builder().provider(provider).build()
        }
        "workers-ai" => {
            let account_id = model.account_id.as_deref().ok_or_else(|| {
                AgentRpcError::Config(format!(
                    "model {}: workers-ai needs an account_id",
                    model.name
                ))
            })?;
            let mut config = WorkersAiConfig::api_token(key).with_account_id(account_id);
            if let Some(base_url) = &model.base_url {
                config = config.with_base_url(base_url);
            }
            let provider = WorkersAiProvider::with_http_client(http, ctx.clone())
                .create(Some(config))
                .map_err(provider_error(&model.name))?;
            ProviderRouter::builder().provider(provider).build()
        }
        other => {
            return Err(AgentRpcError::Config(format!(
                "model {}: unknown provider {other}; use openai, openrouter, anthropic or workers-ai",
                model.name
            ))
            .into());
        }
    };
    Ok(provider)
}

fn provider_error(
    name: &str,
) -> impl FnOnce(foundation_ai::errors::ModelProviderErrors) -> AgentRpcError + '_ {
    move |error| AgentRpcError::Config(format!("model {name}: the provider is not valid: {error}"))
}
