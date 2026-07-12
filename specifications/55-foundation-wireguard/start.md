---
workspace_name: "ewe_platform"
spec_directory: "specifications/55-foundation-wireguard"
this_file: "specifications/55-foundation-wireguard/start.md"
feature_name: "overview"
created: 2026-07-12
updated: 2026-07-12
---

# Start: `foundation_wireguard` — userspace WireGuard mesh

## What this is

A userspace WireGuard® crate: a service stands up (or joins) a **private encrypted mesh** from a
single **bootstrap secret**, and only members can talk to each other. Membership spreads by
**masterless SWIM gossip** (no single point of failure). Runs on **native, wasm/browser, Android,
iOS**. Bootstrap over **TCP+TLS-PSK** (`boring`), steady-state WireGuard (`boringtun`) with a
**dual data plane** (smoltcp userspace netstack + kernel TUN) added to `foundation_nativeapis`.

## Workflow

1. Read `requirements.md` (the vision, layering, feature map, success criteria).
2. Read every file under `decisions/` — **all architectural forks are resolved there**. Do not
   re-open a resolved decision; decisions 12 and 13 are *proposed* and may be adjusted at review.
3. Read the feature you are implementing under `features/NN-*/feature.md`, then its `start.md`.
4. **Retrieval first** — read the real code these build on before writing:
   - `boringtun` core: `@formulas/.../src.cloudflare/boringtun/boringtun/src/noise/mod.rs`
     (`Tunn::new/encapsulate/decapsulate/update_timers`, `TunnResult`).
   - `foundation_nativeapis/src/native/net/udp.rs` (existing `UdpSocket`).
   - `foundation_netio/src/quic/` (`quinn-proto` valtron driver) + `foundation_netio/src/http3/`
     (for WebTransport, feature 06).
   - `foundation_connectrpc` shared/native split (for the Join RPC + wss client, F51 WebSocket).
   - `foundation_proxy` (`config.rs`, `lib.rs`) + `foundation_macros/src/proxy.rs` (the tri-config
     + macro pattern feature 09 mirrors).
   - `foundation_iogate` (F48 bridge crate) — the pattern for avoiding netio↔nativeapis cycles.
   - `smoltcp` vendor: `@formulas/.../src.cloudflare/smoltcp/` (sans-I/O poll model).
   - `web-transport-proto`/`-wasm`: `@formulas/.../src.WebTransport/src.MoqDev/web-transport/rs/`.
5. Read skills: `rust-clean-code`, `rust-valtron-usage`, and the specifications-management skill.

## Build order (finish each 100% before the next — `feedback_finish_features`)

`00 → 01 → 02 → 03 → 04` is the critical path to a working native mesh.
`05` (relay/NAT), `06`+`07` (WebTransport→wasm/WebRTC), `08` (identity/mTLS), `09` (config/macro),
`10` (deployment) follow. `06` has no deps and can be built in parallel by a separate track.

## Non-negotiables

- **Zero tokio.** All async on **valtron**. Sans-I/O cores (`Tunn`, smoltcp, SWIM,
  `web-transport-proto`) are *driven* from valtron tasks, one task driving `Tunn` timers + smoltcp
  `poll()` together.
- **Cross-platform structure** `shared/ native/ wasm/ android/ ios/`; `shared/` is cross-platform
  only (no platform re-exports — `feedback_shared_module_purpose`).
- Tests in `tests/`, run with `--profile uat`; use `#[valtron_test]` for pool tests; WHY/WHAT/HOW
  docs; imports at file top; `tracing` not `eprintln`.
- No silent defaults for identity/name/endpoint — error or panic.

_Created: 2026-07-12_
