---
workspace_name: "ewe_platform"
spec_directory: "specifications/55-foundation-wireguard"
this_file: "specifications/55-foundation-wireguard/features/06-webtransport-netio/start.md"
feature_name: "06-webtransport-netio"
created: 2026-07-12
updated: 2026-07-12
---

# Start: WebTransport in foundation_netio

## Workflow

1. Read this feature's `feature.md` (full architecture, tasks, test plan).
2. Read the spec-level `../../start.md` and `../../requirements.md` (vision, layering, constraints).
3. Read the governing decisions: decision 09 (under `../../decisions/`). Do not re-open resolved
   decisions; proposed ones (12, 13) may be adjusted at review.
4. Retrieval first — read before writing: foundation_netio/src/quic (quinn-proto) + src/http3; @formulas web-transport-proto/-wasm.
5. Read skills `rust-clean-code` and `rust-valtron-usage`.

## Non-negotiables

- Zero tokio; async on valtron; sans-I/O cores driven from valtron tasks
  (`BoxedSendExecutionAction`, never `NoAction`).
- Cross-platform `shared/ native/ wasm/ android/ ios/`; `shared/` is cross-platform only.
- Tests in `tests/`, run with `--profile uat`; `#[valtron_test]` for pool tests; WHY/WHAT/HOW docs;
  imports at file top; `tracing` not `eprintln`; no silent defaults.
- Finish this feature 100% (tests + examples green, decision-doc conformance) before the next.

_Created: 2026-07-12_
