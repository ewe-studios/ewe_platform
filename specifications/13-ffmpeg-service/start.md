---
workspace_name: "clover-ma-pipelines"
spec_directory: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter"
this_file: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter/start.md"
created: 2026-04-10
last_updated: 2026-04-10
---

# Start: DE-4295 FFmpeg Opus Converter

## Quick Reference

**Ticket**: DE-4295  
**Status**: In Progress (0% complete — 0 of 5 features done)  
**Next Feature**: `00-library-implementation`

## Agent Workflow

### Step 1: Understand the Specification
Read `requirements.md` to understand:
- Overall goals and architecture
- Feature index (4 features total)
- Key design decisions
- Success criteria

### Step 2: Identify Your Task
Check the Feature Index in `requirements.md`:

| # | Feature | Status |
|---|---|---|
| 00 | library-implementation | ⬜ pending ← **START HERE** |
| 01 | library-tests | ⬜ pending |
| 02 | cli-binary | ⬜ pending |
| 03 | lambda-worker | ⬜ pending |
| 04 | ci-docker | ⬜ pending |
| 05 | slack-alerter | ⬜ pending |

### Step 3: Navigate to the Feature Directory

```bash
cd features/[NN-feature-name]/
```

### Step 4: Follow the Feature `start.md`

Each feature directory has its own `start.md` with a complete 13-step implementation workflow.

## Feature Directory Structure

### Pending Features ⬜

- **`features/00-library-implementation/`** ← **START HERE**  
  Core library: `FileStore` trait, `OpusConverter`, `ConversionEnum`, upload support

- `features/01-library-tests/`  
  Unit + integration tests for the library crate *(depends on 00)*

- `features/02-cli-binary/`  
  `bin/converter` CLI with clap *(depends on 00)*

- `features/03-lambda-worker/`  
  `bin/lambda_worker` AWS Lambda binary *(depends on 00)*

## Key Files Reference

| File | Purpose |
|---|---|
| `packages/ffmpeg-converter/src/lib.rs` | Crate root |
| `packages/ffmpeg-converter/src/file_store.rs` | `FileStore` trait + implementations |
| `packages/ffmpeg-converter/src/converter.rs` | `OpusConverter` + `ConversionEnum` |
| `packages/ffmpeg-converter/tests/` | Integration tests |
| `bin/converter/src/main.rs` | CLI entry point |
| `bin/lambda_worker/src/main.rs` | Lambda entry point |

## Quick Commands

```bash
# Build entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

---

**Next Action**: Navigate to `features/00-library-implementation/start.md` to begin.

_Created: 2026-04-10_
