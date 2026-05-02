use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use derive_more::{Display, From};
use foundation_core::valtron::{self, Stream, TaskIterator, TaskIteratorExt};
use foundation_core::wire::simple_http::client::HttpRequestPending;
use foundation_core::wire::simple_http::client::RequestIntro;
use foundation_core::wire::simple_http::client::{body_reader, SendRequestTask, SimpleHttpClient};
use serde::Deserialize;

use crate::types::{MessageType, ModelAPI, ModelProviderDescriptor, ModelProviders, ModelUsageCosting};

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Display, From)]
pub enum GenModelError {
    #[display("http error for {url}: {source}")]
    Http {
        url: String,
        source: foundation_core::wire::simple_http::HttpClientError,
    },

    #[display("http {status} from {url}")]
    BadStatus { url: String, status: usize },

    #[display("response body from {url} is not valid UTF-8: {source}")]
    InvalidUtf8 {
        url: String,
        source: std::string::FromUtf8Error,
    },

    #[display("unexpected response body type from {url}")]
    UnexpectedBody { url: String },

    #[display("json parse error for {url}: {source}")]
    Json {
        url: String,
        source: serde_json::Error,
    },

    #[display("failed to write {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },
}

impl std::error::Error for GenModelError {}

// ---------------------------------------------------------------------------
// Generation result
// ---------------------------------------------------------------------------

pub struct GenerationResult {
    /// Map of filename (e.g. "anthropic.rs") → generated Rust source
    pub provider_files: BTreeMap<String, String>,
    /// Generated providers/mod.rs content
    pub mod_source: String,
    pub total_models: usize,
    pub reasoning_count: usize,
    pub provider_counts: BTreeMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Progress tracking for parallel fetch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FetchPending {
    Connecting { source: &'static str },
    AwaitingResponse { source: &'static str },
}

impl FetchPending {
    fn from_http(p: HttpRequestPending, source: &'static str) -> Self {
        match p {
            HttpRequestPending::WaitingForStream => Self::Connecting { source },
            HttpRequestPending::WaitingIntroAndHeaders
            | HttpRequestPending::CheckRedirectResponse => Self::AwaitingResponse { source },
        }
    }
}

impl std::fmt::Display for FetchPending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchPending::Connecting { source } => write!(f, "{source}: Connecting..."),
            FetchPending::AwaitingResponse { source } => {
                write!(f, "{source}: Awaiting response...")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Compile-time shape verification
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn verify_struct_shape() -> ModelProviderDescriptor {
    ModelProviderDescriptor {
        id: "",
        name: "",
        reasoning: false,
        api: ModelAPI::OpenAICompletions,
        provider: ModelProviders::ANTHROPIC,
        base_url: Some(""),
        inputs: MessageType::Text,
        cost: ModelUsageCosting {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 0,
        max_tokens: 0,
    }
}

// ---------------------------------------------------------------------------
// Intermediate model representation
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
    cost_cache_read: f64,
    cost_cache_write: f64,
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
    id: &str,
    name: &str,
    api: &str,
    provider: &str,
    base_url: &str,
    reasoning: bool,
    has_image_input: bool,
    cost: (f64, f64, f64, f64),
    context_window: u32,
    max_tokens: u32,
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
        cost_cache_read: cost.2,
        cost_cache_write: cost.3,
        context_window,
        max_tokens,
    }
}

fn from_dev(
    model_id: &str,
    m: &ModelsDevModel,
    api: &str,
    provider: &str,
    base_url: &str,
    default_ctx: u32,
    default_max: u32,
) -> ModelEntry {
    let (ci, co, cr, cw) = dev_cost(m.cost.as_ref());
    entry(
        model_id,
        m.name.as_deref().unwrap_or(model_id),
        api,
        provider,
        base_url,
        m.reasoning == Some(true),
        has_image(m.modalities.as_ref()),
        (ci, co, cr, cw),
        ctx(m.limit.as_ref(), default_ctx),
        max_tok(m.limit.as_ref(), default_max),
    )
}

// ---------------------------------------------------------------------------
// Static / fallback models
// ---------------------------------------------------------------------------

fn static_codex_models() -> Vec<ModelEntry> {
    let b = "https://chatgpt.com/backend-api";
    let (cw, mt) = (272_000, 128_000);
    vec![
        entry(
            "gpt-5.1",
            "GPT-5.1",
            "openai-codex-responses",
            "openai-codex",
            b,
            true,
            true,
            (1.25, 10.0, 0.125, 0.0),
            cw,
            mt,
        ),
        entry(
            "gpt-5.1-codex-max",
            "GPT-5.1 Codex Max",
            "openai-codex-responses",
            "openai-codex",
            b,
            true,
            true,
            (1.25, 10.0, 0.125, 0.0),
            cw,
            mt,
        ),
        entry(
            "gpt-5.1-codex-mini",
            "GPT-5.1 Codex Mini",
            "openai-codex-responses",
            "openai-codex",
            b,
            true,
            true,
            (0.25, 2.0, 0.025, 0.0),
            cw,
            mt,
        ),
        entry(
            "gpt-5.2",
            "GPT-5.2",
            "openai-codex-responses",
            "openai-codex",
            b,
            true,
            true,
            (1.75, 14.0, 0.175, 0.0),
            cw,
            mt,
        ),
        entry(
            "gpt-5.2-codex",
            "GPT-5.2 Codex",
            "openai-codex-responses",
            "openai-codex",
            b,
            true,
            true,
            (1.75, 14.0, 0.175, 0.0),
            cw,
            mt,
        ),
        entry(
            "gpt-5.3-codex",
            "GPT-5.3 Codex",
            "openai-codex-responses",
            "openai-codex",
            b,
            true,
            true,
            (1.75, 14.0, 0.175, 0.0),
            cw,
            mt,
        ),
        entry(
            "gpt-5.3-codex-spark",
            "GPT-5.3 Codex Spark",
            "openai-codex-responses",
            "openai-codex",
            b,
            true,
            false,
            (0.0, 0.0, 0.0, 0.0),
            128_000,
            mt,
        ),
    ]
}

fn static_cloud_code_assist() -> Vec<ModelEntry> {
    let ep = "https://cloudcode-pa.googleapis.com";
    let (a, p) = ("google-gemini-cli", "google-gemini-cli");
    vec![
        entry(
            "gemini-2.5-pro",
            "Gemini 2.5 Pro (Cloud Code Assist)",
            a,
            p,
            ep,
            true,
            true,
            (0.0, 0.0, 0.0, 0.0),
            1_048_576,
            65535,
        ),
        entry(
            "gemini-2.5-flash",
            "Gemini 2.5 Flash (Cloud Code Assist)",
            a,
            p,
            ep,
            true,
            true,
            (0.0, 0.0, 0.0, 0.0),
            1_048_576,
            65535,
        ),
        entry(
            "gemini-2.0-flash",
            "Gemini 2.0 Flash (Cloud Code Assist)",
            a,
            p,
            ep,
            false,
            true,
            (0.0, 0.0, 0.0, 0.0),
            1_048_576,
            8192,
        ),
        entry(
            "gemini-3-pro-preview",
            "Gemini 3 Pro Preview (Cloud Code Assist)",
            a,
            p,
            ep,
            true,
            true,
            (0.0, 0.0, 0.0, 0.0),
            1_048_576,
            65535,
        ),
        entry(
            "gemini-3-flash-preview",
            "Gemini 3 Flash Preview (Cloud Code Assist)",
            a,
            p,
            ep,
            true,
            true,
            (0.0, 0.0, 0.0, 0.0),
            1_048_576,
            65535,
        ),
    ]
}

fn static_antigravity() -> Vec<ModelEntry> {
    let ep = "https://daily-cloudcode-pa.sandbox.googleapis.com";
    let (a, p) = ("google-gemini-cli", "google-antigravity");
    vec![
        entry(
            "gemini-3-pro-high",
            "Gemini 3 Pro High (Antigravity)",
            a,
            p,
            ep,
            true,
            true,
            (2.0, 12.0, 0.2, 2.375),
            1_048_576,
            65535,
        ),
        entry(
            "gemini-3-pro-low",
            "Gemini 3 Pro Low (Antigravity)",
            a,
            p,
            ep,
            true,
            true,
            (2.0, 12.0, 0.2, 2.375),
            1_048_576,
            65535,
        ),
        entry(
            "gemini-3-flash",
            "Gemini 3 Flash (Antigravity)",
            a,
            p,
            ep,
            true,
            true,
            (0.5, 3.0, 0.5, 0.0),
            1_048_576,
            65535,
        ),
        entry(
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5 (Antigravity)",
            a,
            p,
            ep,
            false,
            true,
            (3.0, 15.0, 0.3, 3.75),
            200_000,
            64000,
        ),
        entry(
            "claude-sonnet-4-5-thinking",
            "Claude Sonnet 4.5 Thinking (Antigravity)",
            a,
            p,
            ep,
            true,
            true,
            (3.0, 15.0, 0.3, 3.75),
            200_000,
            64000,
        ),
        entry(
            "claude-opus-4-5-thinking",
            "Claude Opus 4.5 Thinking (Antigravity)",
            a,
            p,
            ep,
            true,
            true,
            (5.0, 25.0, 0.5, 6.25),
            200_000,
            64000,
        ),
        entry(
            "claude-opus-4-6-thinking",
            "Claude Opus 4.6 Thinking (Antigravity)",
            a,
            p,
            ep,
            true,
            true,
            (5.0, 25.0, 0.5, 6.25),
            200_000,
            128_000,
        ),
        entry(
            "gpt-oss-120b-medium",
            "GPT-OSS 120B Medium (Antigravity)",
            a,
            p,
            ep,
            false,
            false,
            (0.09, 0.36, 0.0, 0.0),
            131_072,
            32768,
        ),
    ]
}

#[allow(clippy::too_many_lines)]
fn static_vertex() -> Vec<ModelEntry> {
    let (b, a, p) = (
        "https://{location}-aiplatform.googleapis.com",
        "google-vertex",
        "google-vertex",
    );
    vec![
        entry(
            "gemini-3-pro-preview",
            "Gemini 3 Pro Preview (Vertex)",
            a,
            p,
            b,
            true,
            true,
            (2.0, 12.0, 0.2, 0.0),
            1_000_000,
            64000,
        ),
        entry(
            "gemini-3.1-pro-preview",
            "Gemini 3.1 Pro Preview (Vertex)",
            a,
            p,
            b,
            true,
            true,
            (2.0, 12.0, 0.2, 0.0),
            1_048_576,
            65536,
        ),
        entry(
            "gemini-3-flash-preview",
            "Gemini 3 Flash Preview (Vertex)",
            a,
            p,
            b,
            true,
            true,
            (0.5, 3.0, 0.05, 0.0),
            1_048_576,
            65536,
        ),
        entry(
            "gemini-2.0-flash",
            "Gemini 2.0 Flash (Vertex)",
            a,
            p,
            b,
            false,
            true,
            (0.15, 0.6, 0.0375, 0.0),
            1_048_576,
            8192,
        ),
        entry(
            "gemini-2.0-flash-lite",
            "Gemini 2.0 Flash Lite (Vertex)",
            a,
            p,
            b,
            true,
            true,
            (0.075, 0.3, 0.01875, 0.0),
            1_048_576,
            65536,
        ),
        entry(
            "gemini-2.5-pro",
            "Gemini 2.5 Pro (Vertex)",
            a,
            p,
            b,
            true,
            true,
            (1.25, 10.0, 0.125, 0.0),
            1_048_576,
            65536,
        ),
        entry(
            "gemini-2.5-flash",
            "Gemini 2.5 Flash (Vertex)",
            a,
            p,
            b,
            true,
            true,
            (0.3, 2.5, 0.03, 0.0),
            1_048_576,
            65536,
        ),
        entry(
            "gemini-2.5-flash-lite-preview-09-2025",
            "Gemini 2.5 Flash Lite Preview 09-25 (Vertex)",
            a,
            p,
            b,
            true,
            true,
            (0.1, 0.4, 0.01, 0.0),
            1_048_576,
            65536,
        ),
        entry(
            "gemini-2.5-flash-lite",
            "Gemini 2.5 Flash Lite (Vertex)",
            a,
            p,
            b,
            true,
            true,
            (0.1, 0.4, 0.01, 0.0),
            1_048_576,
            65536,
        ),
        entry(
            "gemini-1.5-pro",
            "Gemini 1.5 Pro (Vertex)",
            a,
            p,
            b,
            false,
            true,
            (1.25, 5.0, 0.3125, 0.0),
            1_000_000,
            8192,
        ),
        entry(
            "gemini-1.5-flash",
            "Gemini 1.5 Flash (Vertex)",
            a,
            p,
            b,
            false,
            true,
            (0.075, 0.3, 0.01875, 0.0),
            1_000_000,
            8192,
        ),
        entry(
            "gemini-1.5-flash-8b",
            "Gemini 1.5 Flash-8B (Vertex)",
            a,
            p,
            b,
            false,
            true,
            (0.0375, 0.15, 0.01, 0.0),
            1_000_000,
            8192,
        ),
    ]
}

fn static_kimi_fallbacks() -> Vec<ModelEntry> {
    let b = "https://api.kimi.com/coding";
    vec![
        entry(
            "kimi-k2-thinking",
            "Kimi K2 Thinking",
            "anthropic-messages",
            "kimi-coding",
            b,
            true,
            false,
            (0.0, 0.0, 0.0, 0.0),
            262_144,
            32768,
        ),
        entry(
            "k2p5",
            "Kimi K2.5",
            "anthropic-messages",
            "kimi-coding",
            b,
            true,
            false,
            (0.0, 0.0, 0.0, 0.0),
            262_144,
            32768,
        ),
    ]
}

// ---------------------------------------------------------------------------
// Overrides & missing-model insertion
// ---------------------------------------------------------------------------

fn has(models: &[ModelEntry], provider: &str, id: &str) -> bool {
    models.iter().any(|m| m.provider == provider && m.id == id)
}

#[allow(clippy::too_many_lines)]
fn apply_overrides(models: &mut Vec<ModelEntry>) {
    for m in models.iter_mut() {
        if m.provider == "anthropic" && m.id == "claude-opus-4-5" {
            m.cost_cache_read = 0.5;
            m.cost_cache_write = 6.25;
        }
    }

    for m in models.iter_mut() {
        if m.provider == "amazon-bedrock" && m.id.contains("anthropic.claude-opus-4-6-v1") {
            m.cost_cache_read = 0.5;
            m.cost_cache_write = 6.25;
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
        models.push(entry(
            "eu.anthropic.claude-opus-4-6-v1",
            "Claude Opus 4.6 (EU)",
            "bedrock-converse-stream",
            "amazon-bedrock",
            "https://bedrock-runtime.us-east-1.amazonaws.com",
            true,
            true,
            (5.0, 25.0, 0.5, 6.25),
            200_000,
            128_000,
        ));
    }
    if !has(models, "anthropic", "claude-opus-4-6") {
        models.push(entry(
            "claude-opus-4-6",
            "Claude Opus 4.6",
            "anthropic-messages",
            "anthropic",
            "https://api.anthropic.com",
            true,
            true,
            (5.0, 25.0, 0.5, 6.25),
            200_000,
            128_000,
        ));
    }
    if !has(models, "anthropic", "claude-sonnet-4-6") {
        models.push(entry(
            "claude-sonnet-4-6",
            "Claude Sonnet 4.6",
            "anthropic-messages",
            "anthropic",
            "https://api.anthropic.com",
            true,
            true,
            (3.0, 15.0, 0.3, 3.75),
            200_000,
            64_000,
        ));
    }
    if !has(models, "openai", "gpt-5-chat-latest") {
        models.push(entry(
            "gpt-5-chat-latest",
            "GPT-5 Chat Latest",
            "openai-responses",
            "openai",
            "https://api.openai.com/v1",
            false,
            true,
            (1.25, 10.0, 0.125, 0.0),
            128_000,
            16384,
        ));
    }
    if !has(models, "openai", "gpt-5.1-codex") {
        models.push(entry(
            "gpt-5.1-codex",
            "GPT-5.1 Codex",
            "openai-responses",
            "openai",
            "https://api.openai.com/v1",
            true,
            true,
            (1.25, 5.0, 0.125, 1.25),
            400_000,
            128_000,
        ));
    }
    if !has(models, "openai", "gpt-5.1-codex-max") {
        models.push(entry(
            "gpt-5.1-codex-max",
            "GPT-5.1 Codex Max",
            "openai-responses",
            "openai",
            "https://api.openai.com/v1",
            true,
            true,
            (1.25, 10.0, 0.125, 0.0),
            400_000,
            128_000,
        ));
    }
    if !has(models, "openai", "gpt-5.3-codex-spark") {
        models.push(entry(
            "gpt-5.3-codex-spark",
            "GPT-5.3 Codex Spark",
            "openai-responses",
            "openai",
            "https://api.openai.com/v1",
            true,
            false,
            (0.0, 0.0, 0.0, 0.0),
            128_000,
            16384,
        ));
    }
    if !has(models, "xai", "grok-code-fast-1") {
        models.push(entry(
            "grok-code-fast-1",
            "Grok Code Fast 1",
            "openai-completions",
            "xai",
            "https://api.x.ai/v1",
            false,
            false,
            (0.2, 1.5, 0.02, 0.0),
            32768,
            8192,
        ));
    }
    if !has(models, "openrouter", "auto") {
        models.push(entry(
            "auto",
            "Auto",
            "openai-completions",
            "openrouter",
            "https://openrouter.ai/api/v1",
            true,
            true,
            (0.0, 0.0, 0.0, 0.0),
            2_000_000,
            30_000,
        ));
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

    let azure: Vec<ModelEntry> = models
        .iter()
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
        providers
            .entry(m.provider.clone())
            .or_default()
            .entry(m.id.clone())
            .or_insert(m);
    }
    providers
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

fn format_u32(v: u32) -> String {
    let raw = v.to_string();
    if v < 10_000 {
        return raw;
    }
    let bytes: Vec<u8> = raw.bytes().collect();
    let mut result = String::with_capacity(raw.len() + bytes.len() / 3);
    for (i, ch) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            result.push('_');
        }
        result.push(*ch as char);
    }
    result
}

fn format_f64(v: f64) -> String {
    if v == 0.0 {
        return "0.0".to_string();
    }
    let s = format!("{v}");
    let s = if s.contains('.') { s } else { format!("{s}.0") };

    if let Some(dot_pos) = s.find('.') {
        let int_part = &s[..dot_pos];
        if let Ok(int_val) = int_part.parse::<i64>() {
            if int_val.unsigned_abs() >= 10_000 {
                let frac_part = &s[dot_pos..];
                let formatted_int = format_int_with_separators(int_part);
                return format!("{formatted_int}{frac_part}");
            }
        }
    }
    s
}

fn format_int_with_separators(s: &str) -> String {
    let (sign, digits) = if let Some(rest) = s.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", s)
    };
    let bytes: Vec<u8> = digits.bytes().collect();
    let mut result = String::with_capacity(digits.len() + bytes.len() / 3 + sign.len());
    result.push_str(sign);
    for (i, ch) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            result.push('_');
        }
        result.push(*ch as char);
    }
    result
}

fn api_to_variant(api: &str) -> String {
    match api {
        "openai-completions" => "ModelAPI::OpenAICompletions".to_string(),
        "openai-responses" => "ModelAPI::OpenAIResponses".to_string(),
        "azure-openai-responses" => "ModelAPI::AzureOpenaiResponses".to_string(),
        "openai-codex-responses" => "ModelAPI::OpenaiCodexResponses".to_string(),
        "anthropic-messages" => "ModelAPI::AnthropicMessages".to_string(),
        "bedrock-converse-stream" => "ModelAPI::BedrockConverseStream".to_string(),
        "google-generative-ai" => "ModelAPI::GoogleGenerativeAi".to_string(),
        "google-gemini-cli" => "ModelAPI::GoogleGeminiCli".to_string(),
        "google-vertex" => "ModelAPI::GoogleVertex".to_string(),
        other => format!("ModelAPI::Custom({other:?}.to_string())"),
    }
}

fn provider_to_variant(provider: &str) -> String {
    match provider {
        "amazon-bedrock" => "ModelProviders::AMAZONBEDROCK".to_string(),
        "anthropic" => "ModelProviders::ANTHROPIC".to_string(),
        "google" => "ModelProviders::GOOGLE".to_string(),
        "google-gemini-cli" => "ModelProviders::GOOGLEGEMINICLI".to_string(),
        "google-antigravity" => "ModelProviders::GOOGLEANTIGRAVITY".to_string(),
        "google-vertex" => "ModelProviders::GOOGLEVERTEX".to_string(),
        "openai" => "ModelProviders::OPENAI".to_string(),
        "azure-openai-responses" => "ModelProviders::AZUREOPENAIRESPONSES".to_string(),
        "openai-codex" => "ModelProviders::OPENAICODEX".to_string(),
        "github-copilot" => "ModelProviders::GITHUBCOPILOT".to_string(),
        "xai" => "ModelProviders::XAI".to_string(),
        "groq" => "ModelProviders::GROQ".to_string(),
        "cerebras" => "ModelProviders::CEREBRAS".to_string(),
        "openrouter" => "ModelProviders::OPENROUTER".to_string(),
        "vercel-ai-gateway" => "ModelProviders::VERCELAIGATEWAY".to_string(),
        "zai" => "ModelProviders::ZAI".to_string(),
        "mistral" => "ModelProviders::MISTRAL".to_string(),
        "minimax" => "ModelProviders::MINIMAX".to_string(),
        "minimax-cn" => "ModelProviders::MINIMAXCN".to_string(),
        "huggingface" => "ModelProviders::HUGGINGFACE".to_string(),
        "opencode" => "ModelProviders::OPENCODE".to_string(),
        "kimi-coding" => "ModelProviders::KIMICODING".to_string(),
        "llamacpp" => "ModelProviders::LLAMACPP".to_string(),
        other => format!("ModelProviders::Custom({other:?}.to_string())"),
    }
}

fn provider_to_filename(provider: &str) -> String {
    provider.replace('-', "_")
}

fn generate_provider_file(provider_name: &str, models: &BTreeMap<String, ModelEntry>) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(64 * 1024);

    let _ = writeln!(
        out,
        "\
// Auto-generated by `cargo run --bin ewe_platform gen_model_descriptors`.
// Do not edit manually.

use crate::types::{{MessageType, ModelAPI, ModelProviderDescriptor, ModelProviders, ModelUsageCosting}};

/// Model descriptors for the `{provider_name}` provider.
pub static MODELS: &[ModelProviderDescriptor] = &["
    );

    for m in models.values() {
        let input_type = if m.has_image_input {
            "MessageType::TextAndImages"
        } else {
            "MessageType::Text"
        };

        let base_url_str = if m.base_url.is_empty() {
            "None".to_string()
        } else {
            format!("Some({:?})", m.base_url)
        };

        let _ = write!(
            out,
            "\
    ModelProviderDescriptor {{
        id: {id:?},
        name: {name:?},
        reasoning: {reasoning},
        api: {api},
        provider: {provider},
        base_url: {base_url},
        inputs: {input},
        cost: ModelUsageCosting {{
            input: {ci},
            output: {co},
            cache_read: {cr},
            cache_write: {cw},
        }},
        context_window: {context_window},
        max_tokens: {max_tokens},
    }},
",
            id = m.id,
            name = m.name,
            reasoning = m.reasoning,
            api = api_to_variant(&m.api),
            provider = provider_to_variant(&m.provider),
            base_url = base_url_str,
            input = input_type,
            ci = format_f64(m.cost_input),
            co = format_f64(m.cost_output),
            cr = format_f64(m.cost_cache_read),
            cw = format_f64(m.cost_cache_write),
            context_window = format_u32(m.context_window),
            max_tokens = format_u32(m.max_tokens),
        );
    }

    out.push_str("];\n");
    out
}

fn generate_providers_mod(providers: &BTreeMap<String, BTreeMap<String, ModelEntry>>) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(4096);

    out.push_str(
        "\
// Auto-generated by `cargo run --bin ewe_platform gen_model_descriptors`.
// Do not edit manually.

use crate::types::ModelProviderDescriptor;

",
    );

    let filenames: Vec<String> = providers.keys().map(|p| provider_to_filename(p)).collect();

    for name in &filenames {
        let _ = writeln!(out, "pub mod {name};");
    }

    out.push('\n');

    let _ = writeln!(
        out,
        "\
/// Returns a combined slice of all provider model descriptors.
///
/// Uses `LazyLock` to concatenate per-provider static slices once.
pub fn model_descriptors() -> &'static [ModelProviderDescriptor] {{
    static ALL: std::sync::LazyLock<Vec<ModelProviderDescriptor>> = std::sync::LazyLock::new(|| {{
        let mut all = Vec::new();"
    );

    for name in &filenames {
        let _ = writeln!(out, "        all.extend_from_slice({name}::MODELS);");
    }

    let _ = write!(
        out,
        "\
        all
    }});
    &ALL
}}"
    );

    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// Fetch task creation
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_lines)]
fn create_fetch_task<F>(
    client: &mut SimpleHttpClient,
    source: &'static str,
    url: &'static str,
    parser: F,
) -> Result<
    Box<
        dyn TaskIterator<
                Ready = Vec<ModelEntry>,
                Pending = FetchPending,
                Spawner = foundation_core::valtron::BoxedSendExecutionAction,
            > + Send
            + 'static,
    >,
    GenModelError,
>
where
    F: Fn(&str, &'static str) -> Vec<ModelEntry> + Send + Clone + 'static,
{
    let request = client
        .get(url)
        .map_err(|e| GenModelError::Http {
            url: url.to_string(),
            source: e,
        })?
        .build()
        .map_err(|e| GenModelError::Http {
            url: url.to_string(),
            source: e,
        })?;

    let task = SendRequestTask::new(
        request,
        5,
        client.client_pool().expect("should have pool"),
        client.client_config(),
    )
    .map_ready(move |intro| match intro {
        RequestIntro::Success { stream, .. } => {
            let body_text = body_reader::collect_string(stream);
            parser(&body_text, source)
        }
        RequestIntro::Failed(e) => {
            tracing::warn!("Request failed for {url}: {e}");
            Vec::new()
        }
    })
    .map_pending(move |p| FetchPending::from_http(p, source));

    Ok(Box::new(task))
}

// ---------------------------------------------------------------------------
// Parser functions
// ---------------------------------------------------------------------------

fn parse_models_dev_response(body: &str, _source: &'static str) -> Vec<ModelEntry> {
    let data: serde_json::Value = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse models.dev response: {e}");
            return Vec::new();
        }
    };

    let mut models = Vec::new();
    if let Some(val) = data.get("openai") {
        if let Ok(p) = serde_json::from_value::<ModelsDevProvider>(val.clone()) {
            for (id, m) in p.models.iter().flat_map(|ms| ms.iter()) {
                if m.tool_call != Some(true) {
                    continue;
                }
                if m.status.as_deref() == Some("deprecated") {
                    continue;
                }
                let npm = m.provider.as_ref().and_then(|p| p.npm.as_deref());
                let (api, base_url) = match npm {
                    Some("@ai-sdk/openai") => ("openai-responses", "https://opencode.ai/zen/v1"),
                    Some("@ai-sdk/anthropic") => ("anthropic-messages", "https://opencode.ai/zen"),
                    Some("@ai-sdk/google") => {
                        ("google-generative-ai", "https://opencode.ai/zen/v1")
                    }
                    _ => ("openai-completions", "https://opencode.ai/zen/v1"),
                };
                models.push(from_dev(id, m, api, "opencode", base_url, 4096, 4096));
            }
        }
    }
    models
}

#[allow(clippy::cast_possible_truncation)]
fn parse_openrouter_response(body: &str, _source: &'static str) -> Vec<ModelEntry> {
    let data: OpenRouterResponse = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse OpenRouter response: {e}");
            return Vec::new();
        }
    };

    let mut models = Vec::new();
    for m in &data.data {
        let has_tools = m
            .supported_parameters
            .as_ref()
            .is_some_and(|p| p.iter().any(|s| s == "tools"));
        if !has_tools {
            continue;
        }

        let img = m
            .architecture
            .as_ref()
            .and_then(|a| a.modality.as_deref())
            .is_some_and(|s| s.contains("image"));

        let parse = |s: &Option<String>| -> f64 {
            s.as_deref()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0)
                * 1_000_000.0
        };

        let cost = (
            parse(&m.pricing.as_ref().and_then(|p| p.prompt.clone())),
            parse(&m.pricing.as_ref().and_then(|p| p.completion.clone())),
            parse(&m.pricing.as_ref().and_then(|p| p.input_cache_read.clone())),
            parse(&m.pricing.as_ref().and_then(|p| p.input_cache_write.clone())),
        );

        let reasoning = m
            .supported_parameters
            .as_ref()
            .is_some_and(|p| p.iter().any(|s| s == "reasoning"));

        models.push(entry(
            &m.id,
            m.name.as_deref().unwrap_or(&m.id),
            "openai-completions",
            "openrouter",
            "https://openrouter.ai/api/v1",
            reasoning,
            img,
            cost,
            m.context_length.unwrap_or(4096) as u32,
            m.top_provider
                .as_ref()
                .and_then(|t| t.max_completion_tokens)
                .unwrap_or(4096) as u32,
        ));
    }
    models
}

#[allow(clippy::cast_possible_truncation)]
fn parse_ai_gateway_response(body: &str, _source: &'static str) -> Vec<ModelEntry> {
    let data: AiGatewayResponse = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse AI Gateway response: {e}");
            return Vec::new();
        }
    };

    let mut models = Vec::new();
    for m in data.data.iter().flat_map(|d| d.iter()) {
        let tags = m.tags.as_deref().unwrap_or(&[]);
        if !tags.iter().any(|t| t == "tool-use") {
            continue;
        }

        let cost = (
            pricing_val(m.pricing.as_ref().and_then(|p| p.input.as_ref())) * 1_000_000.0,
            pricing_val(m.pricing.as_ref().and_then(|p| p.output.as_ref())) * 1_000_000.0,
            pricing_val(m.pricing.as_ref().and_then(|p| p.input_cache_read.as_ref())) * 1_000_000.0,
            pricing_val(
                m.pricing
                    .as_ref()
                    .and_then(|p| p.input_cache_write.as_ref()),
            ) * 1_000_000.0,
        );

        models.push(entry(
            &m.id,
            m.name.as_deref().unwrap_or(&m.id),
            "anthropic-messages",
            "vercel-ai-gateway",
            "https://ai-gateway.vercel.sh",
            tags.iter().any(|t| t == "reasoning"),
            tags.iter().any(|t| t == "vision"),
            cost,
            m.context_window.unwrap_or(4096) as u32,
            m.max_tokens.unwrap_or(4096) as u32,
        ));
    }
    models
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fetches model metadata from upstream APIs, merges with static/fallback
/// entries, deduplicates, and generates the Rust source for
/// `model_descriptors.rs`.
///
/// Caller is responsible for tracing subscriber setup.
///
/// # Errors
///
/// Returns `GenModelError` if HTTP or JSON parsing fails fatally.
/// Individual source failures are logged and skipped.
///
/// # Panics
///
/// Panics if the valtron execution fails to return a stream.
pub fn generate_model_descriptors() -> Result<GenerationResult, BoxedError> {
    let _guard = valtron::initialize_pool(100, None);

    let mut client = SimpleHttpClient::from_system()
        .max_body_size(None)
        .batch_size(8192 * 2)
        .read_timeout(Duration::from_secs(1))
        .max_retries(5);

    tracing::info!("Starting model descriptor generation with PARALLEL fetch...");
    let start_time = Instant::now();

    let models_dev_task = create_fetch_task(
        &mut client,
        "models.dev",
        "https://models.dev/api.json",
        parse_models_dev_response,
    )?;

    let openrouter_task = create_fetch_task(
        &mut client,
        "openrouter",
        "https://openrouter.ai/api/v1/models",
        parse_openrouter_response,
    )?;

    let ai_gateway_task = create_fetch_task(
        &mut client,
        "ai-gateway",
        "https://ai-gateway.vercel.sh/v1/models",
        parse_ai_gateway_response,
    )?;

    let mut all_models = Vec::new();

    let model_report_stream = valtron::execute_collect_all(
        vec![models_dev_task, openrouter_task, ai_gateway_task],
        None,
    )
    .expect("return stream");

    for stream_item in model_report_stream {
        if let Stream::Next(models) = stream_item {
            all_models.extend(models.into_iter().flatten());
        }
    }

    let fetch_elapsed = start_time.elapsed();
    tracing::info!("Parallel fetch completed in {:?}", fetch_elapsed);
    tracing::info!("Total models collected: {}", all_models.len());

    apply_overrides(&mut all_models);

    let total = all_models.len();
    let reasoning_count = all_models.iter().filter(|m| m.reasoning).count();

    let providers = deduplicate(all_models);

    let mut provider_files = BTreeMap::new();
    for (provider_name, models) in &providers {
        let filename = format!("{}.rs", provider_to_filename(provider_name));
        let source = generate_provider_file(provider_name, models);
        provider_files.insert(filename, source);
    }

    let mod_source = generate_providers_mod(&providers);

    let provider_counts = providers
        .iter()
        .map(|(name, models)| (name.clone(), models.len()))
        .collect();

    Ok(GenerationResult {
        provider_files,
        mod_source,
        total_models: total,
        reasoning_count,
        provider_counts,
    })
}
