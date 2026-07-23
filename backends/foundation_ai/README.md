# foundation_ai

A unified AI inference framework for the ewe-platform. Talk to Anthropic Claude,
OpenAI GPT, any OpenAI-compatible API (Ollama, OpenRouter, vLLM, llama.cpp
server), and run local models via **llama.cpp** or **Candle** — on CPU or GPU —
all through the same `Model` trait and the higher-level **agentic** session
system.

No tokio. No async-trait in the hot path. Built on Valtron's `StreamIterator`
pattern for sync APIs with threaded execution.

---

## Getting Started

New here? These guides take you from zero to a working agent, each showing the
**harness** shortcut *and* the **manual** setup:

| Guide | What it covers |
|-------|---------------|
| [**01. Providers**](docs/getting-started/01-providers.md) | Setup for llama.cpp, OpenAI, Anthropic, OpenRouter — persistence levels, harness *and* manual paths |
| [**02. Agent Harness**](docs/getting-started/02-agent-harness.md) | The agent lifecycle: sessions, turns, steering, memory, resume |
| [**03. Tools & Presets**](docs/getting-started/03-tools-and-presets.md) | The `read`/`write`/`edit`/`bash` tools, `ToolPreset` helpers, and sub-agent delegation |

---

## Deep Dives

Reference and how-to docs for everything the getting-started guides skim:

| Doc | Topic |
|-----|-------|
| [00. Overview](docs/00-overview.md) | Module map and architecture |
| [01. The Agentic Loop](docs/01-agentic-loop.md) | How a turn runs end to end |
| [02. Agent Types](docs/02-agent-types.md) | `Messages`, `ModelOutput`, the `Stream` contract |
| [03. Model Providers](docs/03-model-providers.md) | Provider implementations and the router |
| [04. Tools](docs/04-tools.md) | `ToolImpl`, `ToolShed`, `ToolPreset`, the execution DAG |
| [05. Steering Queues](docs/05-steering-queues.md) | Mid-turn injection and cancellation |
| [06. Serialization](docs/06-serialization.md) | JSON and Arrow columns |
| [07. Embeddings](docs/07-embeddings.md) | Embedding provider and vector integration |
| [08. AgentSession API](docs/08-how-to-agent-session.md) | Using the session handle |
| [09. Customizing the Loop](docs/09-customizing-agent-loop.md) | Tuning loop behaviour |
| [10. Building Custom Tools](docs/10-building-custom-tools.md) | Writing your own `ToolImpl` |
| [11. Memory System](docs/11-memory-system.md) | The memory hierarchy and `memory` tool |
| [12. Harness Presets](docs/12-how-to-harness-presets.md) | One-call model + router setup |
| [14. GPU Acceleration](docs/14-gpu-acceleration.md) | CUDA, Metal, Vulkan — both backends |
| [15. Upgrading CUDA](docs/15-cuda-upgrade.md) | What to change when the toolkit, `cudarc`, candle or llama.cpp moves — and how to prove the GPU path still works |

Root-caused bug write-ups live in [`docs/fixes/`](docs/fixes/).

---

## Install

```toml
[dependencies]
foundation_ai = { version = "0.0.1", features = ["agentic"] }
foundation_auth = "0.0.1"
foundation_db = "0.0.1"
serde_json = "1"
```

### Feature flags

| Flag | What it does |
|---|---|
| `agentic` | Agent loop, sessions, tools, memory |
| `llamacpp` | llama.cpp GGUF inference (on by default) |
| `candle` | Candle pure-Rust inference (on by default) |
| `candle-cuda` | Candle on NVIDIA CUDA |
| `cuda` / `cuda_static` | llama.cpp on NVIDIA CUDA |
| `metal` | llama.cpp on Apple Silicon (Candle Metal is automatic on Apple) |
| `vulkan` | llama.cpp on cross-platform GPU |
| `multi` | Multi-threaded executor for concurrent generation |
| `mtmd` | Multi-modal (vision) models |
| `testing` | `MockModelProvider` / `MockTool` for unit tests |

GPU builds need a couple of environment variables — see the
[GPU Acceleration](docs/14-gpu-acceleration.md) guide. Before upgrading the CUDA
toolkit, `cudarc`, candle or llama.cpp, read
[Upgrading CUDA](docs/15-cuda-upgrade.md) — the `CUDARC_CUDA_VERSION=13000` pin
is a deliberate floor-rounding and the rules for changing it are not obvious.

---

## Quick Start

The shortest path to a working agent — a Claude session in a few lines:

```rust
use foundation_ai::harness;
use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::types::{MessageRole, Messages, SessionId, TextContent, UserModelContent};
use foundation_compact::ids::new_scru128;
use foundation_core::valtron::valtron;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")?;

    // Harness wires the provider, router, and memory model for you.
    let agent = harness::claude_session::<Doc, Mem>(SessionId::new(), &api_key)?
        .with_system_prompt("You are a helpful assistant.")
        .build()?;

    let records = agent.run_turn(Messages::User {
        id: new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "Explain Rust ownership in two sentences.".into(),
            signature: None,
        }),
        signature: None,
    })?;

    for record in &records {
        println!("{record:?}");
    }
    agent.end()?;
    Ok(())
}
```

Prefer the raw `Model` trait, a different provider, or a manual `ProviderRouter`?
That's all in **[Getting Started: Providers](docs/getting-started/01-providers.md)**.

### Runnable examples

```bash
# Harness shortcuts — one-call setup:
cargo run -p foundation_ai --example hello_claude     --features agentic
cargo run -p foundation_ai --example hello_openai     --features agentic
cargo run -p foundation_ai --example hello_openrouter --features agentic
cargo run -p foundation_ai --example hello_llamacpp   --features "agentic llamacpp"

# Manual (no helpers) — full ProviderRouter setup:
cargo run -p foundation_ai --example manual_claude_router --features agentic
cargo run -p foundation_ai --example manual_openrouter    --features agentic

# Tools and delegation:
cargo run -p foundation_ai --example agent_with_tools --features agentic
```

---

## What you get

- **Every provider through one `Model` trait** — Anthropic Messages, OpenAI
  Chat Completions, OpenAI Responses (o1/o3 reasoning), and any
  OpenAI-compatible endpoint (Ollama, OpenRouter, vLLM, LM Studio, llama.cpp
  server) via one `base_url`.
- **Local inference** — llama.cpp (GGUF) and Candle (safetensors), on **CPU or
  GPU** (CUDA, Metal, Vulkan). See [GPU Acceleration](docs/14-gpu-acceleration.md).
- **The agentic layer** — `AgentSession` orchestrates generation, a memory
  hierarchy (working / observation / reflection), tool execution, token budgets,
  loop detection, and mid-turn steering.
- **Tools** — built-in `read`/`write`/`edit` (over a swappable VFS) and `bash`,
  bundled by [`ToolPreset`](docs/getting-started/03-tools-and-presets.md), plus
  background **sub-agent delegation**. Provider-specific tool formatters for
  Anthropic, OpenAI Completions, and OpenAI Responses, with an XML fallback for
  local models.
- **Streaming** — Server-Sent Events through `StreamIterator`, with automatic
  reconnection on 429/5xx.
- **Cost tracking** — per-interaction costing and running totals via
  `CostAccumulator`.
- **HuggingFace integration** — automatic GGUF/safetensors download with
  quantization parsing from IDs like `TheBloke/Llama-2-7B-GGUF:q4_k_m`.

---

## Testing & coverage

Tests live in `tests/`. Run them with the `uat` profile (the `dev` profile's
Cranelift backend can't do `catch_unwind` or coverage instrumentation):

```bash
# Offline suite — no network, no model downloads.
cargo test --profile uat -p foundation_ai --features testing

# Full suite, including gated live-model and external-service tests.
cargo test --profile uat -p foundation_ai \
  --features "testing live-model-tests external-service-tests"

# GPU paths (self-skip without a device):
cargo test --profile uat -p foundation_ai --features "testing candle-cuda" candle::candle_cuda
cargo test --profile uat -p foundation_ai --features "testing cuda"        llamacpp_cuda
```

Coverage is measured with `cargo llvm-cov` under the same `uat` profile. Gated
tests (live vendor APIs, downloaded models, GPU) self-skip when their
prerequisites are absent, so the offline suite always runs clean.

**Latest measured:** ~**86% lines / ~87% regions**, ~1,760 tests, across the
offline, live-model, and external-service features (via
`cargo llvm-cov --profile uat -p foundation_ai --features "testing
live-model-tests external-service-tests"`). Every source file is above the
reviewed bar; no file sits at 0%. GPU paths are additionally covered by the
gated `candle-cuda` / `cuda` suites.

---

## Design notes

- **Sync API, threaded execution.** No tokio, no async-trait on the hot path;
  the streaming APIs are `StreamIterator`s driven on the Valtron pool.
- **Provider parity.** Tool calling, streaming, and cost tracking behave the
  same across providers; differences are confined to per-provider formatters.
- **VFS-backed tools.** File tools go through `AsyncVfsFileSystem`, so the same
  agent runs against real disk, an in-memory FS (tests), or a remote FS — and on
  wasm.

See [`docs/00-overview.md`](docs/00-overview.md) for the full architecture.
