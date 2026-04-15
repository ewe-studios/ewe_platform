# Foundation ErrStacks - Implementation Progress

**Specification:** [requirements.md](./requirements.md)
**Created:** 2026-04-12
**Status:** In Progress — Phase 1 complete (5/5 tasks)

---

## Implementation Checklist

28 tasks across 6 phases (mirrors requirements.md §8).

### Phase 1: Core Types (5 tasks)

- [x] **Task 1.1**: Create `foundation_errstacks` crate skeleton in `backends/foundation_errstacks/` (Cargo.toml with full feature matrix, lib.rs with `no_std`+`extern crate alloc`, module decls, tests/ scaffold)
- [x] **Task 1.2**: Implement `ErrorTrace<C>` struct with frame storage (new, change_context, attach, attach_opaque, attach_with, attach_opaque_with, frames, current_context, downcast_ref, contains)
- [x] **Task 1.3**: Implement `Frame`, `FrameImpl`, `FrameIter`, `FrameKind`, `AttachmentKind`, `PrintableAttachment` (plus private `ContextFrame`/`PrintableFrame`/`OpaqueFrame`)
- [x] **Task 1.4**: Implement `PlainResultExt` trait for `Result<T, E>` (plain errors) and `ErrorTraceResultExt` for `Result<T, ErrorTrace<C>>`
- [x] **Task 1.5**: Implement `IntoErrorTrace` trait for error conversion

### Phase 2: Formatting & Output (4 tasks)

- [ ] **Task 2.1**: Implement `Display` for `ErrorTrace` (basic and alternate)
- [ ] **Task 2.2**: Implement `Debug` for `ErrorTrace` with tree visualization
- [ ] **Task 2.3**: Implement location capture using `core::panic::Location`
- [ ] **Task 2.4**: Add optional backtrace capture (feature-gated)

### Phase 3: Serialization & Integration (4 tasks)

- [ ] **Task 3.1**: Implement `Serialize` for `ErrorTrace` (serde feature)
- [ ] **Task 3.2**: Implement `to_structured()` method for JSON output
- [ ] **Task 3.3**: Implement `to_slack_blocks()` helper (slack feature)
- [ ] **Task 3.4**: Add `derive_more` integration examples in documentation

### Phase 4: Testing & Documentation (5 tasks)

- [ ] **Task 4.1**: Write unit tests for core types (in `tests/` directory per Rust skill)
- [ ] **Task 4.2**: Write integration tests for context changes
- [ ] **Task 4.3**: Add compile-fail tests for type safety
- [ ] **Task 4.4**: Write comprehensive crate-level documentation (WHY/WHAT/HOW)
- [ ] **Task 4.5**: Add usage examples to `examples/` directory

### Phase 5: Integration (3 tasks)

- [ ] **Task 5.1**: Integrate `foundation_errstacks` into `foundation_auth` crate
- [ ] **Task 5.2**: Migrate existing error handling to use `ErrorTrace`
- [ ] **Task 5.3**: Verify Slack alert formatting works end-to-end

### Phase 6: `no_std` Support (7 tasks)

- [ ] **Task 6.1**: Configure `Cargo.toml` features — `std` default, `alloc` baseline, `backtrace` gated on `std`; set `derive_more` to `default-features = false`.
- [ ] **Task 6.2**: Add `#![cfg_attr(not(feature = "std"), no_std)]` and `extern crate alloc;` to `lib.rs`.
- [ ] **Task 6.3**: Replace `std::` with `alloc::`/`core::` equivalents throughout the crate (`Box`, `Vec`, `String`, `fmt`, `any`, `slice`, `panic::Location`).
- [ ] **Task 6.4**: Feature-gate `std::backtrace::Backtrace` capture and its field behind `#[cfg(feature = "std")]`.
- [ ] **Task 6.5**: Switch the error trait bound to `core::error::Error` (requires MSRV 1.81).
- [ ] **Task 6.6**: CI — add `cargo build --no-default-features --features alloc` verification to workspace checks.
- [ ] **Task 6.7**: CI — add a `no_std` target smoke build (`thumbv7em-none-eabi`) to catch accidental `std::` leaks.

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-04-12 | Specification created | claude-code |
| 2026-04-12 | Specification reviewed and enhanced | claude-code |
| 2026-04-15 | Phase 1 Tasks 1.1–1.3 implemented (TDD). 6 integration tests pass; clippy/fmt/no_std-alloc all clean. Paused for review. | implementation-agent |
| 2026-04-15 | Fixup: spec §4.1.1 updated to `Vec<Frame>` (clippy::box_collection); Decision Log entry 7 added; PROGRESS.md regenerated with Phase 6 tasks; workspace Cargo.toml dependency entry added. | implementation-agent |
| 2026-04-15 | Phase 1 Tasks 1.4–1.5 complete: `PlainResultExt` (for `Result<T, E>`), `ErrorTraceResultExt` (for `Result<T, ErrorTrace<C>>`), `IntoErrorTrace`. 23 tests pass; clippy/fmt clean. | implementation-agent |

---

## Notes

- **MSRV:** 1.81.0 (per requirements.md — required for `core::error::Error`)
- **Primary dependencies:** `derive_more` (`default-features = false`, features: display/error/from), `tracing`
- **Optional features:** `serde`, `backtrace`, `async`, `slack`
- **Tests live in:** `backends/foundation_errstacks/tests/mod.rs` → `tests/units/errstacks_*_tests.rs` (integration-test binary), per the rust-clean-code testing skill.

---

## Verification Status

| Check | Status | Date |
|-------|--------|------|
| `cargo fmt --check` | Pass | 2026-04-15 |
| `cargo clippy -- -D warnings` | Pass | 2026-04-15 |
| All tests passing | 23/23 pass | 2026-04-15 |
| Documentation builds | Pending | - |
