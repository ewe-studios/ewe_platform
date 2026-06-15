---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/21-testing-strategy"
this_file: "specifications/36-agentic-api/features/21-testing-strategy/start.md"
created: 2026-06-14
---

# Start: Testing Strategy

## Agent Workflow

1. Read `feature.md` + Decision 17. Note: MockModelProvider is **ModelInteraction-driven, NOT regex**
   (user correction). Read the real `ModelInteraction` (`types/mod.rs:1080`), `Messages::Assistant`
   (:899), the existing `foundation_testing::huggingface::TestHarness`
   (`tests/llamacpp_integration.rs:12`), and `valtron::initialize_pool` (`executors/non_sendables.rs:35`).
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`. Memory
   `feedback_no_bespoke_test_machinery` — build mocks from foundation capabilities, don't invent infra.
3. Read `../../LEARNINGS.md`. Resolve OD-21-1..5. **OD-21-1 (interaction matchers) + OD-21-2 (mock
   implements F12 RoutableProvider, not the assoc-type ModelProvider)** — surface for confirmation.
   Pool annotations are mandatory: `initialize_pool` first line, `#[serial]`, `#[ntest::timeout]`.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** MockModelProvider (RoutableProvider) → message builders → MockTool → standard
   matchers → pool-annotation template → integration/e2e via TestHarness → wasm tier.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm, deterministic + integration tiers); update `../../LEARNINGS.md`. DONE —
   this is the last feature (32).

---

**Workflow:** feature.md → Decision 17 → real ModelInteraction + TestHarness + initialize_pool → Resolve OD-21 (flag 1+2) → Compact → ONE ITEM → fundamentals → Report → Verify → DONE

---

_Created: 2026-06-14_
