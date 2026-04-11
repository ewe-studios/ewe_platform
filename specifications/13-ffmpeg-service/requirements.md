---
description: "Rust ffmpeg-based opus audio converter: library crate, CLI binary, and AWS Lambda worker with pluggable FileStore backends"
status: "in-progress"
priority: "high"
created: 2026-04-10
author: "Ewetumo Alexander"
metadata:
  version: "1.0"
  last_updated: 2026-04-10
  estimated_effort: "large"
  tags:
    - rust
    - ffmpeg
    - audio-conversion
    - lambda
    - s3
    - gcs
    - cli
    - error-stack
    - derive_more
  skills: [rust-patterns, testing, ffmpeg, aws-lambda, clap, error-handling]
  tools: [Read, Write, Edit, Bash, Grep, Glob]
  ticket: "DE-4295"
has_features: true
has_fundamentals: false
builds_on: ""
related_specs: []
features:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# DE-4295: FFmpeg Opus Converter

## Overview

Build a Rust workspace that uses `ffmpeg-sys-next` to convert opus audio files into selectable output formats (WAV, MP4, etc.).  
The system is composed of three crates:

| Crate | Path | Role |
|---|---|---|
| `ffmpeg-converter` | `packages/ffmpeg-converter` | Core library: `FileStore` trait, `OpusConverter`, `ConversionEnum` |
| `converter` | `bin/converter` | CLI binary (clap) |
| `lambda_worker` | `bin/lambda_worker` | AWS Lambda binary |

---

## Feature Index

| # | Name | Description | Status |
|---|---|---|---|
| 00 | library-implementation | `FileStore` trait + `OpusConverter` + upload support in `packages/ffmpeg-converter` | ⬜ pending |
| 01 | library-tests | Integration and unit tests for `packages/ffmpeg-converter` | ⬜ pending |
| 02 | cli-binary | `bin/converter` CLI with clap: input path, `--format` flag, runs `OpusConverter` | ⬜ pending |
| 03 | lambda-worker | `bin/lambda_worker` AWS Lambda: parses event, resolves `FileStore` from URL scheme, converts, delivers to SQS | ⬜ pending |
| 05 | slack-alerter | `packages/slack-alerter` library: `SlackAlerter::send_alert(message, payload)` for reporting failures to Slack | ⬜ pending |

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     packages/ffmpeg-converter                    │
│                                                                  │
│  FileStore (trait)          OpusConverter (struct)               │
│  ├── get_stream() → Read    ├── new(store: impl FileStore)       │
│  └── upload(path, Read)     └── convert(src, fmt: ConversionEnum)│
│                                                                  │
│  Implementations:           ConversionEnum                       │
│  ├── LocalFileStore         ├── Wave                             │
│  ├── S3FileStore            ├── MP4                              │
│  └── GcsFileStore           └── (extendable)                    │
└─────────────────────────────────────────────────────────────────┘
            ↑                            ↑
   bin/converter (CLI)        bin/lambda_worker (Lambda)
   clap args → FileStore      JSON event → FileStore resolver
   → OpusConverter            → OpusConverter → upload → response
```

---

## Key Design Decisions

1. **`FileStore` trait** abstracts over local FS, S3, and GCS — `OpusConverter` is backend-agnostic.
2. **`get_stream()`** returns `error_stack::Result<Box<dyn io::Read + Send>, ConverterError>` — callers pipe bytes into ffmpeg without materialising the full file; every failure site carries a file/line/column trace.
3. **`upload(dest_path, reader)`** on `FileStore` allows each backend to handle output upload independently.
4. **`ConversionFormat`** is the single source of truth for supported output formats; adding a new format only requires extending this enum.
5. **URL scheme dispatch** (`gs://`/`gcs://`, `s3://`, local path) in the Lambda resolves the concrete `FileStore` at runtime.
6. **Error strategy**: no `thiserror` or `anyhow`. All errors use `derive_more` (`Display`, `Error`) + `serde` (`Serialize`, `Deserialize`) for JSON-serializable error types, wrapped in `error-stack::Report<E>` for location-annotated traces (file, line, column captured at every `.change_context()` call site).

---

## Success Criteria (Spec-Wide)

- [ ] `cargo build --workspace` succeeds with no warnings
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] Library crate compiles without `ffmpeg` system library installed in CI (feature-gated or detected at link time)
- [ ] CLI converts a sample opus file to each supported format successfully
- [ ] Lambda handler returns a JSON response with the output file URL on success

---

## Constraints & Assumptions

- Rust edition 2024, rust-version 1.86 (per workspace Cargo.toml)
- `ffmpeg-sys-next` provides the low-level C bindings; the library must handle linking correctly (dynamic or static, documented in feature 00)
- GCS and S3 credentials are injected via environment variables / IAM roles — no credentials hard-coded
- The Lambda receives a single JSON event per invocation (no batching)
- Output file is written to the **same backend** as the source file (e.g., S3 in → S3 out, unless overridden)

---

## Module References

- `packages/ffmpeg-converter/src/lib.rs` — crate root, re-exports public API
- `packages/ffmpeg-converter/src/file_store.rs` — `FileStore` trait + all implementations
- `packages/ffmpeg-converter/src/converter.rs` — `OpusConverter` + `ConversionEnum`
- `packages/ffmpeg-converter/tests/` — integration test directory
- `bin/converter/src/main.rs` — CLI entry point
- `bin/lambda_worker/src/main.rs` — Lambda entry point
