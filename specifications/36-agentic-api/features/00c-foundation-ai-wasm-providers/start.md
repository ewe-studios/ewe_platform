---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/00c-foundation-ai-wasm-providers"
this_file: "specifications/36-agentic-api/features/00c-foundation-ai-wasm-providers/start.md"
created: 2026-06-14
---

# Start: foundation_ai — wasm-safe providers + wasm build

## Agent Workflow

1. Read `feature.md` (cooperative backoff + optional native deps + wasm build; Open Decisions).
2. **Stack:** Rust + valtron `Stream`. Read `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-iterator/skill.md` (the provider streams are TaskIterators).
3. Read the 5 `thread::sleep` sites + their enclosing provider stream state machines. Read
   `foundation_core/src/valtron/streams.rs` (`Stream::Delayed`).
4. Read `../../LEARNINGS.md`; confirm 00b landed (llama optional, native backend-less build green).
5. Grep tests + `lib.rs` for `foundation_deployment`/`foundation_auth`/`chrono` usage to resolve
   remove-vs-gate (OD-00c-2/3).
6. Generate `compacted.md`, clear, reload.
7. **One item at a time:** convert one provider's backoff to `Stream::Delayed` → native green +
   backoff test → next provider → … → gate deps → wasm build green.
8. Report to Main Agent (no commit). Wait for verification.
9. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to Feature 03.
10. **ALWAYS UPDATE ../../LEARNINGS.md.**

## Critical Notes

- Backoff = `Stream::Delayed(duration)` / `TaskStatus::Delayed` (the valtron executor yields to the
  JS event loop — `foundation_core/.../local.rs:2682` — and reschedules the wake); `TaskStatus::Wait`
  yields to the event loop too. NOT `thread::sleep`, and NOT `SleepIterator`. Native total backoff
  must be preserved.
- Verify the HTTP transport itself compiles on wasm (OD-00c-5) — if not, document the gap.

---

**Workflow:** feature.md → valtron Stream skill → sleep sites → Compact → ONE PROVIDER AT A TIME → native green → gate deps → wasm green → Report → Verify → 02

---

_Created: 2026-06-14_
