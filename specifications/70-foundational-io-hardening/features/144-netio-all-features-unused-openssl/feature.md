---
feature: "The all-features netio check is warning-free"
status: "dropped"
lane: "priority follow-up (found under feature 142)"
model: "haiku"
branch: "spec70/netio-all-features-warning"
crates: ["foundation_netio"]
catalogue_rows: ["2"]
---

# Feature 144: the all-features check stops reporting an unused `openssl`

**Dropped as a feature and fixed directly on 2026-11-06.** The owner closed new feature
opening for spec 70. The item was GitHub issue
[#188](https://github.com/ewe-studios/ewe_platform/issues/188), severity `low`. The fix
is the scoped `#[cfg(all(feature = "ssl-openssl", feature = "ssl-rustls"))] use openssl
as _;` in `backends/foundation_netio/src/native/ssl/mod.rs`. `cargo check --profile uat
-p foundation_netio --all-features` now prints 0 warnings, and the netio suite passes
(1298 tests). The backlog entry is deleted.

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (found under feature 142 on 2026-11-06, backlog §2)

`cargo check --profile uat -p foundation_netio --all-features` prints:

```text
warning: unused dependency `openssl`
   = note: `cargo::unused_dependencies` is set to `warn` by default
warning: `foundation_netio` (manifest) generated 1 warning
```

The crate prefers rustls over openssl when both backends are on, so no code names
`openssl` in that build. Each backend alone builds clean. A warning in a supported
command is debt: a later gate that runs `--all-features` with `-D warnings` fails.

## Design

Pick the first option that keeps both backends supported:

1. In the TLS module of `backends/foundation_netio/src/native/`, add
   `#[cfg(all(feature = "ssl-openssl", feature = "ssl-rustls"))] use openssl as _;`
   with a comment that rustls wins. This marks the crate as used and changes no
   behaviour.
2. If the import is refused by a lint (for example `unused_imports`), declare the two
   features mutually exclusive with a `compile_error!` and say in the crate docs which
   backend to select. This removes the contradictory build instead of tolerating it.

Do not add a whole-manifest `unused_dependencies = "allow"`; it hides real unused
dependencies.

## Verify

```bash
cargo fmt --all --check
cargo check --profile uat -p foundation_netio --all-features
cargo check --profile uat -p foundation_netio --no-default-features
cargo clippy --profile uat -p foundation_netio --all-targets --all-features --no-deps -- -D warnings
cargo test --profile uat -p foundation_netio
```

## Done when

- [ ] The all-features check prints no warning.
- [ ] Each single backend and the default set stay clean.
- [ ] The backlog §2 entry is deleted.
- [ ] CI green.
