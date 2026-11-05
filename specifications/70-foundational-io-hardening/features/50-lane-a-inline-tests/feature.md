---
feature: "Move inline #[cfg(test)] modules out of lane A crates (rule 7)"
status: "done"
lane: "A"
model: "haiku"
branch: "spec70/f47-f50-lane-a-mechanical"
crates: ["lane A crates"]
catalogue_rows: [16]
---

# Feature 50: inline tests out (lane A)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem

`CLAUDE.md` rule 7: no `#[cfg(test)] mod tests` in source files; a change that touches such a
file moves its tests. PR #172 touched many of them (mostly through `cargo fmt --all`).

## Work

1. List: `rg -l '#\[cfg\(test\)\]' backends/{foundation_core,foundation_netio,foundation_http,foundation_reactor,foundation_nostd,foundation_nativeapis,foundation_keychain,foundation_proxy,foundation_testbed,foundation_deployment_docker,foundation_deployment,foundation_wireguard,foundation_compact,foundation_iogate,foundation_connectrpc}/src`
   (known: `foundation_nativeapis/src/ipc/memory_registry.rs`,
   `foundation_nativeapis/src/shared/ipc/{version,message}.rs`,
   `foundation_proxy/src/control.rs`, `foundation_keychain/src/core/util.rs`,
   `foundation_compact/src/ids/{generator,id}.rs`, `foundation_reactor/src/send_pool.rs`,
   `foundation_wireguard/src/ios/mod.rs`).
2. For each: move the test module to the crate's `tests/` (register in `tests/**/mod.rs`).
   Test only the public API. If a test needs a private item, make the right item public (as
   rule 7 says) or test it through the public behaviour; say which in the commit.
3. A `#[cfg(test)]` attribute on a non-test item (a test-only constructor) is also not
   allowed: replace it with a public, documented item or remove it.
4. One commit per crate.

## Verify

```bash
rg -l '#\[cfg\(test\)\]' backends/<each lane A crate>/src   # no output
python3 scripts/ci/ci.py run <each crate changed> --profile dev
```
The number of tests per crate before and after is the same or higher (write both in the PR).

## Done when

- [ ] No `#[cfg(test)]` in lane A `src` trees. Test counts kept. CI green.

## Result

2026-11-05. Every lane A source tree named in the feature has no `#[cfg(test)]`
module. The only remaining match is `foundation_nostd/src/comp/QUICKSTART.md`, a
documentation file, not Rust source.

| Crate | Tests moved | Before | After |
|---|---|---|---|
| `foundation_reactor` | `send_pool` | 5 | 5 |
| `foundation_keychain` | `core::util` | 2 | 2 |
| `foundation_nativeapis` | `MemoryRegistry`, IPC version, `BytesMessage` | 9 | 9 |
| `foundation_proxy` | control wire format | 4 | 4 |
| `foundation_testbed` | QEMU monitor, docker environment | 5 + 4 | 5 + 4 |
| `foundation_compact` | `Id`, serde `Id`, `Generator` | 5 + 10 | 5 + 10 |
| `foundation_wireguard` | iOS C FFI, Android JNI | 5 + 1 | 5 + 1 |

Rules that needed a public item:

- `foundation_proxy`: `Request` and `Response` in `control.rs` became public with
  documented fields.
- `foundation_testbed`: `strip_ansi` and `port_is_open` became public with doc
  comments.
- `foundation_compact`: `ParseErrorKind` and `ParseError::kind` became public
  with docs; `Version` uses its public `from_parts` constructor; the test-only
  `Generator::for_testing` is replaced by a local helper over the public
  `with_rand_and_time_sources`.
- `foundation_wireguard`: the `ios` and `android` modules were never declared in
  `lib.rs`, so their code and tests never compiled. They are now declared for
  their target OS and for `spec55-complete`.

No consumer calls the new public items, so no version bump is made. The mobile
targets still need a real iOS/Android build check (backlog §8).
