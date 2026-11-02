---
feature: "Move inline #[cfg(test)] modules out of lane A crates (rule 7)"
status: "pending"
lane: "A"
model: "haiku"
branch: "fix/io-41-50-lane-a"
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
