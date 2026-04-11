---
feature: "03-lambda-worker"
spec_directory: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter"
feature_directory: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter/features/03-lambda-worker"
created: 2026-04-10
last_updated: 2026-04-10
---

# Start: Feature 03 — Lambda Worker

## Prerequisites
Feature 00 (library-implementation) must be completed before starting this feature.

## Step 1: Read Requirements
Read `feature.md` for the full task list (T01–T07), event schema, and environment variables.

## Step 2: Survey Existing Code
```bash
cat bin/lambda_worker/Cargo.toml
cat bin/lambda_worker/src/main.rs
cat packages/ffmpeg-converter/src/lib.rs
```

## Step 3: Read Spec Context
Read `../../requirements.md` for the URL scheme → FileStore mapping.

## Step 4: Generate compacted.md
Summarise what you have read. Clear context, reload.

## Step 5: Implement — ONE Task at a Time

1. **T01** — Update `Cargo.toml`
2. **T02** — Define `ConversionEvent` and `ConversionResponse` types in `src/event.rs`
3. **T03** — Implement URL scheme resolver in `src/store_resolver.rs`
4. **T04** — Implement default output path derivation
5. **T05** — Implement the Lambda handler function
6. **T06** — Implement `main` with `lambda_runtime` bootstrap
7. **T07** — Handle error mapping

## Step 6: Verify After Each Task
```bash
cargo build -p lambda_worker
cargo clippy -p lambda_worker -- -D warnings
```

## Step 7: Final Verification
```bash
cargo build --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

## Step 8: Smoke Test (optional, requires cargo-lambda)
```bash
cargo lambda invoke --data-file tests/fixtures/sample_event.json
```

## Step 9: Update feature.md, Report, Wait

---

## Key Files to Create / Modify

| File | Action |
|---|---|
| `bin/lambda_worker/Cargo.toml` | Add dependencies |
| `bin/lambda_worker/src/main.rs` | Handler + main |
| `bin/lambda_worker/src/event.rs` | Event/response types |
| `bin/lambda_worker/src/store_resolver.rs` | URL scheme resolver |

---

_Feature: 03-lambda-worker | Ticket: DE-4295_
