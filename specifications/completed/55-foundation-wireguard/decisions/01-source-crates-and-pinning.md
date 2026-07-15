# 01 — Source Crates & Version Pinning

**Date:** 2026-07-12
**Status:** Resolved

## Decision

`foundation_wireguard` builds on three external crates, **pinned to their published crates.io
versions** (not git), even though we track upstream *master* for review:

| Crate | Pin | Role |
|-------|-----|------|
| `boringtun` | `= "0.7.1"` | Userspace WireGuard Noise state machine (`noise::Tunn`) + x25519 re-export |
| `boring` | `= "<latest crates.io>"` (resolve at manifest time) | BoringSSL bindings for the **TLS-PSK** bootstrap/control channel |
| `smoltcp` | `= "0.8"` (or latest crates.io) | Userspace TCP/IP netstack — added to `foundation_nativeapis` |

## Why

- The owner explicitly requires **crates.io releases**, not git dependencies, for supply-chain
  reproducibility and to match what the wider ecosystem audits. Local checkouts of `boringtun`
  (master `@6dcc889`) and `boring` are for *reading and reference*; the manifest pins the release.
- `boringtun 0.7.1` is the current published release; local master is only 3 commits past the
  `boringtun-cli-0.7.1` tag with **no library version bump**, so `0.7.1` == master for our purposes.
  0.7.0 made `Tunn::new` infallible (breaking) and 0.7.1 fixed a 32-bit nonce-reuse security issue —
  we want ≥ 0.7.1.
- `boring` is Cloudflare's BoringSSL binding and is the reason it sits alongside `boringtun` in the
  sources: **BoringSSL supports external PSK ciphersuites**, which we need for the seed-authenticated
  TLS bootstrap ([decision 06](06-hybrid-transport-tls-psk.md)). rustls' external-PSK support is far
  less mature, so `boring` is the right tool for the control channel specifically.
- `smoltcp` is the userspace protocol stack that makes the portable data plane possible
  ([decision 02](02-dual-dataplane.md)); it is `no_std`, heap-optional, and has **no async runtime**.

## What we do

1. Pin the three crates in the relevant manifests: `boringtun` + `smoltcp` primitives referenced by
   `foundation_nativeapis` (smoltcp) and `foundation_wireguard` (boringtun); `boring` referenced
   where the TLS-PSK channel lives ([decision 06](06-hybrid-transport-tls-psk.md) — likely
   `foundation_wireguard::native`, possibly surfaced through `foundation_netio` TLS).
2. Enable only the `boringtun` features we need. Default features are empty; we use the **`noise`**
   core (always compiled) and **do not** enable `device` (its kernel-TUN reactor is Linux/Darwin,
   privileged, and thread-per-tunnel) — we build our own dual data plane instead
   ([decision 02](02-dual-dataplane.md)). We may consult `device/tun_linux.rs`/`tun_darwin.rs` as a
   reference for our own TUN wrapper.
3. Enable `smoltcp` with `medium-ip`, `proto-ipv4`, `proto-ipv6`, `socket-tcp`, `socket-udp`, and
   **without** `std`/`phy-*` where wasm requires `no_std` (the phy layer is *us* — the `Tunn`
   plaintext side — not smoltcp's raw-socket/tuntap phys).
4. `boringtun` re-exports `x25519_dalek` types via `boringtun::x25519`; we use those directly rather
   than adding a separate `x25519-dalek` dependency, to guarantee ABI/key-type compatibility with
   `Tunn`.

## Notes & risks

- `boringtun` pulls `ring` (0.17) and `x25519-dalek` (2.x); `boring` pulls a vendored/system
  BoringSSL. Two different crypto backends coexist (ring for WG, BoringSSL for the TLS control
  channel) — acceptable, they serve different layers.
- If `boring`'s build (BoringSSL via cmake/Go toolchain) proves painful in some CI/wasm targets,
  note that **`boring` is native-only** anyway — the browser uses `web_sys` WebTransport/`wss` TLS
  from the platform, not `boring` ([decision 08](08-wasm-browser-and-webrtc.md)). So `boring` is
  target-gated `cfg(not(wasm32))`.

## Related decisions

- [02 — Dual data plane](02-dual-dataplane.md)
- [06 — Hybrid transport & TLS-PSK](06-hybrid-transport-tls-psk.md)
- [09 — WebTransport on quinn-proto](09-webtransport-on-quinn-proto.md)
