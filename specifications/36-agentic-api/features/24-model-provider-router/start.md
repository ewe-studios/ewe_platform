---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/24-model-provider-router"
this_file: "specifications/36-agentic-api/features/24-model-provider-router/start.md"
created: 2026-06-14
---

# Start: ModelProviderRouter

## Agent Workflow

1. Read `feature.md`. Read the REAL `ModelProvider` (`types/mod.rs:1461`, associated `Config`/`Model`),
   `Model` (:1391, `type Formatter` + `stream -> impl StreamIterator`), `ModelId` (:362),
   `ModelProviderDescriptor` (:254), `ModelProviderErrors` (errors :188). Confirm `dyn ModelProvider`
   is impossible — the erased `RoutableProvider` adapter is mandatory.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
3. Read `../../LEARNINGS.md`. Resolve OD-24-1..5. **OD-24-1 (erased surface reshapes Decision 18's
   `Arc<dyn ModelProvider>`) needs a user ruling** — surface it.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** RoutableProvider + ProviderAdapter → ProviderRouter single/builder →
   resolve → generate/stream → memory/embedding model helpers.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 25.

---

**Workflow:** feature.md → real ModelProvider/Model traits → object-safety → Resolve OD-24 (flag 1) → Compact → ONE ITEM → fundamentals → Report → Verify → 25

---

_Created: 2026-06-14_
