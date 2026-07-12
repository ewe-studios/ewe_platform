---
workspace_name: "ewe_platform"
spec_directory: "specifications/55-foundation-wireguard"
this_file: "specifications/55-foundation-wireguard/features/08-identity-handoff-and-mtls/start.md"
feature_name: "08-identity-handoff-and-mtls"
created: 2026-07-12
updated: 2026-07-12
---

# Start: Identity Handoff & Optional mTLS

## Workflow

1. Read this feature's `feature.md` (full architecture, tasks, test plan).
2. Read the spec-level `../../start.md` and `../../requirements.md` (vision, layering, constraints).
3. Read the governing decisions: decisions 10, 11, 03 (under `../../decisions/`). Do not re-open resolved
   decisions; proposed ones (12, 13) may be adjusted at review.
4. Retrieval first — read before writing: feature 04 mesh; @formulas boring (mTLS).
5. Read skills `rust-clean-code` and `rust-valtron-usage`.

## Non-negotiables

- Zero tokio; async on valtron; sans-I/O cores driven from valtron tasks
  (`BoxedSendExecutionAction`, never `NoAction`).
- Cross-platform `shared/ native/ wasm/ android/ ios/`; `shared/` is cross-platform only.
- Tests in `tests/`, run with `--profile uat`; `#[valtron_test]` for pool tests; WHY/WHAT/HOW docs;
  imports at file top; `tracing` not `eprintln`; no silent defaults.
- Finish this feature 100% (tests + examples green, decision-doc conformance) before the next.

_Created: 2026-07-12_
