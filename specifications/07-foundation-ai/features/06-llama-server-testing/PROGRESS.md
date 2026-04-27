# Progress - llama-server Testing Infrastructure

## Overview

Build llama-server via `build.rs` as part of the Rust build process, manage its
lifecycle via mise tasks, and run real integration tests against it for both
the Chat Completions and Responses API providers.

## Task Status

| # | Task | Status |
|---|------|--------|
| 1 | build.rs with conditional CMake build | Pending |
| 2 | mise build/clean/version tasks (delegates to build.rs) | Pending |
| 3 | mise test model download tasks | Pending |
| 4 | llama:server:start (health-check polling) | Pending |
| 5 | llama:server:stop (PID file) | Pending |
| 6 | llama:server:status (check PID + endpoint) | Pending |
| 7 | test runner mise tasks (auto start/stop) | Pending |
| 8 | Document environment variables | Pending |
| 9 | Optional: LlamaServerTestGuard helper | Pending |
| 10 | test_llama_server_generate | Pending |
| 11 | test_llama_server_streaming | Pending |
| 12 | test_llama_server_multi_turn | Pending |
| 13 | test_llama_server_max_tokens | Pending |
| 14 | test_llama_server_list_models | Pending |
| 15 | Responses provider tests (generate + stream) | Pending |
| 16 | Update spec-level PROGRESS.md | Pending |

**Totals:** 0 / 16 tasks complete (0%)

## What's Done

Nothing yet — feature spec just created.

## Notes

- llama-server binary: `support/bin/llama-server` (built by build.rs)
- build.rs gated on `LLAMA_SERVER_BUILD=1` (normal builds skip it)
- Test model: `Qwen/Qwen2.5-0.5B-Instruct-GGUF` (q4_k_m, ~370MB)
- Default server port: 8999
- All tests are `#[ignore]`-gated behind `LLAMA_SERVER_TEST=1`
