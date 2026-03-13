//! WHY: Fetches model metadata from upstream APIs (models.dev, `OpenRouter`, Vercel AI Gateway)
//! and generates `backends/foundation_ai/src/models/model_descriptors.rs` so the static
//! model registry stays in sync with live provider catalogs.
//!
//! WHAT: A CLI subcommand that pulls JSON from three sources, normalises model entries,
//! applies overrides and static fallbacks, deduplicates, then writes a Rust source file
//! that compiles against `foundation_ai::types`.
//!
//! HOW: Uses `foundation_core::wire::simple_http::client::SimpleHttpClient` for synchronous
//! HTTP GET, `serde_json` for parsing, and a code-generation pass that emits const data
//! matching `ModelProviderDescriptor`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use derive_more::{Display, From};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, Status};
use serde::Deserialize;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

// Import the real types from foundation_ai so the compiler enforces that our
// codegen stays in sync with the actual struct definitions.
use foundation_ai::types::{MessageType, ModelProviderDescriptor, ModelUsageCosting};

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// WHY: Provides structured, actionable error reporting for every failure mode
/// in the model-generation pipeline.
///
/// WHAT: Covers HTTP transport, JSON parsing, body handling, and file I/O.
///
/// HOW: Uses `derive_more` for `Display`/`From` to avoid boilerplate.
///
/// # Panics
///
/// Never panics.
#[derive(Debug, Display, From)]
pub enum GenModelError {
    /// HTTP client could not be constructed or the request failed.
    #[display("http error for {url}: {source}")]
    Http {
        url: String,
        source: foundation_core::wire::simple_http::HttpClientError,
    },

    /// Server returned a non-200 status code.
    #[display("http {status} from {url}")]
    BadStatus { url: String, status: usize },

    /// Response body was not UTF-8.
    #[display("response body from {url} is not valid UTF-8: {source}")]
    InvalidUtf8 {
        url: String,
        source: std::string::FromUtf8Error,
    },

    /// Response body was an unexpected variant (e.g. stream instead of text).
    #[display("unexpected response body type from {url}")]
    UnexpectedBody { url: String },

    /// JSON deserialization failed.
    #[display("json parse error for {url}: {source}")]
    Json {
        url: String,
        source: serde_json::Error,
    },

    /// Could not write the generated file.
    #[display("failed to write {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },
}

impl std::error::Error for GenModelError {}

// ---------------------------------------------------------------------------
// Compile-time shape verification
// ---------------------------------------------------------------------------

/// WHY: Ensures the codegen template stays in lockstep with the real struct.
///
/// WHAT: Constructs a `ModelProviderDescriptor` using every field so that any
/// addition, removal, or rename causes a compile error in this binary.
///
/// HOW: Dead code — never called at runtime.
///
/// # Panics
///
/// Never panics.
#[allow(dead_code)]
fn verify_struct_shape() -> ModelProviderDescriptor {
    ModelProviderDescriptor {
        id: "",
        name: "",
        api: "",
        provider: "",
        base_url: "",
        reasoning: false,
        inputs: [MessageType::Text, MessageType::Text],
        cost: ModelUsageCosting {
            input: 0.0,
            output: 0.0,
            cach_read: 0.0,
            cach_write: 0.0,
        },
        context_window: 0,
        max_tokens: 0,
    }
}

// ---------------------------------------------------------------------------
// Intermediate model representation (owns Strings at runtime)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct ModelEntry {
    id: String,
    name: String,
    api: String,
    provider: String,
    base_url: String,
    reasoning: bool,
    has_image_input: bool,
    cost_input: f64,
    cost_output: f64,
    cost_cach_read: f64,
    cost_cach_write: f64,
    context_window: u32,
    max_tokens: u32,
}

// ---------------------------------------------------------------------------
// JSON shapes for the three upstream APIs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ModelsDevProvider {
    models: Option<BTreeMap<String, ModelsDevModel>>,
}

#[derive(Deserialize)]
struct ModelsDevModel {
    name: Option<String>,
    tool_call: Option<bool>,
    reasoning: Option<bool>,
    status: Option<String>,
    limit: Option<ModelsDevLimit>,
    cost: Option<ModelsDevCost>,
    modalities: Option<ModelsDevModalities>,
    provider: Option<ModelsDevProviderInfo>,
}

#[derive(Deserialize)]
struct ModelsDevLimit {
    context: Option<u64>,
    output: Option<u64>,
}

#[derive(Deserialize)]
struct ModelsDevCost {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
}

#[derive(Deserialize)]
struct ModelsDevModalities {
    input: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ModelsDevProviderInfo {
    npm: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Deserialize)]
struct OpenRouterModel {
    id: String,
    name: Option<String>,
    supported_parameters: Option<Vec<String>>,
    context_length: Option<u64>,
    architecture: Option<OpenRouterArch>,
    pricing: Option<OpenRouterPricing>,
    top_provider: Option<OpenRouterTopProvider>,
}

#[derive(Deserialize)]
struct OpenRouterArch {
    modality: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterPricing {
    prompt: Option<String>,
    completion: Option<String>,
    input_cache_read: Option<String>,
    input_cache_write: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterTopProvider {
    max_completion_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct AiGatewayResponse {
    data: Option<Vec<AiGatewayModel>>,
}

#[derive(Deserialize)]
struct AiGatewayModel {
    id: String,
    name: Option<String>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    tags: Option<Vec<String>>,
    pricing: Option<AiGatewayPricing>,
}

#[derive(Deserialize)]
struct AiGatewayPricing {
    input: Option<serde_json::Value>,
    output: Option<serde_json::Value>,
    input_cache_read: Option<serde_json::Value>,
    input_cache_write: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// HTTP helper
// ---------------------------------------------------------------------------

/// WHY: Centralises HTTP GET + JSON parse so each fetcher stays focused on
/// its domain logic.
///
/// WHAT: Sends a GET request, checks for 200, extracts the body text, and
/// deserialises into `T`.
///
/// HOW: Uses `SimpleHttpClient` from `foundation_core`.
///
/// # Errors
///
/// Returns `GenModelError::Http` on transport failure, `GenModelError::BadStatus`
/// on non-200, `GenModelError::InvalidUtf8` / `GenModelError::UnexpectedBody` on
/// body issues, and `GenModelError::Json` on parse failure.
///
/// # Panics
///
/// Never panics.
fn http_get_json<T: serde::de::DeserializeOwned>(
    client: &SimpleHttpClient,
    url: &str,
) -> Result<T, GenModelError> {
    let response = client
        .get(url)
        .map_err(|e| GenModelError::Http { url: url.to_string(), source: e })?
        .header(SimpleHeader::ACCEPT, "application/json")
        .header(SimpleHeader::USER_AGENT, "gen_model_descriptors/0.1 (ewe-platform)")
        .build_client()
        .map_err(|e| GenModelError::Http { url: url.to_string(), source: e })?
        .send()
        .map_err(|e| GenModelError::Http { url: url.to_string(), source: e })?;

    let status = response.get_status();
    if status != Status::OK {
        return Err(GenModelError::BadStatus {
            url: url.to_string(),
            status: status.into_usize(),
        });
    }

    let body_text = match response.get_body_ref() {
        SendSafeBody::Text(t) => t.clone(),
        SendSafeBody::Bytes(b) => String::from_utf8(b.clone())
            .map_err(|e| GenModelError::InvalidUtf8 { url: url.to_string(), source: e })?,
        _ => return Err(GenModelError::UnexpectedBody { url: url.to_string() }),
    };

    serde_json::from_str(&body_text)
        .map_err(|e| GenModelError::Json { url: url.to_string(), source: e })
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn has_image(modalities: Option<&ModelsDevModalities>) -> bool {
    modalities
        .and_then(|m| m.input.as_ref())
        .is_some_and(|v| v.iter().any(|s| s == "image"))
}

fn dev_cost(c: Option<&ModelsDevCost>) -> (f64, f64, f64, f64) {
    match c {
        Some(c) => (
            c.input.unwrap_or(0.0),
            c.output.unwrap_or(0.0),
            c.cache_read.unwrap_or(0.0),
            c.cache_write.unwrap_or(0.0),
        ),
        None => (0.0, 0.0, 0.0, 0.0),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn ctx(limit: Option<&ModelsDevLimit>, default: u32) -> u32 {
    limit.and_then(|l| l.context).map_or(default, |v| v as u32)
}

#[allow(clippy::cast_possible_truncation)]
fn max_tok(limit: Option<&ModelsDevLimit>, default: u32) -> u32 {
    limit.and_then(|l| l.output).map_or(default, |v| v as u32)
}

fn pricing_val(v: Option<&serde_json::Value>) -> f64 {
    match v {
        Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(serde_json::Value::String(s)) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn entry(
    id: &str, name: &str, api: &str, provider: &str, base_url: &str,
    reasoning: bool, has_image_input: bool, cost: (f64, f64, f64, f64),
    context_window: u32, max_tokens: u32,
) -> ModelEntry {
    ModelEntry {
        id: id.to_string(),
        name: name.to_string(),
        api: api.to_string(),
        provider: provider.to_string(),
        base_url: base_url.to_string(),
        reasoning,
        has_image_input,
        cost_input: cost.0,
        cost_output: cost.1,
        cost_cach_read: cost.2,
        cost_cach_write: cost.3,
        context_window,
        max_tokens,
    }
}

fn from_dev(
    model_id: &str, m: &ModelsDevModel, api: &str, provider: &str, base_url: &str,
    default_ctx: u32, default_max: u32,
) -> ModelEntry {
    let (ci, co, cr, cw) = dev_cost(m.cost.as_ref());
    entry(
        model_id, m.name.as_deref().unwrap_or(model_id),
        api, provider, base_url,
        m.reasoning == Some(true), has_image(m.modalities.as_ref()),
        (ci, co, cr, cw), ctx(m.limit.as_ref(), default_ctx), max_tok(m.limit.as_ref(), default_max),
    )
}

// ---------------------------------------------------------------------------
// Source fetchers
// ---------------------------------------------------------------------------

fn fetch_models_dev(client: &SimpleHttpClient) -> Vec<ModelEntry> {
    tracing::info!("Fetching models from models.dev API...");
    let data: serde_json::Value = match http_get_json(client, "https://models.dev/api.json") {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to fetch models.dev: {e}");
            return Vec::new();
        }
    };

    let mut models = Vec::new();

    struct Cfg { key: &'static str, api: &'static str, provider: &'static str, base_url: &'static str, default_ctx: u32, default_max: u32 }

    let providers = [
        Cfg { key: "amazon-bedrock", api: "bedrock-converse-stream", provider: "amazon-bedrock",
              base_url: "https://bedrock-runtime.us-east-1.amazonaws.com", default_ctx: 4096, default_max: 4096 },
        Cfg { key: "anthropic", api: "anthropic-messages", provider: "anthropic",
              base_url: "https://api.anthropic.com", default_ctx: 4096, default_max: 4096 },
        Cfg { key: "google", api: "google-generative-ai", provider: "google",
              base_url: "https://generativelanguage.googleapis.com/v1beta", default_ctx: 4096, default_max: 4096 },
        Cfg { key: "openai", api: "openai-responses", provider: "openai",
              base_url: "https://api.openai.com/v1", default_ctx: 4096, default_max: 4096 },
        Cfg { key: "groq", api: "openai-completions", provider: "groq",
              base_url: "https://api.groq.com/openai/v1", default_ctx: 4096, default_max: 4096 },
        Cfg { key: "cerebras", api: "openai-completions", provider: "cerebras",
              base_url: "https://api.cerebras.ai/v1", default_ctx: 4096, default_max: 4096 },
        Cfg { key: "xai", api: "openai-completions", provider: "xai",
              base_url: "https://api.x.ai/v1", default_ctx: 4096, default_max: 4096 },
        Cfg { key: "mistral", api: "openai-completions", provider: "mistral",
              base_url: "https://api.mistral.ai/v1", default_ctx: 4096, default_max: 4096 },
        Cfg { key: "huggingface", api: "openai-completions", provider: "huggingface",
              base_url: "https://router.huggingface.co/v1", default_ctx: 4096, default_max: 4096 },
    ];

    for cfg in &providers {
        let Some(provider_val) = data.get(cfg.key) else { continue };
        let Ok(p) = serde_json::from_value::<ModelsDevProvider>(provider_val.clone()) else { continue };
        let Some(provider_models) = p.models else { continue };

        for (model_id, m) in &provider_models {
            if m.tool_call != Some(true) { continue; }
            if cfg.key == "amazon-bedrock" {
                if model_id.starts_with("ai21.jamba") { continue; }
                if model_id.starts_with("mistral.mistral-7b-instruct-v0") { continue; }
            }
            models.push(from_dev(model_id, m, cfg.api, cfg.provider, cfg.base_url, cfg.default_ctx, cfg.default_max));
        }
    }

    // zAi
    if let Some(val) = data.get("zai") {
        if let Ok(p) = serde_json::from_value::<ModelsDevProvider>(val.clone()) {
            for (id, m) in p.models.iter().flat_map(|ms| ms.iter()) {
                if m.tool_call != Some(true) { continue; }
                models.push(from_dev(id, m, "openai-completions", "zai", "https://api.z.ai/api/coding/paas/v4", 4096, 4096));
            }
        }
    }

    // OpenCode Zen
    if let Some(val) = data.get("opencode") {
        if let Ok(p) = serde_json::from_value::<ModelsDevProvider>(val.clone()) {
            for (id, m) in p.models.iter().flat_map(|ms| ms.iter()) {
                if m.tool_call != Some(true) { continue; }
                if m.status.as_deref() == Some("deprecated") { continue; }
                let npm = m.provider.as_ref().and_then(|p| p.npm.as_deref());
                let (api, base_url) = match npm {
                    Some("@ai-sdk/openai") => ("openai-responses", "https://opencode.ai/zen/v1"),
                    Some("@ai-sdk/anthropic") => ("anthropic-messages", "https://opencode.ai/zen"),
                    Some("@ai-sdk/google") => ("google-generative-ai", "https://opencode.ai/zen/v1"),
                    _ => ("openai-completions", "https://opencode.ai/zen/v1"),
                };
                models.push(from_dev(id, m, api, "opencode", base_url, 4096, 4096));
            }
        }
    }

    // GitHub Copilot
    if let Some(val) = data.get("github-copilot") {
        if let Ok(p) = serde_json::from_value::<ModelsDevProvider>(val.clone()) {
            for (id, m) in p.models.iter().flat_map(|ms| ms.iter()) {
                if m.tool_call != Some(true) { continue; }
                if m.status.as_deref() == Some("deprecated") { continue; }
                let is_claude4 = id.starts_with("claude-haiku-4")
                    || id.starts_with("claude-sonnet-4")
                    || id.starts_with("claude-opus-4");
                let needs_responses = id.starts_with("gpt-5") || id.starts_with("oswe");
                let api = if is_claude4 { "anthropic-messages" }
                    else if needs_responses { "openai-responses" }
                    else { "openai-completions" };
                models.push(from_dev(id, m, api, "github-copilot", "https://api.individual.githubcopilot.com", 128_000, 8192));
            }
        }
    }

    // MiniMax variants
    for (key, provider, base_url) in [
        ("minimax", "minimax", "https://api.minimax.io/anthropic"),
        ("minimax-cn", "minimax-cn", "https://api.minimaxi.com/anthropic"),
    ] {
        if let Some(val) = data.get(key) {
            if let Ok(p) = serde_json::from_value::<ModelsDevProvider>(val.clone()) {
                for (id, m) in p.models.iter().flat_map(|ms| ms.iter()) {
                    if m.tool_call != Some(true) { continue; }
                    models.push(from_dev(id, m, "anthropic-messages", provider, base_url, 4096, 4096));
                }
            }
        }
    }

    // Kimi For Coding
    if let Some(val) = data.get("kimi-for-coding") {
        if let Ok(p) = serde_json::from_value::<ModelsDevProvider>(val.clone()) {
            for (id, m) in p.models.iter().flat_map(|ms| ms.iter()) {
                if m.tool_call != Some(true) { continue; }
                models.push(from_dev(id, m, "anthropic-messages", "kimi-coding", "https://api.kimi.com/coding", 4096, 4096));
            }
        }
    }

    tracing::info!("Loaded {} tool-capable models from models.dev", models.len());
    models
}

fn fetch_openrouter(client: &SimpleHttpClient) -> Vec<ModelEntry> {
    tracing::info!("Fetching models from OpenRouter API...");
    let data: OpenRouterResponse =
        match http_get_json(client, "https://openrouter.ai/api/v1/models") {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Failed to fetch OpenRouter: {e}");
                return Vec::new();
            }
        };

    let mut models = Vec::new();
    for m in &data.data {
        let has_tools = m.supported_parameters.as_ref()
            .map_or(false, |p| p.iter().any(|s| s == "tools"));
        if !has_tools { continue; }

        let img = m.architecture.as_ref()
            .and_then(|a| a.modality.as_deref())
            .map_or(false, |s| s.contains("image"));

        let parse = |s: &Option<String>| -> f64 {
            s.as_deref().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) * 1_000_000.0
        };
        let cost = (
            parse(&m.pricing.as_ref().and_then(|p| p.prompt.clone())),
            parse(&m.pricing.as_ref().and_then(|p| p.completion.clone())),
            parse(&m.pricing.as_ref().and_then(|p| p.input_cache_read.clone())),
            parse(&m.pricing.as_ref().and_then(|p| p.input_cache_write.clone())),
        );

        let reasoning = m.supported_parameters.as_ref()
            .map_or(false, |p| p.iter().any(|s| s == "reasoning"));

        models.push(entry(
            &m.id, m.name.as_deref().unwrap_or(&m.id),
            "openai-completions", "openrouter", "https://openrouter.ai/api/v1",
            reasoning, img, cost,
            m.context_length.unwrap_or(4096) as u32,
            m.top_provider.as_ref().and_then(|t| t.max_completion_tokens).unwrap_or(4096) as u32,
        ));
    }

    tracing::info!("Fetched {} tool-capable models from OpenRouter", models.len());
    models
}

fn fetch_ai_gateway(client: &SimpleHttpClient) -> Vec<ModelEntry> {
    tracing::info!("Fetching models from Vercel AI Gateway API...");
    let data: AiGatewayResponse =
        match http_get_json(client, "https://ai-gateway.vercel.sh/v1/models") {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Failed to fetch AI Gateway: {e}");
                return Vec::new();
            }
        };

    let mut models = Vec::new();
    for m in data.data.iter().flat_map(|d| d.iter()) {
        let tags = m.tags.as_deref().unwrap_or(&[]);
        if !tags.iter().any(|t| t == "tool-use") { continue; }

        let cost = (
            pricing_val(m.pricing.as_ref().and_then(|p| p.input.as_ref())) * 1_000_000.0,
            pricing_val(m.pricing.as_ref().and_then(|p| p.output.as_ref())) * 1_000_000.0,
            pricing_val(m.pricing.as_ref().and_then(|p| p.input_cache_read.as_ref())) * 1_000_000.0,
            pricing_val(m.pricing.as_ref().and_then(|p| p.input_cache_write.as_ref())) * 1_000_000.0,
        );

        models.push(entry(
            &m.id, m.name.as_deref().unwrap_or(&m.id),
            "anthropic-messages", "vercel-ai-gateway", "https://ai-gateway.vercel.sh",
            tags.iter().any(|t| t == "reasoning"),
            tags.iter().any(|t| t == "vision"),
            cost,
            m.context_window.unwrap_or(4096) as u32,
            m.max_tokens.unwrap_or(4096) as u32,
        ));
    }

    tracing::info!("Fetched {} tool-capable models from AI Gateway", models.len());
    models
}

// ---------------------------------------------------------------------------
// Static / fallback models
// ---------------------------------------------------------------------------

fn static_codex_models() -> Vec<ModelEntry> {
    let b = "https://chatgpt.com/backend-api";
    let (cw, mt) = (272_000, 128_000);
    vec![
        entry("gpt-5.1", "GPT-5.1", "openai-codex-responses", "openai-codex", b, true, true, (1.25,10.0,0.125,0.0), cw, mt),
        entry("gpt-5.1-codex-max", "GPT-5.1 Codex Max", "openai-codex-responses", "openai-codex", b, true, true, (1.25,10.0,0.125,0.0), cw, mt),
        entry("gpt-5.1-codex-mini", "GPT-5.1 Codex Mini", "openai-codex-responses", "openai-codex", b, true, true, (0.25,2.0,0.025,0.0), cw, mt),
        entry("gpt-5.2", "GPT-5.2", "openai-codex-responses", "openai-codex", b, true, true, (1.75,14.0,0.175,0.0), cw, mt),
        entry("gpt-5.2-codex", "GPT-5.2 Codex", "openai-codex-responses", "openai-codex", b, true, true, (1.75,14.0,0.175,0.0), cw, mt),
        entry("gpt-5.3-codex", "GPT-5.3 Codex", "openai-codex-responses", "openai-codex", b, true, true, (1.75,14.0,0.175,0.0), cw, mt),
        entry("gpt-5.3-codex-spark", "GPT-5.3 Codex Spark", "openai-codex-responses", "openai-codex", b, true, false, (0.0,0.0,0.0,0.0), 128_000, mt),
    ]
}

fn static_cloud_code_assist() -> Vec<ModelEntry> {
    let ep = "https://cloudcode-pa.googleapis.com";
    let (a, p) = ("google-gemini-cli", "google-gemini-cli");
    vec![
        entry("gemini-2.5-pro", "Gemini 2.5 Pro (Cloud Code Assist)", a, p, ep, true, true, (0.0,0.0,0.0,0.0), 1_048_576, 65535),
        entry("gemini-2.5-flash", "Gemini 2.5 Flash (Cloud Code Assist)", a, p, ep, true, true, (0.0,0.0,0.0,0.0), 1_048_576, 65535),
        entry("gemini-2.0-flash", "Gemini 2.0 Flash (Cloud Code Assist)", a, p, ep, false, true, (0.0,0.0,0.0,0.0), 1_048_576, 8192),
        entry("gemini-3-pro-preview", "Gemini 3 Pro Preview (Cloud Code Assist)", a, p, ep, true, true, (0.0,0.0,0.0,0.0), 1_048_576, 65535),
        entry("gemini-3-flash-preview", "Gemini 3 Flash Preview (Cloud Code Assist)", a, p, ep, true, true, (0.0,0.0,0.0,0.0), 1_048_576, 65535),
    ]
}

fn static_antigravity() -> Vec<ModelEntry> {
    let ep = "https://daily-cloudcode-pa.sandbox.googleapis.com";
    let (a, p) = ("google-gemini-cli", "google-antigravity");
    vec![
        entry("gemini-3-pro-high", "Gemini 3 Pro High (Antigravity)", a, p, ep, true, true, (2.0,12.0,0.2,2.375), 1_048_576, 65535),
        entry("gemini-3-pro-low", "Gemini 3 Pro Low (Antigravity)", a, p, ep, true, true, (2.0,12.0,0.2,2.375), 1_048_576, 65535),
        entry("gemini-3-flash", "Gemini 3 Flash (Antigravity)", a, p, ep, true, true, (0.5,3.0,0.5,0.0), 1_048_576, 65535),
        entry("claude-sonnet-4-5", "Claude Sonnet 4.5 (Antigravity)", a, p, ep, false, true, (3.0,15.0,0.3,3.75), 200_000, 64000),
        entry("claude-sonnet-4-5-thinking", "Claude Sonnet 4.5 Thinking (Antigravity)", a, p, ep, true, true, (3.0,15.0,0.3,3.75), 200_000, 64000),
        entry("claude-opus-4-5-thinking", "Claude Opus 4.5 Thinking (Antigravity)", a, p, ep, true, true, (5.0,25.0,0.5,6.25), 200_000, 64000),
        entry("claude-opus-4-6-thinking", "Claude Opus 4.6 Thinking (Antigravity)", a, p, ep, true, true, (5.0,25.0,0.5,6.25), 200_000, 128_000),
        entry("gpt-oss-120b-medium", "GPT-OSS 120B Medium (Antigravity)", a, p, ep, false, false, (0.09,0.36,0.0,0.0), 131_072, 32768),
    ]
}

fn static_vertex() -> Vec<ModelEntry> {
    let (b, a, p) = ("https://{location}-aiplatform.googleapis.com", "google-vertex", "google-vertex");
    vec![
        entry("gemini-3-pro-preview", "Gemini 3 Pro Preview (Vertex)", a, p, b, true, true, (2.0,12.0,0.2,0.0), 1_000_000, 64000),
        entry("gemini-3.1-pro-preview", "Gemini 3.1 Pro Preview (Vertex)", a, p, b, true, true, (2.0,12.0,0.2,0.0), 1_048_576, 65536),
        entry("gemini-3-flash-preview", "Gemini 3 Flash Preview (Vertex)", a, p, b, true, true, (0.5,3.0,0.05,0.0), 1_048_576, 65536),
        entry("gemini-2.0-flash", "Gemini 2.0 Flash (Vertex)", a, p, b, false, true, (0.15,0.6,0.0375,0.0), 1_048_576, 8192),
        entry("gemini-2.0-flash-lite", "Gemini 2.0 Flash Lite (Vertex)", a, p, b, true, true, (0.075,0.3,0.01875,0.0), 1_048_576, 65536),
        entry("gemini-2.5-pro", "Gemini 2.5 Pro (Vertex)", a, p, b, true, true, (1.25,10.0,0.125,0.0), 1_048_576, 65536),
        entry("gemini-2.5-flash", "Gemini 2.5 Flash (Vertex)", a, p, b, true, true, (0.3,2.5,0.03,0.0), 1_048_576, 65536),
        entry("gemini-2.5-flash-lite-preview-09-2025", "Gemini 2.5 Flash Lite Preview 09-25 (Vertex)", a, p, b, true, true, (0.1,0.4,0.01,0.0), 1_048_576, 65536),
        entry("gemini-2.5-flash-lite", "Gemini 2.5 Flash Lite (Vertex)", a, p, b, true, true, (0.1,0.4,0.01,0.0), 1_048_576, 65536),
        entry("gemini-1.5-pro", "Gemini 1.5 Pro (Vertex)", a, p, b, false, true, (1.25,5.0,0.3125,0.0), 1_000_000, 8192),
        entry("gemini-1.5-flash", "Gemini 1.5 Flash (Vertex)", a, p, b, false, true, (0.075,0.3,0.01875,0.0), 1_000_000, 8192),
        entry("gemini-1.5-flash-8b", "Gemini 1.5 Flash-8B (Vertex)", a, p, b, false, true, (0.0375,0.15,0.01,0.0), 1_000_000, 8192),
    ]
}

fn static_kimi_fallbacks() -> Vec<ModelEntry> {
    let b = "https://api.kimi.com/coding";
    vec![
        entry("kimi-k2-thinking", "Kimi K2 Thinking", "anthropic-messages", "kimi-coding", b, true, false, (0.0,0.0,0.0,0.0), 262_144, 32768),
        entry("k2p5", "Kimi K2.5", "anthropic-messages", "kimi-coding", b, true, false, (0.0,0.0,0.0,0.0), 262_144, 32768),
    ]
}

// ---------------------------------------------------------------------------
// Overrides & missing-model insertion
// ---------------------------------------------------------------------------

fn has(models: &[ModelEntry], provider: &str, id: &str) -> bool {
    models.iter().any(|m| m.provider == provider && m.id == id)
}

fn apply_overrides(models: &mut Vec<ModelEntry>) {
    for m in models.iter_mut() {
        if m.provider == "anthropic" && m.id == "claude-opus-4-5" {
            m.cost_cach_read = 0.5;
            m.cost_cach_write = 6.25;
        }
    }

    for m in models.iter_mut() {
        if m.provider == "amazon-bedrock" && m.id.contains("anthropic.claude-opus-4-6-v1") {
            m.cost_cach_read = 0.5;
            m.cost_cach_write = 6.25;
            m.context_window = 200_000;
        }
        if (m.provider == "anthropic" || m.provider == "opencode") && m.id == "claude-opus-4-6" {
            m.context_window = 200_000;
        }
        if m.provider == "opencode" && (m.id == "claude-sonnet-4-5" || m.id == "claude-sonnet-4") {
            m.context_window = 200_000;
        }
    }

    if !has(models, "amazon-bedrock", "eu.anthropic.claude-opus-4-6-v1") {
        models.push(entry("eu.anthropic.claude-opus-4-6-v1", "Claude Opus 4.6 (EU)", "bedrock-converse-stream", "amazon-bedrock",
            "https://bedrock-runtime.us-east-1.amazonaws.com", true, true, (5.0,25.0,0.5,6.25), 200_000, 128_000));
    }
    if !has(models, "anthropic", "claude-opus-4-6") {
        models.push(entry("claude-opus-4-6", "Claude Opus 4.6", "anthropic-messages", "anthropic",
            "https://api.anthropic.com", true, true, (5.0,25.0,0.5,6.25), 200_000, 128_000));
    }
    if !has(models, "anthropic", "claude-sonnet-4-6") {
        models.push(entry("claude-sonnet-4-6", "Claude Sonnet 4.6", "anthropic-messages", "anthropic",
            "https://api.anthropic.com", true, true, (3.0,15.0,0.3,3.75), 200_000, 64_000));
    }
    if !has(models, "openai", "gpt-5-chat-latest") {
        models.push(entry("gpt-5-chat-latest", "GPT-5 Chat Latest", "openai-responses", "openai",
            "https://api.openai.com/v1", false, true, (1.25,10.0,0.125,0.0), 128_000, 16384));
    }
    if !has(models, "openai", "gpt-5.1-codex") {
        models.push(entry("gpt-5.1-codex", "GPT-5.1 Codex", "openai-responses", "openai",
            "https://api.openai.com/v1", true, true, (1.25,5.0,0.125,1.25), 400_000, 128_000));
    }
    if !has(models, "openai", "gpt-5.1-codex-max") {
        models.push(entry("gpt-5.1-codex-max", "GPT-5.1 Codex Max", "openai-responses", "openai",
            "https://api.openai.com/v1", true, true, (1.25,10.0,0.125,0.0), 400_000, 128_000));
    }
    if !has(models, "openai", "gpt-5.3-codex-spark") {
        models.push(entry("gpt-5.3-codex-spark", "GPT-5.3 Codex Spark", "openai-responses", "openai",
            "https://api.openai.com/v1", true, false, (0.0,0.0,0.0,0.0), 128_000, 16384));
    }
    if !has(models, "xai", "grok-code-fast-1") {
        models.push(entry("grok-code-fast-1", "Grok Code Fast 1", "openai-completions", "xai",
            "https://api.x.ai/v1", false, false, (0.2,1.5,0.02,0.0), 32768, 8192));
    }
    if !has(models, "openrouter", "auto") {
        models.push(entry("auto", "Auto", "openai-completions", "openrouter",
            "https://openrouter.ai/api/v1", true, true, (0.0,0.0,0.0,0.0), 2_000_000, 30_000));
    }

    models.extend(static_codex_models());
    models.extend(static_cloud_code_assist());
    models.extend(static_antigravity());
    models.extend(static_vertex());

    for kimi in static_kimi_fallbacks() {
        if !has(models, &kimi.provider, &kimi.id) {
            models.push(kimi);
        }
    }

    let azure: Vec<ModelEntry> = models.iter()
        .filter(|m| m.provider == "openai" && m.api == "openai-responses")
        .map(|m| ModelEntry {
            api: "azure-openai-responses".to_string(),
            provider: "azure-openai-responses".to_string(),
            base_url: String::new(),
            ..m.clone()
        })
        .collect();
    models.extend(azure);
}

// ---------------------------------------------------------------------------
// Deduplicate & group
// ---------------------------------------------------------------------------

fn deduplicate(models: Vec<ModelEntry>) -> BTreeMap<String, BTreeMap<String, ModelEntry>> {
    let mut providers: BTreeMap<String, BTreeMap<String, ModelEntry>> = BTreeMap::new();
    for m in models {
        providers.entry(m.provider.clone()).or_default()
            .entry(m.id.clone()).or_insert(m);
    }
    providers
}

// ---------------------------------------------------------------------------
// Code generation — emits the exact format of model_descriptors.rs
// ---------------------------------------------------------------------------

fn format_f64(v: f64) -> String {
    if v == 0.0 { return "0.0".to_string(); }
    let s = format!("{v}");
    if s.contains('.') { s } else { format!("{s}.0") }
}

fn generate_rust(providers: &BTreeMap<String, BTreeMap<String, ModelEntry>>) -> String {
    let mut out = String::with_capacity(512 * 1024);

    out.push_str("\
// This file is auto-generated by `cargo run --bin ewe_platform gen_model_descriptors`.
// Do not edit manually.

use crate::types::{MessageType, ModelProviderDescriptor, ModelUsageCosting};

#[rustfmt::skip]
pub const MODEL_DESCRIPTORS: &[ModelProviderDescriptor] = &[
");

    for models in providers.values() {
        for m in models.values() {
            let input1 = if m.has_image_input { "MessageType::Images" } else { "MessageType::Text" };

            out.push_str(&format!("\
    ModelProviderDescriptor {{
        id: {id:?},
        name: {name:?},
        api: {api:?},
        provider: {provider:?},
        base_url: {base_url:?},
        reasoning: {reasoning},
        inputs: [MessageType::Text, {input1}],
        cost: ModelUsageCosting {{
            input: {ci},
            output: {co},
            cach_read: {cr},
            cach_write: {cw},
        }},
        context_window: {context_window},
        max_tokens: {max_tokens},
    }},
",
                id = m.id,
                name = m.name,
                api = m.api,
                provider = m.provider,
                base_url = m.base_url,
                reasoning = m.reasoning,
                ci = format_f64(m.cost_input),
                co = format_f64(m.cost_output),
                cr = format_f64(m.cost_cach_read),
                cw = format_f64(m.cost_cach_write),
                context_window = m.context_window,
                max_tokens = m.max_tokens,
            ));
        }
    }

    out.push_str("];\n");
    out
}

// ---------------------------------------------------------------------------
// CLI registration & run (follows bin/platform subcommand pattern)
// ---------------------------------------------------------------------------

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(
        clap::Command::new("gen_model_descriptors")
            .about("Fetch upstream model catalogs and regenerate backends/foundation_ai/src/models/model_descriptors.rs"),
    )
}

/// WHY: Entry point for the `gen_model_descriptors` subcommand.
///
/// WHAT: Fetches from three APIs, merges, deduplicates, writes Rust source.
///
/// HOW: Initialises tracing, creates `SimpleHttpClient`, runs fetch pipeline,
/// writes to disk.
///
/// # Errors
///
/// Returns `GenModelError::WriteFile` if the output file cannot be written.
/// HTTP/JSON errors from individual sources are logged and skipped (the
/// pipeline continues with partial data).
///
/// # Panics
///
/// Panics if the tracing subscriber cannot be set (programmer error).
pub fn run(_args: &clap::ArgMatches) -> std::result::Result<(), BoxedError> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let client = SimpleHttpClient::from_system();

    let models_dev = fetch_models_dev(&client);
    let openrouter = fetch_openrouter(&client);
    let ai_gateway = fetch_ai_gateway(&client);

    let mut all_models = Vec::with_capacity(models_dev.len() + openrouter.len() + ai_gateway.len());
    all_models.extend(models_dev);
    all_models.extend(openrouter);
    all_models.extend(ai_gateway);

    apply_overrides(&mut all_models);

    let total = all_models.len();
    let reasoning_count = all_models.iter().filter(|m| m.reasoning).count();

    let providers = deduplicate(all_models);
    let rust_source = generate_rust(&providers);

    let output_path = PathBuf::from("backends/foundation_ai/src/models/model_descriptors.rs");
    std::fs::write(&output_path, &rust_source).map_err(|e| GenModelError::WriteFile {
        path: output_path.display().to_string(),
        source: e,
    })?;

    tracing::info!("Generated {}", output_path.display());
    tracing::info!("Total tool-capable models: {total}");
    tracing::info!("Reasoning-capable models: {reasoning_count}");
    for (provider, models) in &providers {
        tracing::info!("  {provider}: {} models", models.len());
    }

    Ok(())
}
