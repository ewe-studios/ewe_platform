//! The two model paths of the demo.
//!
//! | Tab | Path | How |
//! |---|---|---|
//! | 1, 3 | Cloudflare Workers AI | The `AI` binding of the Worker; no key in the page |
//! | 2 | OpenRouter | The agent-rpc catalog and registry with the OpenRouter provider |
//!
//! A path whose secret or binding is absent answers a message that names the
//! missing value, so a tab and the live test show it plainly.
//!
//! A third path, an agent-rpc container host that serves a GGUF file, waits for
//! issue #1276.

use std::cell::RefCell;
use std::sync::Arc;

use foundation_agent_rpc::core::config::{RegistryConfig, TurnLimits};
use foundation_agent_rpc::core::model::{ApiModelConfig, ModelRequest, RemoteCatalog};
use foundation_agent_rpc::core::registry::{
    Caller, InMemoryStores, InstanceRegistry, InstanceSpec, LocalInstanceRegistry, ScopePolicy,
};
use foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event;
use foundation_agent_rpc::AgentRpcError;
use foundation_auth::client_auth::{AuthMethod, Principal};
use foundation_context::Ctx;
use futures::StreamExt;
use worker::{Env, Response};

use crate::config::DemoConfig;
use crate::sandbox;

/// The model path of one tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelPath {
    /// The Cloudflare Workers AI binding.
    WorkersAi,
    /// OpenRouter through the agent-rpc catalog.
    OpenRouter,
}

impl ModelPath {
    /// The path of the tab at `index` (zero-based). The two paths alternate.
    #[must_use]
    pub fn for_tab(index: usize) -> Self {
        if index % 2 == 0 {
            Self::WorkersAi
        } else {
            Self::OpenRouter
        }
    }

    /// The stable name that the API and the live test read.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkersAi => "workers-ai",
            Self::OpenRouter => "openrouter",
        }
    }

    /// The label that the page shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::WorkersAi => "Cloudflare Workers AI (AI binding)",
            Self::OpenRouter => "OpenRouter (agent RPC)",
        }
    }

    /// The model id, from configuration.
    #[must_use]
    pub fn model_name(self, config: &DemoConfig) -> String {
        match self {
            Self::WorkersAi => config.workers_ai_model.clone(),
            Self::OpenRouter => config.openrouter_model.clone(),
        }
    }

    /// The service that runs the model. The page shows this in the answer
    /// metadata line.
    #[must_use]
    pub fn service(self) -> &'static str {
        match self {
            Self::WorkersAi => "Workers AI",
            Self::OpenRouter => "OpenRouter",
        }
    }

    /// The address that the path calls. The Workers AI path calls the Worker
    /// binding, so its address is the binding, not a URL.
    #[must_use]
    pub fn address(self, config: &DemoConfig) -> String {
        match self {
            Self::WorkersAi => format!("binding {}", config.ai_binding),
            Self::OpenRouter => openrouter_address(&config.openrouter_base_url),
        }
    }

    /// The protocol of the address. The OpenRouter protocol follows the
    /// configured base URL.
    #[must_use]
    pub fn protocol(self, config: &DemoConfig) -> &'static str {
        match self {
            Self::WorkersAi => "HTTPS",
            Self::OpenRouter => {
                if config.openrouter_base_url.starts_with("http://") {
                    "HTTP"
                } else {
                    "HTTPS"
                }
            }
        }
    }
}

/// The OpenRouter endpoint without the provider suffix. The provider adds
/// `/v1/chat/completions`, so the base ends at `/v1`.
fn openrouter_address(base: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.ends_with("/v1") {
        base.to_string()
    } else {
        format!("{base}/v1")
    }
}

thread_local! {
    /// The one OpenRouter registry of this isolate. A Worker isolate runs on
    /// one thread, so the registry stays here and is reused by each request.
    static OPENROUTER: RefCell<Option<Result<Arc<LocalInstanceRegistry>, String>>> =
        const { RefCell::new(None) };
}

/// One agent turn on the Workers AI binding. The binding answers one whole
/// text, so the response holds one chunk.
///
/// # Errors
/// A message that names the missing binding or the failed turn.
pub async fn workers_ai_turn(
    env: &Env,
    config: &DemoConfig,
    prompt: &str,
) -> Result<Response, String> {
    let ai = env
        .ai(&config.ai_binding)
        .map_err(|error| format!("the {} binding is absent: {error}", config.ai_binding))?;
    let output: serde_json::Value = ai
        .run(
            &config.workers_ai_model,
            serde_json::json!({ "prompt": prompt }),
        )
        .await
        .map_err(|error| format!("the Workers AI turn failed: {error}"))?;
    let text = output
        .get("response")
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| output.to_string(), str::to_string);
    let stream =
        futures::stream::once(async move { Ok::<Vec<u8>, worker::Error>(text.into_bytes()) });
    sandbox::text_response(stream)
}

/// One agent turn through the OpenRouter catalog and the agent-rpc registry.
/// The answer streams.
///
/// # Errors
/// A message that names the missing secret or the failed turn.
pub async fn openrouter_turn(
    env: &Env,
    config: &DemoConfig,
    ctx: &Ctx,
    sandbox_id: &str,
    prompt: &str,
) -> Result<Response, String> {
    let registry = openrouter_registry(env, config, ctx)?;
    let caller = caller();
    let model = ModelRequest {
        name: Some(config.openrouter_model.clone()),
        kind: None,
        device: None,
    };
    match registry.get(&caller, sandbox_id).await {
        Ok(_) => {}
        Err(error) if matches!(error.current_context(), AgentRpcError::InstanceNotFound(_)) => {
            let spec = InstanceSpec {
                name: sandbox_id.to_string(),
                model,
                system_prompt: Some(config.agent_system_prompt.clone()),
                max_tokens: None,
                temperature: None,
            };
            registry
                .create(&caller, spec)
                .await
                .map_err(|error| format!("the OpenRouter instance: {}", error.current_context()))?;
        }
        Err(error) => {
            return Err(format!(
                "the OpenRouter instance lookup: {}",
                error.current_context()
            ))
        }
    }
    let stream = registry
        .start_turn(&caller, sandbox_id, prompt.to_string())
        .await
        .map_err(|error| format!("the OpenRouter turn: {}", error.current_context()))?;
    let stream = stream.map(|item| match item {
        Ok(event) => match event.event {
            Some(Event::Text(text)) => Ok::<Vec<u8>, worker::Error>(text.into_bytes()),
            Some(Event::Failed(failed)) => {
                Ok(format!("\n[error: {}]\n", failed.message).into_bytes())
            }
            _ => Ok(Vec::new()),
        },
        Err(error) => Ok(format!("\n[error: {}]\n", error.current_context()).into_bytes()),
    });
    sandbox::text_response(stream)
}

/// Ends the OpenRouter conversation of one sandbox, when there is one.
pub async fn close_openrouter(sandbox_id: &str) {
    let Some(Ok(registry)) = OPENROUTER.with(|slot| slot.borrow().as_ref().cloned()) else {
        return;
    };
    let caller = caller();
    let _ = registry.destroy(&caller, sandbox_id).await;
}

/// The one OpenRouter registry of this isolate, or the reason it did not build.
fn openrouter_registry(
    env: &Env,
    config: &DemoConfig,
    ctx: &Ctx,
) -> Result<Arc<LocalInstanceRegistry>, String> {
    OPENROUTER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(build_openrouter(env, config, ctx));
        }
        match slot.as_ref() {
            Some(Ok(registry)) => Ok(Arc::clone(registry)),
            Some(Err(error)) => Err(error.clone()),
            None => Err("the OpenRouter registry did not build".to_string()),
        }
    })
}

/// Builds the OpenRouter catalog and registry.
fn build_openrouter(
    env: &Env,
    config: &DemoConfig,
    ctx: &Ctx,
) -> Result<Arc<LocalInstanceRegistry>, String> {
    let key = env
        .secret(&config.openrouter_secret)
        .map(|value| value.to_string())
        .map_err(|_| {
            format!(
                "the Worker secret {} is not set; set it with `wrangler secret put {}`",
                config.openrouter_secret, config.openrouter_secret
            )
        })?;
    if key.trim().is_empty() {
        return Err(format!(
            "the Worker secret {} is empty",
            config.openrouter_secret
        ));
    }
    let model = ApiModelConfig {
        name: config.openrouter_model.clone(),
        provider: "openrouter".to_string(),
        model_id: config.openrouter_model.clone(),
        base_url: Some(config.openrouter_base_url.clone()),
        api_key_secret: config.openrouter_secret.clone(),
        account_id: None,
    };
    let http = foundation_netio::HttpClientBuilder::new().build();
    let secrets = |name: &str| {
        env.secret(name)
            .ok()
            .map(|value| value.to_string())
            .filter(|value| !value.trim().is_empty())
    };
    let catalog = RemoteCatalog::from_config(&[model], &secrets, &http, ctx)
        .map_err(|error| format!("the OpenRouter catalog: {}", error.current_context()))?;
    let limits = TurnLimits::default();
    let registry = LocalInstanceRegistry::new(
        Arc::new(catalog),
        InMemoryStores,
        RegistryConfig::default().with_scope_policy(ScopePolicy::Shared),
        limits,
        ctx.clone(),
    )
    .map_err(|error| format!("the OpenRouter registry: {}", error.current_context()))?;
    Ok(Arc::new(registry))
}

/// The one caller of the demo: a shared scope with a fixed local subject.
fn caller() -> Caller {
    ScopePolicy::Shared.caller(Principal {
        subject: "demo".to_string(),
        method: AuthMethod::SignedSecret {
            secret_id: "demo".to_string(),
        },
        roles: vec!["demo".to_string()],
    })
}
