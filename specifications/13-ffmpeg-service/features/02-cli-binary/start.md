---
feature: "02-cli-binary"
spec_directory: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter"
feature_directory: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter/features/02-cli-binary"
created: 2026-04-10
last_updated: 2026-04-10
---

# Start: Feature 02 — CLI Binary

## Prerequisites
Feature 00 (library-implementation) must be completed before starting this feature.

## Step 1: Read Requirements
Read `feature.md` for the task list (T01–T05) and the CLI interface spec.

## Step 2: Survey Existing Code
```bash
cat bin/converter/Cargo.toml
cat bin/converter/src/main.rs
```

## Step 3: Generate compacted.md
Summarise what you know. Clear context, reload.

## Step 4: Implement — ONE Task at a Time

1. **T01** — Update `Cargo.toml`
2. **T02** — Define `Cli` struct
3. **T03** — Implement `FormatArg` enum
4. **T04** — Implement format resolution logic
5. **T05** — Implement `main`

## Step 5: Verify After Each Task
```bash
cargo build -p converter
cargo clippy -p converter -- -D warnings
```

## Step 6: Smoke Test
```bash
cargo run -p converter -- --help
cargo run -p converter -- path/to/sample.opus --format wave
```

## Step 7: Final Verification
```bash
cargo build --workspace
cargo clippy --workspace -- -D warnings
```

## Step 8: Update feature.md, Report, Wait

---

_Feature: 02-cli-binary | Ticket: DE-4295_
