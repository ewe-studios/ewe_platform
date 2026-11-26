//! The model client and the model configuration of the model slave.
//!
//! `WorkerModelClient` is the wasm adapter of feature 07: the Cloudflare Model
//! API uses the `AI` binding, and `OpenRouter` sends one Hypertext Transfer
//! Protocol (HTTP) request with the key from the `Ctx` environment entry. This
//! module selects the backend from the Worker environment.

use foundation_context::Ctx;
use foundation_sandbox::shared::cloudflare::{ModelBackend, ModelConfig, ModelSecrets};
use foundation_sandbox::shared::Secret;

/// The `Ctx` entry name and the Worker secret name of the OpenRouter key.
pub const MODEL_API_KEY_ENTRY: &str = "MODEL_API_KEY";

/// The wasm model client of feature 07.
pub use foundation_sandbox::cloudflare::WorkerModelClient;

/// The model configuration from the Worker environment.
///
/// The variables are `MODEL_BACKEND` (`openrouter` or `cloudflare-ai`),
/// `MODEL_NAME` and, for `openrouter`, `MODEL_BASE_URL`. An absent or unknown
/// backend gives a configuration with no backend, so a turn answers
/// `UnsupportedCapability` instead of guessing a model.
#[must_use]
pub fn model_config_from_env(env: &worker::Env) -> ModelConfig {
    let config = ModelConfig::new();
    let Some(backend) = variable(env, "MODEL_BACKEND") else {
        return config;
    };
    match backend.as_str() {
        "cloudflare-ai" | "cloudflare_ai" => {
            let model = variable(env, "MODEL_NAME").unwrap_or_default();
            config.with_backend(Some(ModelBackend::CloudflareAi { model }))
        }
        "openrouter" => {
            let base_url = variable(env, "MODEL_BASE_URL").unwrap_or_default();
            let model = variable(env, "MODEL_NAME").unwrap_or_default();
            config
                .with_backend(Some(ModelBackend::OpenRouter { base_url, model }))
                .with_api_key_entry(Some(MODEL_API_KEY_ENTRY.to_string()))
        }
        _ => config,
    }
}

/// Stores the model key of `env` in `ctx`, when the environment has one.
///
/// The key never enters a record or a log. The Cloudflare Model API uses the
/// `AI` binding and needs no key, so an absent key is not an error.
pub fn store_model_key(env: &worker::Env, ctx: &Ctx) {
    if let Ok(key) = env.secret(MODEL_API_KEY_ENTRY) {
        ctx.store(ModelSecrets::new().with_key(MODEL_API_KEY_ENTRY, Secret::new(key.to_string())));
    }
}

/// One text variable of the environment.
fn variable(env: &worker::Env, name: &str) -> Option<String> {
    env.var(name).ok().map(|value| value.to_string())
}
