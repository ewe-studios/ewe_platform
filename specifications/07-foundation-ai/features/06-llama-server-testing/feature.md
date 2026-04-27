---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/06-llama-server-testing"
this_file: "specifications/07-foundation-ai/features/06-llama-server-testing/feature.md"

feature: "llama-server Testing Infrastructure"
description: "Build llama-server via build.rs as part of the Rust build process, manage its lifecycle (start/stop/wait-ready), and run real integration tests against it for the OpenAI-compatible Chat Completions and Responses API providers"
status: unapproved
priority: high
depends_on:
  - "01-llamacpp-integration"
  - "00c-openai-provider"
  - "00g-openai-provider-enhancements"
estimated_effort: "medium"
created: 2026-04-27
last_updated: 2026-04-27
author: "Main Agent"

tasks:
  completed: 0
  uncompleted: 16
  total: 16
  completion_percentage: 0%
---

# llama-server Testing Infrastructure

## Overview

The current OpenAI provider integration tests (`tests/openai_provider.rs`) use
`TestHttpServer` — a mock that returns pre-canned JSON/SSE responses. This
verifies parsing logic but does **not** validate that the providers actually
work against a real OpenAI-compatible server.

llama.cpp ships `llama-server` (`tools/server/` in the llama.cpp tree) which
exposes fully OpenAI-compatible `/v1/chat/completions` and `/v1/responses`
endpoints. This feature integrates llama-server building into the Rust build
process via `build.rs`, manages its lifecycle, downloads a small GGUF test
model, and runs real integration tests against it.

**Key design decision: `build.rs` over raw mise CMake**

Rather than having mise tasks run CMake directly (a separate build system
invoked outside of Cargo), we add a `build.rs` to `foundation_ai` that:
- Detects the llama.cpp source tree at `infrastructure/llama-bindings/llama.cpp/`
- Runs CMake + make to build `llama-server`
- Copies the binary to `support/bin/llama-server` (project-relative)
- Is **only** triggered when `LLAMA_SERVER_BUILD=1` is set — regular
  `cargo build` remains fast
- Uses `println!("cargo:rerun-if-env-changed=LLAMA_SERVER_BUILD")` to avoid
  unnecessary rebuilds

This makes the llama-server binary a **natural Rust build artifact** — mise
just sets the env var and runs `cargo build`, then knows to find the binary at
`support/bin/llama-server`.

**What this gives us:**
- Real end-to-end validation of `OpenAIProvider` (Chat Completions API)
- Real end-to-end validation of `ResponsesProvider` (Responses API)
- Text generation, streaming, tool calling, and Responses API event streaming
- CI-guard: `#[ignore]` by default, gated behind `LLAMA_SERVER_TEST=1`

**Iron laws from `requirements.md` apply** — no tokio/async-trait, Valtron-only
async, zero warnings, `derive_more::From` + manual `Display` errors.

## Local File Paths

- Spec: `specifications/07-foundation-ai/features/06-llama-server-testing/feature.md`
- mise config: `mise.toml` (root)
- build.rs: `backends/foundation_ai/build.rs`
- llama.cpp source: `infrastructure/llama-bindings/llama.cpp/`
- llama.cpp server: `infrastructure/llama-bindings/llama.cpp/tools/server/`
- Output binary: `support/bin/llama-server` (project root)
- foundation_ai Cargo.toml: `backends/foundation_ai/Cargo.toml`
- OpenAI provider tests: `backends/foundation_ai/tests/openai_provider.rs`
- Responses provider (new tests): `backends/foundation_ai/tests/openai_responses_provider.rs`

## Task Group 1: build.rs script

Add a `build.rs` to `foundation_ai` that conditionally builds llama-server.
This is the canonical build mechanism — mise delegates to it.

### Implementation

```rust
//! build.rs — Conditionally builds llama-server from the llama.cpp CMake tree.
//!
//! Only runs when LLAMA_SERVER_BUILD=1 is set in the environment.
//! Normal `cargo build` is unaffected.
//!
//! Output: support/bin/llama-server (relative to project root)

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // Tell cargo when to re-run this script
    println!("cargo:rerun-if-env-changed=LLAMA_SERVER_BUILD");

    let should_build = env::var("LLAMA_SERVER_BUILD")
        .ok()
        .map(|v| v == "1")
        .unwrap_or(false);

    if !should_build {
        // Skip: normal build, no llama-server needed
        return;
    }

    // Locate project root (CARGO_MANIFEST_DIR is backends/foundation_ai)
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let project_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("Expected project root two levels up from foundation_ai");

    let llama_dir = project_root.join("infrastructure/llama-bindings/llama.cpp");
    let output_dir = project_root.join("support/bin");
    let output_bin = output_dir.join("llama-server");

    // Check if source exists
    if !llama_dir.join("CMakeLists.txt").exists() {
        println!(
            "cargo:warning=llama.cpp source not found at {}",
            llama_dir.display()
        );
        println!(
            "cargo:warning=Run: git submodule update --init infrastructure/llama-bindings/llama.cpp"
        );
        return;
    }

    // Check if binary already exists and is fresh
    let build_dir = llama_dir.join("build");
    let cmake_cache = build_dir.join("CMakeCache.txt");
    let needs_cmake = !cmake_cache.exists();

    // Create output directory
    fs::create_dir_all(&output_dir).unwrap();

    // Copy existing binary if already built
    if build_dir.join("bin/llama-server").exists()
        && output_bin.exists()
        && !needs_cmake
    {
        println!(
            "cargo:warning=llama-server already built at {}",
            output_bin.display()
        );
        return;
    }

    // Build llama-server
    println!("cargo:warning=Building llama-server...");

    fs::create_dir_all(&build_dir).unwrap();

    // CMake configure
    let status = Command::new("cmake")
        .args([
            "..",
            "-DLLAMA_BUILD_SERVER=ON",
            "-DLLAMA_BUILD_TESTS=OFF",
            "-DLLAMA_BUILD_EXAMPLES=OFF",
            "-DLLAMA_BUILD_COMMON=ON",
            "-DCMAKE_BUILD_TYPE=Release",
        ])
        .current_dir(&build_dir)
        .status()
        .expect("Failed to run cmake");

    if !status.success() {
        panic!("cmake configure failed");
    }

    // Build only the server target
    let status = Command::new("make")
        .args(["llama-server", &format!("-j{}", num_cpus())])
        .current_dir(&build_dir)
        .status()
        .expect("Failed to run make");

    if !status.success() {
        panic!("make llama-server failed");
    }

    let built_bin = build_dir.join("bin/llama-server");
    if !built_bin.exists() {
        panic!(
            "llama-server binary not found at {} after build",
            built_bin.display()
        );
    }

    // Copy to support/bin
    fs::copy(&built_bin, &output_bin).unwrap();

    println!(
        "cargo:warning=llama-server built at {}",
        output_bin.display()
    );

    // Make the output dir discoverable by tests
    println!(
        "cargo:rustc-env=LLAMA_SERVER_BIN_PATH={}",
        output_bin.display()
    );
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
```

### Cargo.toml additions

No new dependencies needed for build.rs (it uses only `std`). The existing
`[package]` section already supports build scripts automatically when
`build.rs` is present.

### mise.toml build task (delegates to build.rs)

```toml
[tasks."llama:server:build"]
description = "Build llama-server via build.rs (places binary at support/bin/llama-server)"
run = """
LLAMA_SERVER_BUILD=1 cargo build --package foundation_ai --lib
echo "✓ llama-server built at $PROJECT_ROOT/support/bin/llama-server"
ls -lh "$PROJECT_ROOT/support/bin/llama-server"
"""

[tasks."llama:server:clean"]
description = "Clean llama-server build artifacts (both CMake and support/bin)"
run = """
#!/usr/bin/env bash
LLAMA_DIR="$PROJECT_ROOT/infrastructure/llama-bindings/llama.cpp"
if [ -d "$LLAMA_DIR/build" ]; then
    echo "Cleaning llama.cpp build directory..."
    rm -rf "$LLAMA_DIR/build"
fi
if [ -f "$PROJECT_ROOT/support/bin/llama-server" ]; then
    echo "Removing support/bin/llama-server..."
    rm -f "$PROJECT_ROOT/support/bin/llama-server"
fi
echo "Clean complete"
"""

[tasks."llama:server:version"]
description = "Show llama-server version and build info"
run = """
#!/usr/bin/env bash
SERVER_BIN="$PROJECT_ROOT/support/bin/llama-server"

if [ ! -f "$SERVER_BIN" ]; then
    echo "ERROR: llama-server not found at $SERVER_BIN"
    echo "Run: mise run llama:server:build"
    exit 1
fi

"$SERVER_BIN" --version
"""
```

## Task Group 2: Model download for testing

Download a small GGUF model for integration tests. We use
`Qwen/Qwen2.5-0.5B-Instruct-GGUF` (q4_k_m quantized, ~370 MB) — small enough
for CI, large enough to test real generation.

**Model paths are centralized** in `mise.toml` `[env]` section as
`LLAMA_TEST_MODEL_DIR`, `LLAMA_TEST_MODEL_FILE`, `LLAMA_TEST_MODEL_NAME`, and
`LLAMA_TEST_MODEL_URL`. All tasks and tests reference these env vars — change
the model in one place and everything updates.

### Tasks (use centralized env from mise.toml)

```toml
[tasks."llama:test-model:download"]
description = "Download $LLAMA_TEST_MODEL_DESC"
run = """
#!/usr/bin/env bash
set -e
mkdir -p "$LLAMA_TEST_MODEL_DIR"
if [ -f "$LLAMA_TEST_MODEL_FILE" ]; then exit 0; fi
curl -L -o "$LLAMA_TEST_MODEL_FILE" "$LLAMA_TEST_MODEL_URL"
"""

[tasks."llama:test-model:path"]
description = "Print the path to the test model file"
run = """
#!/usr/bin/env bash
echo "$LLAMA_TEST_MODEL_FILE"
"""
```

## Task Group 3: Server lifecycle management

Tasks to start, stop, and wait-for-ready on the llama-server. The server binary
is always at `support/bin/llama-server` — built by `build.rs`.

### Tasks to add to `mise.toml`

```toml
[tasks."llama:server:start"]
description = "Start llama-server with test model in background"
run = """
#!/usr/bin/env bash
set -e

SERVER_BIN="$PROJECT_ROOT/support/bin/llama-server"

PORT="${LLAMA_SERVER_PORT:-8999}"
LOG_FILE="$PROJECT_ROOT/llama-server-test.log"

# Validate prerequisites
if [ ! -f "$SERVER_BIN" ]; then
    echo "ERROR: llama-server not found at $SERVER_BIN"
    echo "Run: mise run llama:server:build"
    exit 1
fi

if [ ! -f "$LLAMA_TEST_MODEL_FILE" ]; then
    echo "ERROR: test model not found at $LLAMA_TEST_MODEL_FILE"
    echo "Run: mise run llama:test-model:download"
    exit 1
fi

# Kill any existing server
pkill -f "llama-server.*--port $PORT" 2>/dev/null || true
sleep 1

echo "Starting llama-server on port $PORT..."
echo "Model: $LLAMA_TEST_MODEL_FILE"
echo "Log:   $LOG_FILE"

"$SERVER_BIN" \
    --model "$LLAMA_TEST_MODEL_FILE" \
    --host 127.0.0.1 \
    --port "$PORT" \
    --api-key "test-api-key-123" \
    --threads 4 \
    --ctx-size 2048 \
    --batch-size 512 \
    --ubatch-size 256 \
    --no-webui \
    --log-disable \
    > "$LOG_FILE" 2>&1 &

SERVER_PID=$!
echo "llama-server PID: $SERVER_PID"
echo "$SERVER_PID" > /tmp/llama-server-test.pid

# Wait for server to be ready
echo "Waiting for llama-server to be ready..."
for i in $(seq 1 60); do
    if curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:$PORT/health" | grep -q "200"; then
        echo "llama-server is ready on port $PORT (PID: $SERVER_PID)"
        exit 0
    fi
    sleep 1
done

echo "ERROR: llama-server failed to start within 60 seconds"
echo "=== Last 50 lines of log ==="
tail -50 "$LOG_FILE"
kill "$SERVER_PID" 2>/dev/null || true
exit 1
"""

[tasks."llama:server:stop"]
description = "Stop llama-server background process"
run = """
#!/usr/bin/env bash
if [ -f /tmp/llama-server-test.pid ]; then
    PID=$(cat /tmp/llama-server-test.pid)
    if kill -0 "$PID" 2>/dev/null; then
        echo "Stopping llama-server (PID: $PID)..."
        kill "$PID"
        wait "$PID" 2>/dev/null || true
        echo "llama-server stopped"
    else
        echo "llama-server (PID: $PID) is not running"
    fi
    rm -f /tmp/llama-server-test.pid
else
    echo "No llama-server PID file found"
    pkill -f "llama-server.*--port" 2>/dev/null || true
    echo "Killed any remaining llama-server processes"
fi
"""

[tasks."llama:server:status"]
description = "Check if llama-server is running and healthy"
run = """
#!/usr/bin/env bash
PORT="${LLAMA_SERVER_PORT:-8999}"

if [ -f /tmp/llama-server-test.pid ]; then
    PID=$(cat /tmp/llama-server-test.pid)
    if kill -0 "$PID" 2>/dev/null; then
        echo "llama-server is running (PID: $PID)"
    else
        echo "llama-server PID $PID is not running"
    fi
else
    echo "No PID file found"
fi

echo "Checking health endpoint..."
STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:$PORT/health" 2>/dev/null || echo "000")
echo "Health status: $STATUS"

if [ "$STATUS" = "200" ]; then
    curl -s "http://127.0.0.1:$PORT/health" | head -5
fi
"""
```

## Task Group 4: Integration test runner

Wire up the test execution with automatic server start/stop and cleanup.

### Tasks to add to `mise.toml`

```toml
[tasks."test:llama-server"]
description = "Run all llama-server integration tests (auto-starts/stops server)"
run = """
#!/usr/bin/env bash
set -e

echo "=== llama-server Integration Tests ==="
echo ""

# Start server
mise run llama:server:start
trap 'mise run llama:server:stop' EXIT

# Run tests
export LLAMA_SERVER_TEST=1
export LLAMA_SERVER_URL="http://127.0.0.1:${LLAMA_SERVER_PORT:-8999}"
export LLAMA_SERVER_API_KEY="test-api-key-123"

echo ""
echo "=== Running OpenAI Provider Tests ==="
cargo test --package foundation_ai --test openai_provider -- --ignored --show-output

echo ""
echo "=== Running Responses Provider Tests ==="
cargo test --package foundation_ai --test openai_responses_provider -- --ignored --show-output

echo ""
echo "All llama-server integration tests passed"
"""

[tasks."test:llama-server:chat"]
description = "Run Chat Completions API tests against llama-server"
run = """
#!/usr/bin/env bash
set -e

mise run llama:server:start
trap 'mise run llama:server:stop' EXIT

export LLAMA_SERVER_TEST=1
export LLAMA_SERVER_URL="http://127.0.0.1:${LLAMA_SERVER_PORT:-8999}"
export LLAMA_SERVER_API_KEY="test-api-key-123"

cargo test --package foundation_ai --test openai_provider -- --ignored --show-output
"""

[tasks."test:llama-server:responses"]
description = "Run Responses API tests against llama-server"
run = """
#!/usr/bin/env bash
set -e

mise run llama:server:start
trap 'mise run llama:server:stop' EXIT

export LLAMA_SERVER_TEST=1
export LLAMA_SERVER_URL="http://127.0.0.1:${LLAMA_SERVER_PORT:-8999}"
export LLAMA_SERVER_API_KEY="test-api-key-123"

cargo test --package foundation_ai --test openai_responses_provider -- --ignored --show-output
"""
```

## Task Group 5: Integration test implementations

Write `#[ignore]`-gated integration tests in
`backends/foundation_ai/tests/openai_provider.rs` and a new
`backends/foundation_ai/tests/openai_responses_provider.rs` that connect to the
running llama-server.

### Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `LLAMA_SERVER_TEST` | (unset) | Set to `1` to enable server tests |
| `LLAMA_SERVER_URL` | `http://127.0.0.1:8999` | Base URL of the running server |
| `LLAMA_SERVER_API_KEY` | (empty) | API key for auth |
| `LLAMA_SERVER_MODEL` | `qwen2.5-0.5b-instruct` | Model name to use |
| `LLAMA_SERVER_PORT` | `8999` | Port override (also used by mise tasks) |
| `LLAMA_SERVER_BUILD` | (unset) | Set to `1` to trigger build.rs CMake build |

### OpenAI Provider tests (`tests/openai_provider.rs`)

Add `#[ignore]`-gated tests that use the real server instead of `TestHttpServer`:

```rust
/// Helper: create an OpenAIProvider connected to the running llama-server.
fn setup_llama_server_provider() -> (impl Model, String) {
    let base_url = std::env::var("LLAMA_SERVER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8999".into());
    let api_key = std::env::var("LLAMA_SERVER_API_KEY").unwrap_or_default();
    let model_name = std::env::var("LLAMA_SERVER_MODEL")
        .unwrap_or_else(|_| "qwen2.5-0.5b-instruct".into());

    let config = OpenAIConfig::new()
        .with_base_url(base_url.clone())
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key)));

    let provider = OpenAIProvider::new()
        .create(Some(config))
        .unwrap();

    let model = provider
        .get_model(ModelId::Name(model_name, None))
        .unwrap();

    (model, model_name)
}

/// Test: generate a short response against the real llama-server.
#[test]
#[ignore]
fn test_llama_server_generate() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let (model, _) = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".into()),
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Say hello in one word.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools: vec![],
        chat_template: None,
        tool_choice: None,
    };

    let result = model.generate(interaction, None).unwrap();
    assert!(!result.is_empty());

    if let Messages::Assistant { content, stop_reason, .. } = &result[0] {
        if let ModelOutput::Text(tc) = content {
            assert!(!tc.content.is_empty(), "Response should not be empty");
            assert!(
                tc.content.to_lowercase().contains("hello")
                    || tc.content.to_lowercase().contains("hi"),
                "Expected greeting-like response, got: {}",
                tc.content
            );
        } else {
            panic!("Expected Text output, got {content:?}");
        }
        assert!(
            matches!(stop_reason, StopReason::Stop | StopReason::Length),
            "Expected Stop or Length, got {stop_reason:?}"
        );
    } else {
        panic!("Expected Assistant message");
    }
}

/// Test: streaming text generation against the real llama-server.
#[test]
#[ignore]
fn test_llama_server_streaming() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let (model, _) = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Count from 1 to 3, one number per line.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools: vec![],
        chat_template: None,
        tool_choice: None,
    };

    let mut stream = model.stream(interaction, None).unwrap();
    let mut total_tokens = 0;
    for item in &mut stream {
        if let Stream::Next(msg) = item {
            if let Messages::Assistant { content, .. } = &msg {
                if let ModelOutput::Text(tc) = content {
                    total_tokens += tc.content.len();
                }
            }
        }
    }

    assert!(total_tokens > 0, "Should have received streamed tokens");
}

/// Test: multi-turn conversation with conversation history.
#[test]
#[ignore]
fn test_llama_server_multi_turn() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let (model, _) = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        messages: vec![
            Messages::User {
                role: "user".into(),
                content: UserModelContent::Text(TextContent {
                    content: "What is the capital of France?".into(),
                    signature: None,
                }),
                signature: None,
            },
            Messages::Assistant {
                content: ModelOutput::Text(TextContent {
                    content: "The capital of France is Paris.".into(),
                    signature: None,
                }),
                stop_reason: StopReason::Stop,
                usage: UsageReport::default(),
                metadata: None,
                signature: None,
            },
            Messages::User {
                role: "user".into(),
                content: UserModelContent::Text(TextContent {
                    content: "What about Spain?".into(),
                    signature: None,
                }),
                signature: None,
            },
        ],
        tools: vec![],
        chat_template: None,
        tool_choice: None,
    };

    let result = model.generate(interaction, None).unwrap();
    assert!(!result.is_empty());

    if let Messages::Assistant { content, .. } = &result[0] {
        if let ModelOutput::Text(tc) = content {
            assert!(
                tc.content.to_lowercase().contains("madrid"),
                "Expected Madrid in response, got: {}",
                tc.content
            );
        } else {
            panic!("Expected Text output");
        }
    }
}

/// Test: max_tokens constraint truncates output.
#[test]
#[ignore]
fn test_llama_server_max_tokens() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let (model, _) = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Write a long essay about the history of computing.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools: vec![],
        chat_template: None,
        tool_choice: None,
    };

    let mut params = ModelParams::default();
    params.max_tokens = Some(20);

    let result = model.generate(interaction, Some(params)).unwrap();
    assert!(!result.is_empty());

    if let Messages::Assistant { stop_reason, .. } = &result[0] {
        assert_eq!(
            *stop_reason,
            StopReason::Length,
            "Expected Length stop with max_tokens=20, got {stop_reason:?}"
        );
    }
}

/// Test: model discovery via /v1/models endpoint.
#[test]
#[ignore]
fn test_llama_server_list_models() {
    use foundation_ai::backends::openai_provider::OpenAIProvider;

    let _guard = valtron::initialize_pool(42, Some(4));
    let base_url = std::env::var("LLAMA_SERVER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8999".into());
    let api_key = std::env::var("LLAMA_SERVER_API_KEY").unwrap_or_default();

    let config = OpenAIConfig::new()
        .with_base_url(base_url)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key)));

    let provider = OpenAIProvider::new()
        .create(Some(config))
        .unwrap();

    let models = provider.list_models(None).unwrap();
    assert!(
        !models.is_empty(),
        "llama-server should report at least one model"
    );
}
```

### Responses Provider tests (`tests/openai_responses_provider.rs`)

New test file for Responses API integration against llama-server:

```rust
//! Integration tests for `ResponsesProvider` against a running llama-server.
//!
//! These tests are #[ignore]-gated and require:
//!   1. llama-server built and running: `mise run llama:server:start`
//!   2. Environment: LLAMA_SERVER_TEST=1

use foundation_ai::backends::openai_responses_provider::{
    ResponsesConfig, ResponsesProvider,
};
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelProvider,
    StopReason, TextContent, UsageReport, UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron;
use foundation_core::valtron::Stream;

fn setup_responses_provider() -> impl Model {
    let base_url = std::env::var("LLAMA_SERVER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8999".into());
    let api_key = std::env::var("LLAMA_SERVER_API_KEY").unwrap_or_default();

    let config = ResponsesConfig::new()
        .with_base_url(base_url)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key)));

    let provider = ResponsesProvider::new()
        .create(Some(config))
        .unwrap();

    provider
        .get_model(ModelId::Name("qwen2.5-0.5b-instruct".into(), None))
        .unwrap()
}

/// Test: generate a response via the Responses API.
#[test]
#[ignore]
fn test_llama_server_responses_generate() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let model = setup_responses_provider();

    let interaction = ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".into()),
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Say hello in one word.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools: vec![],
        chat_template: None,
        tool_choice: None,
    };

    let result = model.generate(interaction, None).unwrap();
    assert!(!result.is_empty());

    if let Messages::Assistant { content, stop_reason, .. } = &result[0] {
        if let ModelOutput::Text(tc) = content {
            assert!(!tc.content.is_empty());
        } else {
            panic!("Expected Text output");
        }
        assert!(
            matches!(stop_reason, StopReason::Stop | StopReason::Length),
            "Expected Stop or Length, got {stop_reason:?}"
        );
    }
}

/// Test: streaming via the Responses API.
#[test]
#[ignore]
fn test_llama_server_responses_stream() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let model = setup_responses_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Tell me a joke.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools: vec![],
        chat_template: None,
        tool_choice: None,
    };

    let mut stream = model.stream(interaction, None).unwrap();
    let mut received_any = false;
    for item in &mut stream {
        if let Stream::Next(_) = item {
            received_any = true;
        }
    }
    assert!(received_any, "Should have received streaming events");
}
```

## Task Group 6: Update PROGRESS.md

Add the new feature to the spec-level progress tracking.

## Complete Task List

### build.rs (3 tasks)

1. **[build]** Create `backends/foundation_ai/build.rs` with conditional llama-server CMake build, `LLAMA_SERVER_BUILD=1` gate, `rerun-if-env-changed` directive, and copy to `support/bin/`
2. **[build]** Add `llama:server:build`, `llama:server:clean`, `llama:server:version` mise tasks that delegate to build.rs (set env var + `cargo build`)
3. **[build]** Add `llama:test-model:download` and `llama:test-model:path` mise tasks

### Server lifecycle (3 tasks)

4. **[infra]** Add `llama:server:start` mise task — launch binary from `support/bin/llama-server`, health-check polling
5. **[infra]** Add `llama:server:stop` mise task — kill via PID file
6. **[infra]** Add `llama:server:status` mise task — check PID + health endpoint

### Test runner (3 tasks)

7. **[infra]** Add `test:llama-server`, `test:llama-server:chat`, `test:llama-server:responses` test runner tasks to `mise.toml` with automatic start/stop via trap
8. **[infra]** Document environment variables in the feature spec
9. **[infra]** Add `foundation_testing` helper module for llama-server test lifecycle (optional: `LlamaServerTestGuard` struct)

### OpenAI Provider Integration Tests (5 tasks)

10. **[test]** `test_llama_server_generate` — single-turn text generation against real server
11. **[test]** `test_llama_server_streaming` — streaming SSE generation
12. **[test]** `test_llama_server_multi_turn` — conversation with assistant history
13. **[test]** `test_llama_server_max_tokens` — validates `StopReason::Length`
14. **[test]** `test_llama_server_list_models` — `/v1/models` endpoint

### Responses Provider Integration Tests (1 task)

15. **[test]** Create `tests/openai_responses_provider.rs` with `#[ignore]`-gated generate + stream tests

### Documentation (1 task)

16. **[docs]** Add feature entry to `specifications/07-foundation-ai/PROGRESS.md`

## Execution Order

```
1       (build.rs — foundation of everything)
  -> 2, 3  (mise build/model tasks — depend on build.rs existing)
  -> 4, 5, 6  (server lifecycle — depend on binary path from build.rs)
  -> 7, 8, 9  (test runner — depend on lifecycle)
  -> 10–14  (OpenAI provider tests — can be done in parallel)
  -> 15     (Responses provider tests)
  -> 16     (update PROGRESS.md)
```

## Verification

After implementation, the full test flow should be:

```bash
# 1. Build server (triggers build.rs CMake build)
mise run llama:server:build

# 2. Download test model
mise run llama:test-model:download

# 3. Run all integration tests (auto-starts/stops server)
mise run test:llama-server

# 4. Or manually control the server
mise run llama:server:start
mise run llama:server:status
LLAMA_SERVER_TEST=1 cargo test --package foundation_ai --test openai_provider -- --ignored --show-output
LLAMA_SERVER_TEST=1 cargo test --package foundation_ai --test openai_responses_provider -- --ignored --show-output
mise run llama:server:stop

# 5. Normal Rust builds skip llama-server entirely
cargo build --package foundation_ai   # fast, no CMake
```
