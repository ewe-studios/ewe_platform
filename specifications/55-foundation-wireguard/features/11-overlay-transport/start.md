---
workspace_name: "ewe_platform"
spec_directory: "specifications/55-foundation-wireguard"
this_file: "specifications/55-foundation-wireguard/features/11-overlay-transport/start.md"
feature_name: "11-overlay-transport"
created: 2026-07-13
updated: 2026-07-13
---

# Start: Overlay Transport Bridge

## Workflow

1. Read this feature's `feature.md` (full architecture, tasks, test plan).
2. Read the spec-level `../../start.md` and `../../requirements.md`.
3. Read `../../decisions/02-dual-dataplane.md` and `../../decisions/14-crate-layering.md`.
4. Read `foundation_netio::native::connection` (`CompletionReadWrite` pattern — the reference).
5. Read `foundation_iogate::native::completion_socket` (`CompletionReadWrite` impl — the impl-side pattern).

## Non-negotiables

- Zero tokio. Overlay HTTP runs on the sans-I/O smoltcp netstack, polled by the mesh runtime thread.
- No crate cycles. The `OverlayReadWrite` trait lives in `foundation_netio`. The impl lives in `foundation_wireguard`. Wireguard depends on netio — one-way.
- Tests in `tests/`, run with `--profile uat`; WHY/WHAT/HOW docs; imports at file top; `tracing` not `eprintln`.
- Finish this feature 100% before the next.

_Created: 2026-07-13_
