---
feature: "01-library-tests"
spec_directory: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter"
feature_directory: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter/features/01-library-tests"
created: 2026-04-10
last_updated: 2026-04-10
---

# Start: Feature 01 — Library Tests

## Prerequisites
Feature 00 (library-implementation) must be completed before starting this feature.

## Step 1: Read Requirements
Read `feature.md` for the full task list (T01–T06) and test layout.

## Step 2: Read Source Code
```bash
cat packages/ffmpeg-converter/src/lib.rs
cat packages/ffmpeg-converter/src/file_store.rs
cat packages/ffmpeg-converter/src/converter.rs
cat packages/ffmpeg-converter/src/error.rs
```

## Step 3: Generate compacted.md
Write `compacted.md` summarising what you have read. Clear context, reload.

## Step 4: Implement — ONE Task at a Time

Work through T01 → T06 in order.

### Task Order
1. **T01** — Add test fixtures (`sample.opus`, `common/mod.rs`)
2. **T02** — Unit tests for `ConversionFormat`
3. **T03** — Unit tests for `LocalFileStore`
4. **T04** — Unit tests for `S3FileStore` (mocked)
5. **T05** — Unit tests for `GcsFileStore` (mocked)
6. **T06** — Integration tests for `OpusConverter`

## Step 5: Verify After Each Task
```bash
cargo test -p ffmpeg-converter
```

## Step 6: Final Verification
```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

## Step 7: Update feature.md
Mark tasks `[x]` and update counters.

## Step 8: Report to Main Agent
Report pass/fail and any skipped tests with justification.

## Step 9: Wait for Verification
Do NOT commit.

---

_Feature: 01-library-tests | Ticket: DE-4295_
