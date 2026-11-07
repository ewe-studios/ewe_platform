---
feature: "A failed hole punch with no relay returns an error, not a panic"
status: "in-review"
priority: "medium"
depends_on: []
crates: ["foundation_wireguard"]
issues: [223]
model: "sonnet"
branch: "spec72/f03-wireguard-hole-punch"
---

# Feature 03: Wireguard hole-punch error

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

## 1. Goal

Replace the panic in `RelaySession::on_direct_failure` with a typed error or a documented
fallback. Close issue [#223](https://github.com/ewe-studios/ewe_platform/issues/223).

## 2. Problem

`foundation_wireguard::RelaySession::on_direct_failure` panics when the path is `HolePunching`
and `relay` is `None`. A peer that fails a punch in that state takes the process down instead
of falling back or reporting an error.

## 3. Requirements

1. The function returns the crate's existing error type. Do not add a new error type if one
   fits.
2. The error names the state: hole punching failed and no relay is configured.
3. No `unwrap`, `expect` or `panic!` remains on this path.
4. The caller decides the next step. If the caller already has a direct-path fallback, use it
   and document it.

## 4. Design

Return `Err(<crate error>::NoRelayForPunchedSession)` (exact name per the crate's error enum)
or the closest existing variant. Audit every caller of `on_direct_failure` in the crate and in
`foundation_wireguard`'s users. A caller that cannot handle the error must log it and end the
session, not ignore it (root rule: do not ignore a return value).

## 5. Tasks

- [x] Read `RelaySession::on_direct_failure` and the crate error enum.
- [x] Add the variant or reuse one. Return the error.
- [x] Update every caller.
- [x] Add the regression test.
- [x] Run the verify commands. Record the results.
- [x] Close #223 in the PR body.

## 6. Tests

In `backends/foundation_wireguard/tests/`:

1. `a_failed_punch_without_relay_returns_an_error`: build a session in the `HolePunching` path
   with `relay = None`, call `on_direct_failure`, and assert an `Err`. On the old code the
   call panics; the test fails with the panic before the fix.
2. `a_failed_punch_with_a_relay_falls_back`: the existing relay fallback still works.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_wireguard
cargo clippy --profile uat -p foundation_wireguard --all-targets -- -D warnings
cargo test --profile uat -p foundation_wireguard
```

## 8. Done when

- No panic remains on the failed-punch path.
- The new test fails on the old code (panic) and passes on the new code.
- Issue #223 is closed by the PR.
- `progress.md` and `learnings.md` record the change.
