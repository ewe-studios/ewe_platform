# foundation_ai

A unified AI inference framework for the ewe-platform. Talk to Anthropic Claude,
OpenAI GPT, any OpenAI-compatible API (Ollama, OpenRouter, vLLM, llama.cpp
server), and run local models via llama.cpp or Candle — all through the same
`Model` trait and the higher-level **agentic** session system.

No tokio. No async-trait. Zero heap allocations in the hot path. Built on
Valtron's `StreamIterator` pattern for sync APIs with threaded execution.

---

## Getting Started

Start here — these guides walk you from zero to a working agent, with both
harness shortcuts and manual (no-helpers) setups:

| Guide | What it covers |
|-------|---------------|
| [**01. Providers**](docs/getting-started/01-providers.md) | Setup for Llama.cpp, OpenAI, Anthropic, OpenRouter — harness *and* manual paths |
| [**02. Agent Harness**](docs/getting-started/02-agent-harness.md) | Agent lifecycle: sessions, turns, steering, memory, tools, resume |

### Runnable Examples

```bash
# Harness shortcuts — one-call setup:
cargo run -p foundation_ai --example hello_claude --features agentic
cargo run -p foundation_ai --example hello_openai --features agentic
cargo run -p foundation_ai --example hello_openrouter --features agentic
cargo run -p foundation_ai --example hello_llamacpp --features "agentic llamacpp"

# Manual (no helpers) — full ProviderRouter setup:
cargo run -p foundation_ai --example manual_claude_router --features agentic
cargo run -p foundation_ai --example manual_openrouter --features agentic

# Advanced:
cargo run -p foundation_ai --example custom_router --features agentic
cargo run -p foundation_ai --example agent_with_tools --features agentic
```

---

## Features

- **Three provider types** — HTTP APIs (Anthropic, OpenAI), local inference
  (llama.cpp GGUF), and pure-Rust (Candle safetensors)
- **20+ provider identifiers** — Anthropic, OpenAI, OpenRouter, Groq, xAI,
  Google Vertex, Mistral, GitHub Copilot, and more (model registry only)
- **OpenAI-compatible routing** — one `OpenAIConfig` with `base_url` works
  with Ollama, llama.cpp server, vLLM, LM Studio, OpenRouter, etc.
- **Tool calling** — provider-specific formatters for Anthropic Messages,
  OpenAI Completions, and OpenAI Responses APIs; text-based XML fallback
  for local models without JSON tool templates
- **Streaming** — Server-Sent Events via `StreamIterator` with automatic
  reconnection on 429/5xx; token-by-token callback interface
- **Cost tracking** — per-interaction cost calculation with character-based
  token estimation, running totals via `CostAccumulator`
- **Auto-generated model descriptors** — static metadata for 30+ models
  across 12 providers (pricing, context windows, API endpoints)
- **HuggingFace integration** — automatic GGUF/safetensors download with
  quantization parsing from model IDs like `TheBloke/Llama-2-7B-GGUF:q4_k_m`

## Installation

```toml
[dependencies]
foundation_ai = { path = "backends/foundation_ai" }
foundation_auth = { path = "backends/foundation_auth" }
serde_json = "1"
```

### Feature flags

```toml
[dependencies]
foundation_ai = { path = "backends/foundation_ai", features = ["candle"] }
```

| Flag | What it does |
|---|---|
| `candle` | Candle pure-Rust inference (CPU only) |
| `candle-cuda` | Candle with CUDA GPU acceleration |
| `candle-metal` | Candle with Apple Metal GPU |
| `cuda` | llama.cpp with CUDA GPU offload |
| `cuda_static` | llama.cpp with static CUDA linking |
| `metal` | llama.cpp with Apple Metal (M1/M2/M3) |
| `vulkan` | llama.cpp with Vulkan (cross-platform GPU) |
| `multi` | Multi-threaded executor for concurrent generation |
| `mtmd` | Multi-modal model support (vision) |
| `openmp` | OpenMP parallelism for llama.cpp |

## Quick Start

### Anthropic (Claude)

```rust
use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelProvider,
    TextContent, UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};

// 1. Create the provider
let provider = AnthropicMessagesProvider::new();

// 2. Configure with your API key
let config = AnthropicConfig::new()
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
        std::env::var("ANTHROPIC_API_KEY").unwrap(),
    )));

// 3. Initialize the provider (validates credentials, sets up HTTP client)
let provider = provider.create(Some(config)).unwrap();

// 4. Get a model
let model = provider
    .get_model(ModelId::Name("claude-sonnet-4-20250514".into(), None))
    .unwrap();

// 5. Build an interaction
let interaction = ModelInteraction {
    system_prompt: Some("You are a helpful assistant.".into()),
    soul: None,
    messages: vec![Messages::User {
        role: "user".into(),
        content: UserModelContent::Text(TextContent {
            content: "Explain Rust ownership in two sentences.".into(),
            signature: None,
        }),
        signature: None,
    }],
    tools_shed: None,
    chat_template: None,
    tool_choice: None,
};

// 6. Generate (blocking)
let outputs = model.generate(interaction, None).unwrap();

// 7. Extract the response
for output in outputs {
    match output {
        Messages::Assistant { content, .. } => {
            if let ModelOutput::Text(text) = content {
                println!("{}", text.content);
            }
        }
        _ => {}
    }
}
```

### OpenAI (GPT)

```rust
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelProvider,
    TextContent, UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};

let provider = OpenAIProvider::new();

let config = OpenAIConfig::new()
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
        std::env::var("OPENAI_API_KEY").unwrap(),
    )));

let provider = provider.create(Some(config)).unwrap();
let model = provider
    .get_model(ModelId::Name("gpt-4o".into(), None))
    .unwrap();

let interaction = ModelInteraction {
    system_prompt: None,
    soul: None,
    messages: vec![Messages::User {
        role: "user".into(),
        content: UserModelContent::Text(TextContent {
            content: "Write a haiku about Rust.".into(),
            signature: None,
        }),
        signature: None,
    }],
    tools_shed: None,
    chat_template: None,
    tool_choice: None,
};

let outputs = model.generate(interaction, None).unwrap();
```

### OpenAI Responses API (Reasoning Models: o1, o3, o1-pro)

For OpenAI's reasoning models that use the `/v1/responses` endpoint instead
of `/v1/chat/completions`:

```rust
use foundation_ai::backends::openai_responses_provider::{
    ResponsesConfig, ResponsesProvider,
};
use foundation_ai::types::{Model, ModelId, ModelProvider};
use foundation_auth::{AuthCredential, ConfidentialText};

let provider = ResponsesProvider::new();

let config = ResponsesConfig::new()
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
        std::env::var("OPENAI_API_KEY").unwrap(),
    )))
    .with_timeout_secs(300); // reasoning models take longer

let provider = provider.create(Some(config)).unwrap();
let model = provider
    .get_model(ModelId::Name("o3-mini".into(), None))
    .unwrap();

// Same ModelInteraction interface as the Completions API
let outputs = model.generate(interaction, None).unwrap();
```

## OpenAI-Compatible Providers

The `OpenAIProvider` works with any endpoint that implements the OpenAI
chat completions API. Just change the `base_url`.

### Ollama (Local)

```rust
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{Model, ModelId, ModelProvider};
use foundation_auth::{AuthCredential, ConfidentialText};

let provider = OpenAIProvider::new();

let config = OpenAIConfig::new()
    .with_base_url("http://localhost:11434")
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
        "not-needed".into(), // Ollama ignores auth
    )));

let provider = provider.create(Some(config)).unwrap();
let model = provider
    .get_model(ModelId::Name("llama3".into(), None))
    .unwrap();
```

### llama.cpp Server (Local)

When running `llama-server` from llama.cpp:

```rust
let config = OpenAIConfig::new()
    .with_base_url("http://localhost:8080")
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
        "not-needed".into(),
    )));
```

### OpenRouter (Cloud, multi-provider)

```rust
let config = OpenAIConfig::new()
    .with_base_url("https://openrouter.ai/api/v1")
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
        std::env::var("OPENROUTER_API_KEY").unwrap(),
    )));

let provider = provider.create(Some(config)).unwrap();
// Use any OpenRouter model slug
let model = provider
    .get_model(ModelId::Name("meta-llama/llama-3-70b-instruct".into(), None))
    .unwrap();
```

### vLLM (Self-hosted, high-throughput)

```rust
let config = OpenAIConfig::new()
    .with_base_url("http://your-vllm-endpoint:8000")
    .with_timeout_secs(60)
    .with_max_retries(2);
```

### LM Studio (Local desktop)

```rust
let config = OpenAIConfig::new()
    .with_base_url("http://localhost:1234")
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
        "lm-studio".into(),
    )));
```

### GitHub Copilot API

```rust
let config = OpenAIConfig::new()
    .with_base_url("https://api.githubcopilot.com")
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
        std::env::var("GITHUB_TOKEN").unwrap(),
    )));
```

### Custom base_url with Anthropic

The `AnthropicConfig` supports a custom `messages_endpoint` for proxies:

```rust
use foundation_ai::backends::anthropic_messages_provider::AnthropicConfig;

let config = AnthropicConfig::new()
    .with_base_url("https://your-proxy.example.com")
    .with_messages_endpoint("/v1/messages") // override default /{api_version}/messages
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
        "your-proxy-key".into(),
    )));
```

## Local Inference with llama.cpp

Run GGUF models locally without any network connection. Built on the
`infrastructure_llama_cpp` crate (Rust bindings around llama.cpp).

### CPU-only

```rust
use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{Model, ModelProvider, ModelId, ModelSpec};
use std::path::PathBuf;

// Choose the CPU backend
let backend = LlamaBackends::LLamaCPU;

// Configure with defaults (CPU, 4096 context, mmap enabled)
let config = LlamaBackendConfig::builder()
    .n_threads(8)
    .context_length(4096)
    .build();

let provider = backend.create(Some(config)).unwrap();

// Load a local GGUF model
let model = provider.get_model_by_spec(ModelSpec {
    name: "llama-3".into(),
    id: ModelId::Name("llama-3".into(), None),
    devices: None,
    model_location: Some(PathBuf::from("/path/to/Meta-Llama-3-8B-Q4_K_M.gguf")),
    lora_location: None,
}).unwrap();
```

### GPU offload (CUDA or Metal)

```rust
let backend = LlamaBackends::LLamaGPU;

let config = LlamaBackendConfig::builder()
    .n_gpu_layers(32)       // offload all layers to GPU
    .context_length(8192)
    .build();

let provider = backend.create(Some(config)).unwrap();
```

### Metal (Apple Silicon)

Enable the `metal` feature:

```toml
[dependencies]
foundation_ai = { path = "backends/foundation_ai", features = ["metal"] }
```

```rust
let backend = LlamaBackends::LLamaMetal;
let config = LlamaBackendConfig::builder()
    .n_gpu_layers(35)
    .context_length(4096)
    .build();
```

### Multi-GPU with split mode

```rust
use foundation_ai::types::SplitMode;

let config = LlamaBackendConfig::builder()
    .n_gpu_layers(999)      // offload everything, split across GPUs
    .split_mode(SplitMode::Row)
    .main_gpu(0)
    .build();
```

## Candle Backend (Pure Rust)

Candle is HuggingFace's pure-Rust ML framework. No C dependencies, no
llama.cpp — runs safetensors models directly in Rust.

### Setup

```toml
[dependencies]
foundation_ai = { path = "backends/foundation_ai", features = ["candle"] }
```

### CPU inference

```rust
use foundation_ai::backends::candle::{CandleBackend, CandleBackendConfig, CandleArchitecture, CandleDType};
use foundation_ai::types::{Model, ModelProvider, ModelId, ModelSpec};
use std::path::PathBuf;

let backend = CandleBackend::cpu();

let config = CandleBackendConfig::builder()
    .context_length(4096)
    .dtype(CandleDType::F32)
    .architecture(CandleArchitecture::Llama)
    .build();

let provider = backend.create(Some(config)).unwrap();

let model = provider.get_model_by_spec(ModelSpec {
    name: "llama-3".into(),
    id: ModelId::Name("llama-3".into(), None),
    devices: None,
    model_location: Some(PathBuf::from("/path/to/llama-3/")), // directory with config.json + *.safetensors
    lora_location: None,
}).unwrap();
```

### CUDA acceleration

```toml
[dependencies]
foundation_ai = { path = "backends/foundation_ai", features = ["candle-cuda"] }
```

```rust
let config = CandleBackendConfig::builder()
    .dtype(CandleDType::F16)
    .build();

let provider = CandleBackend::cpu().create(Some(config)).unwrap();
// Requires candle-cuda feature; the backend auto-selects CUDA device 0.
```

### Metal (Apple Silicon)

```toml
[dependencies]
foundation_ai = { path = "backends/foundation_ai", features = ["candle-metal"] }
```

```rust
let provider = CandleBackend::cpu().create(None).unwrap();
// Requires candle-metal feature; the backend auto-selects Metal device 0.
```

## HuggingFace Integration

### GGUF models (via llama.cpp)

Automatically downloads GGUF files from HuggingFace Hub:

```rust
use foundation_ai::backends::huggingface_gguf_provider::{
    HuggingFaceGGUFProvider, HuggingFaceGGUFConfig,
};
use foundation_ai::backends::llamacpp::LlamaBackends;
use foundation_ai::types::{Model, ModelProvider, ModelId, Quantization};

let config = HuggingFaceGGUFConfig::builder()
    .llama_backend(LlamaBackends::LLamaCPU)
    .n_gpu_layers(0)
    .cache_dir("/home/user/.cache/huggingface")
    .default_quantization("q4_k_m")
    .build();

let provider = HuggingFaceGGUFProvider::new(config).unwrap();

// Parses model ID + quantization from the name
let model = provider.get_model(ModelId::Name(
    "TheBloke/Mistral-7B-Instruct-v0.2-GGUF".into(),
    Some(Quantization::Q4_K_M),
)).unwrap();
```

### Safetensors models (via Candle)

Requires the `candle` feature:

```rust
use foundation_ai::backends::huggingface_candle_provider::{
    HuggingFaceCandleProvider, HuggingFaceCandleConfig,
};
use foundation_ai::types::{Model, ModelProvider, ModelId};

let config = HuggingFaceCandleConfig::builder()
    .cache_dir("/home/user/.cache/huggingface")
    .build();

let provider = HuggingFaceCandleProvider::new(config).unwrap();
let model = provider.get_model(ModelId::Name("meta-llama/Llama-3-8B".into(), None)).unwrap();
```

## Messages and Interactions

The `Messages` enum is the universal message type across all providers:

```rust
use foundation_ai::types::{Messages, UserModelContent, TextContent, ModelOutput, StopReason, ModelProviders, UsageReport, UsageCosting, CostStatus};
use std::time::SystemTime;

// User message
let user = Messages::User {
    role: "user".into(),
    content: UserModelContent::Text(TextContent {
        content: "Hello!".into(),
        signature: None,
    }),
    signature: None,
};

// Model response (you typically receive this from generate())
let assistant = Messages::Assistant {
    model: ModelId::Name("gpt-4o".into(), None),
    timestamp: SystemTime::now(),
    usage: UsageReport {
        input: 10.0, output: 50.0, cache_read: 0.0, cache_write: 0.0, total_tokens: 60.0,
        cost: UsageCosting { currency: "USD".into(), input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0, total_tokens: 60.0, status: CostStatus::Estimated },
    },
    content: ModelOutput::Text(TextContent { content: "Hi there!".into(), signature: None }),
    stop_reason: StopReason::Stop,
    provider: ModelProviders::OPENAI,
    error_detail: None,
    signature: None,
    metadata: None,
};

// Tool result (for tool-calling round-trips)
let tool_result = Messages::ToolResult {
    id: "call_abc123".into(),
    name: "get_weather".into(),
    timestamp: SystemTime::now(),
    details: None,
    content: UserModelContent::Text(TextContent { content: "72°F, sunny".into(), signature: None }),
    error_detail: None,
    signature: None,
};

// Note: UsageReport comes from the provider response — you rarely
// construct Assistant messages manually. The fields above are
// populated by the HTTP backend after each generate() call.
```

### Multimodal input

```rust
use foundation_ai::types::{ImageContent, MimeType};

let user_with_image = Messages::User {
    role: "user".into(),
    content: UserModelContent::Image(ImageContent {
        b64: base64_image_data,
        mime_type: MimeType::ImageJpeg,
    }),
    signature: None,
};
```

## Streaming

Both the HTTP providers and llama.cpp backend support streaming via
`StreamIterator`. The iterator yields `Stream<T>` variants — filter for
`Stream::Next` to get actual message data:

```rust
use foundation_core::valtron::Stream;

let mut stream = model.stream(interaction, None).unwrap();

for item in &mut stream {
    if let Stream::Next(msg) = item {
        if let Messages::Assistant { content, .. } = msg {
            match content {
                ModelOutput::Text(text) => print!("{}", text.content),
                ModelOutput::ThinkingContent { thinking, .. } => eprintln!("🤔 {thinking}"),
                _ => {}
            }
        }
    }
}
```

`Stream` has four variants: `Next(msg)` — a new message, `Pending` — no data
yet, `Init` — connection established, `Ignore` — skip this item.

For HTTP providers, streaming uses Server-Sent Events with automatic
reconnection on 429/5xx responses.

## Tool Calling

### Defining tools

```rust
use foundation_ai::types::{Tool, Args, ToolShed, ToolChoice, ToolChoiceFunction, ToolFunctionRef};
use foundation_ai::types::ModelInteraction;

let weather_tool = Tool {
    id: "weather_1".into(),
    name: "get_weather".into(),
    description: "Get the current weather for a location.".into(),
    arguments: Some(Args::from_value(json!({
        "type": "object",
        "properties": {
            "location": {"type": "string"},
            "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
        },
        "required": ["location"]
    }))),
    returns: None,
};
```

### Forcing a specific tool

```rust
let interaction = ModelInteraction {
    system_prompt: None,
    soul: None,
    messages: messages,
    tools_shed: Some(ToolShed {
        shed: weather_tool,
        memory: None,
        delegate: None,
        read: Tool {
            id: "read_1".into(), name: "read_file".into(),
            description: "Read a file.".into(), arguments: None, returns: None,
        },
        edit: Tool {
            id: "edit_1".into(), name: "edit_file".into(),
            description: "Edit a file.".into(), arguments: None, returns: None,
        },
        write: Tool {
            id: "write_1".into(), name: "write_file".into(),
            description: "Write a file.".into(), arguments: None, returns: None,
        },
        search: Tool {
            id: "search_1".into(), name: "search".into(),
            description: "Search files.".into(), arguments: None, returns: None,
        },
        bash: None,
        others: None,
    }),
    chat_template: None,
    tool_choice: Some(ToolChoice::Function(ToolChoiceFunction {
        tool_type: "function".into(),
        function: ToolFunctionRef { name: "get_weather".into() },
    })),
};
```

Tool calling is handled automatically by provider-specific formatters:

- `AnthropicFormatter` — Anthropic tool JSON schema
- `OpenAIFormatter` — OpenAI function calling schema
- `TextBasedFormatter` — XML `<ToolCall>...</ToolCall>` fallback for local
  models without JSON tool templates

## Model Parameters

```rust
use foundation_ai::types::ModelParams;
use foundation_ai::types::{ThinkingLevels, CacheRetention};

let params = ModelParams {
    max_tokens: 4096,
    temperature: 0.7,
    top_p: 0.9,
    top_k: 40.0,
    repeat_penalty: 1.1,
    seed: Some(42),
    stop_tokens: vec!["\n\n".into()],
    thinking_level: ThinkingLevels::Medium,
    cache_retention: CacheRetention::None,
    thinking_budget: None,
    output_format: None,
    frequency_penalty: None,  // OpenAI only
    presence_penalty: None,    // OpenAI only
    logit_bias: None,          // OpenAI only
};

let outputs = model.generate(interaction, Some(params)).unwrap();
```

### Structured output (JSON schema)

```rust
use foundation_ai::types::{OutputFormat, JsonSchema};

let params = ModelParams {
    output_format: Some(OutputFormat::JsonSchema(JsonSchema {
        name: "Response".into(),
        description: Some("A structured response with confidence".into()),
        schema: json!({
            "type": "object",
            "properties": {
                "answer": {"type": "string"},
                "confidence": {"type": "number"}
            },
            "required": ["answer", "confidence"]
        }),
        strict: Some(true),
    })),
    ..Default::default()
};
```

## Cost Tracking

### Post-call cost calculation

```rust
use foundation_ai::costing::calculate_cost;
use foundation_ai::types::{CostStatus, ModelUsageCosting, UsageReport, UsageCosting};

// From the model's descriptor
let pricing = ModelUsageCosting {
    input: 2.50,       // $2.50 per million input tokens
    output: 7.50,      // $7.50 per million output tokens
    cache_read: 0.30,  // $0.30 per million cache read tokens
    cache_write: 3.75, // $3.75 per million cache write tokens
};

let usage = UsageReport {
    input: 1500.0,
    output: 300.0,
    cache_read: 0.0,
    cache_write: 0.0,
    total_tokens: 1800.0,
    cost: UsageCosting { currency: "USD".into(), input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0, total_tokens: 1800.0, status: CostStatus::Estimated },
};

let cost = calculate_cost(&pricing, &usage, CostStatus::Actual);
// cost.input = 0.003750
// cost.output = 0.002250
```

### Pre-call token estimation

```rust
use foundation_ai::costing::estimate_tokens;

let estimated = estimate_tokens(&interaction.messages);
// Uses character-based heuristics — no tokenizer required
```

### Running totals

Each model instance has a `CostAccumulator`. Call `model.costing()` to get
the running total across all interactions with that model.

### Retrospective cost analysis

```rust
use foundation_ai::costing::message_cost;

let total = message_cost(&conversation_messages);
```

## Model Descriptors

The crate ships with auto-generated static metadata for 30+ models:

```rust
use foundation_ai::models::model_descriptors;

let all = model_descriptors();
// &'static [ModelProviderDescriptor] — one entry per model

// Each descriptor contains: id, name, reasoning flag, API type, provider,
// base_url, inputs, cost (per-million pricing), context_window, max_tokens
```

Models are grouped by provider in `src/models/providers/`:

- `anthropic.rs` — Claude models
- `openai.rs` — GPT models
- `openrouter.rs` — OpenRouter model slugs
- `xai.rs` — Grok models
- `google_vertex.rs` — Gemini models
- And 7 more...

The descriptor generator (`src/models/generator.rs`) fetches from models.dev,
the OpenRouter API, and the Vercel AI Gateway.

## Error Handling

```rust
use foundation_ai::errors::ModelProviderErrors;

match provider.create(Some(config)) {
    Ok(p) => p,
    Err(ModelProviderErrors::NotFound(msg)) => {
        eprintln!("Provider not found: {}", msg);
    }
    Err(ModelProviderErrors::FailedFetching(msg)) => {
        eprintln!("Failed to fetch provider config: {}", msg);
    }
    Err(ModelProviderErrors::ModelErrors(e)) => {
        eprintln!("Model error: {:?}", e);
    }
}
```

Generation errors include provider-specific context overflow detection:
12+ provider patterns matched via regex (Anthropic, OpenAI, Google, xAI, Groq,
OpenRouter, llama.cpp, LM Studio, GitHub Copilot, MiniMax, Kimi, Cerebras, Mistral).

## Configuration Reference

### AnthropicConfig

| Method | Default | Description |
|---|---|---|
| `with_base_url()` | `https://api.anthropic.com` | API base URL |
| `with_api_version()` | `2023-06-01` | Anthropic API version |
| `with_timeout_secs()` | `120` | Request timeout |
| `with_max_retries()` | `3` | Exponential backoff retries |
| `with_streaming()` | `true` | Enable SSE streaming |
| `with_proxy_url()` | `None` | HTTP proxy |
| `with_messages_endpoint()` | `None` | Override endpoint path |
| `with_auth()` | required | API key credential |

### OpenAIConfig

| Method | Default | Description |
|---|---|---|
| `with_base_url()` | `https://api.openai.com` | API base URL |
| `with_api_version()` | `v1` | API version path segment |
| `with_timeout_secs()` | `30` | Request timeout |
| `with_max_retries()` | `3` | Exponential backoff retries |
| `with_streaming()` | `true` | Enable SSE streaming |
| `with_proxy_url()` | `None` | HTTP proxy |
| `with_auth()` | required | API key credential |

### LlamaBackendConfig

| Field | Default | Description |
|---|---|---|
| `n_gpu_layers` | `0` | Layers offloaded to GPU |
| `context_length` | `4096` | Max context window |
| `batch_size` | `512` | Inference batch size |
| `n_threads` | `available CPUs` | CPU thread count |
| `use_mmap` | `true` | Memory-map model file |
| `use_mlock` | `false` | Lock model in RAM |
| `kv_cache_type` | `F16` | KV cache precision |
| `split_mode` | `Layer` | Multi-GPU split strategy |
| `main_gpu` | `0` | Primary GPU index |

## Architecture

```
foundation_ai/
  src/
    lib.rs                      -- pub mod: backends, costing, errors, models, types
    types/mod.rs                -- Core types: Messages, ModelInteraction, ModelParams,
    |                           -- Model, ModelProvider, ToolFormatter, Tool, ToolShed
    backends/
    |   mod.rs                  -- Re-exports all backend modules
    |   anthropic_messages_provider.rs  -- Anthropic /v1/messages
    |   openai_provider.rs              -- OpenAI /v1/chat/completions
    |   openai_responses_provider.rs    -- OpenAI /v1/responses (reasoning models)
    |   llamacpp.rs                     -- llama.cpp local inference
    |   candle.rs                       -- Candle pure-Rust inference (feature: candle)
    |   huggingface_gguf_provider.rs    -- HF GGUF download + llama.cpp
    |   huggingface_candle_provider.rs  -- HF safetensors + Candle (feature: candle)
    |   llamacpp_helpers.rs             -- build_sampler_chain() utility
    costing.rs                -- calculate_cost, estimate_tokens, CostAccumulator
    errors/
    |   mod.rs                -- GenerationError, ModelErrors, ModelProviderErrors
    models/
        mod.rs                -- re-export model_descriptors()
        generator.rs          -- Upstream fetcher (models.dev, OpenRouter, Vercel)
        providers/            -- Auto-generated per-provider model lists
            anthropic.rs, openai.rs, xai.rs, google_vertex.rs, ...
```

All providers implement the same two traits:

- **`ModelProvider::create(config)`** — authenticate and initialize
- **`Model::generate(interaction, params)`** — blocking inference
- **`Model::stream(interaction, params)`** — token-by-token streaming

The uniform API means you can swap Anthropic for Ollama or OpenAI for
llama.cpp by changing only the provider instantiation — the rest of your code
(messages, params, output extraction) stays identical.

## Working with a Model Instance

Once you have a `Model` from any provider, the API is identical. The
`Model` trait exposes six methods:

```rust
// spec() — metadata about the loaded model
let spec = model.spec();
println!("Model: {}", spec.name);

// descriptor() — pricing, context window, provider info (None for local models)
if let Some(desc) = model.descriptor() {
    println!("Context window: {}", desc.context_window);
    println!("Pricing: ${:.2} per M input tokens", desc.cost.input);
}

// generate() — blocking inference, returns Vec<Messages>
let outputs = model.generate(interaction, None).unwrap();

// stream() — token-by-token streaming
let mut stream = model.stream(interaction, None).unwrap();

// costing() — cumulative usage report for this model instance
let usage = model.costing().unwrap();
println!("Total tokens used: {}", usage.total_tokens);
```

### Multi-turn conversations

Pass previous messages back in the `messages` array to continue a
conversation. The model remembers context from the entire message history:

```rust
let turn1 = model.generate(interaction1, None).unwrap();

// Build turn 2 by appending the assistant's response
let turn2_interaction = ModelInteraction {
    system_prompt: None,
    soul: None,
    messages: vec![
        user_message,                // first user turn
        turn1[0].clone(),            // assistant response
        Messages::User {             // follow-up
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Tell me more.".into(),
                signature: None,
            }),
            signature: None,
        },
    ],
    tools_shed: None,
    chat_template: None,
    tool_choice: None,
};

let turn2 = model.generate(turn2_interaction, None).unwrap();
```

### Extracting response content

Each provider returns the same `Messages` enum. Pattern-match to extract
what you need:

```rust
for msg in outputs {
    match msg {
        Messages::Assistant { content, stop_reason, usage, .. } => {
            match content {
                ModelOutput::Text(tc) => println!("{}", tc.content),
                ModelOutput::ThinkingContent { thinking, .. } => eprintln!("{thinking}"),
                ModelOutput::ToolCall { id, name, arguments, .. } => {
                    // Handle tool call — see Tool Calling section
                }
                ModelOutput::Image(ic) => {
                    // Base64 image data with ic.mime_type
                }
                ModelOutput::Embedding { dimensions, values } => {
                    // Vector for RAG / semantic search
                }
            }
            println!("Tokens: {:.0}", usage.total_tokens);
        }
        _ => {}
    }
}
```

### Passing per-call parameters

Override defaults for a single call with `ModelParams`:

```rust
let params = ModelParams {
    max_tokens: 512,
    temperature: 0.3,
    seed: Some(42),
    ..Default::default()
};

let outputs = model.generate(interaction, Some(params)).unwrap();
```

### Inspecting model pricing

Use the descriptor to check costs before sending a request:

```rust
if let Some(desc) = model.descriptor() {
    let estimated = estimate_tokens(&interaction.messages);
    let cost = calculate_cost(&desc.cost, &estimated, CostStatus::Estimated);
    println!("Estimated cost: ${:.4}", cost.input + cost.output);
}
```
