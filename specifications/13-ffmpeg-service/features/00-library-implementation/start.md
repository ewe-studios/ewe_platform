---
feature: "00-library-implementation"
spec_directory: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter"
feature_directory: "services/call-quality-ffmpeg/specifications/DE-4295-ffmpeg-opus-converter/features/00-library-implementation"
created: 2026-04-10
last_updated: 2026-04-10
---

# Start: Feature 00 — Library Implementation

## Step 1: Read Requirements
Read `feature.md` for the full task list (T01–T09) and technical notes.

## Step 2: Read Spec Context
Read `../../requirements.md` for architecture overview and design decisions.

## Step 3: Read Agent Documentation
Read `../../../../../../.agents/agents/implementation.md` for your implementation workflow.

## Step 4: Read Skills
Load only the skills specified in your agent documentation.

## Step 5: Survey Existing Code
```bash
cat packages/ffmpeg-converter/Cargo.toml
cat packages/ffmpeg-converter/src/lib.rs
```

## Step 6: Generate compacted.md
Write `compacted.md` in this directory summarising what you have read.  
Clear context, then reload from `compacted.md` before coding.

## Step 7: Implement — ONE Task at a Time

Work through tasks T01 → T09 in order. Do NOT implement multiple tasks simultaneously.

### Task Order (respect dependencies)
1. **T01** — Update `Cargo.toml` with all dependencies
2. **T02** — Define `ConversionFormat` enum in `converter.rs`
3. **T08** — Define `ConverterError` in `error.rs`
4. **T03** — Define `FileStore` trait in `file_store.rs`
5. **T04** — Implement `LocalFileStore`
6. **T05** — Implement `S3FileStore`
7. **T06** — Implement `GcsFileStore`
8. **T07** — Implement `OpusConverter`
9. **T09** — Update `lib.rs` re-exports

## Step 8: Verify After Each Task
```bash
cargo build -p ffmpeg-converter
cargo clippy -p ffmpeg-converter -- -D warnings
```

## Step 9: TDD — Write Tests Alongside Code
Write unit tests for each module as you implement it. Integration tests live in feature 01.

## Step 10: Final Verification
```bash
cargo build --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

## Step 11: Update feature.md
Mark each completed task `[x]` and update `tasks.completed` / `tasks.completion_percentage`.

## Step 12: Report to Main Agent
Report:
- What was implemented
- Any deviations from spec (with justification)
- Verification output (pass/fail)

## Step 13: Wait for Verification
Do NOT commit. Main Agent coordinates verification and commits.

---

## Key Files to Modify

| File | Action |
|---|---|
| `packages/ffmpeg-converter/Cargo.toml` | Add dependencies |
| `packages/ffmpeg-converter/src/lib.rs` | Re-exports |
| `packages/ffmpeg-converter/src/file_store.rs` | Create — trait + impls |
| `packages/ffmpeg-converter/src/converter.rs` | Create — OpusConverter + ConversionFormat |
| `packages/ffmpeg-converter/src/error.rs` | Create — ConverterError |

---

_Feature: 00-library-implementation | Ticket: DE-4295_
