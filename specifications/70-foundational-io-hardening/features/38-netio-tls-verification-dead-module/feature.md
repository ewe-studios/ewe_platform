---
feature: "Delete the dead tls_verification module"
status: "pending"
priority: "low"
depends_on: []
branch: "fix/io-38-netio-tls-verification-dead-module"
crates: ["foundation_netio"]
backlog: "docs/bug-backlog.md §2 (1 entry)"
---

# Feature 38: Delete the dead `tls_verification` module

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Transport Layer Security (TLS); pull request (PR);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## T0. Confirm the backlog entry is still open

- **Where:** `foundation_netio/src/native/mod.rs` lines 14–15 still hold the commented-out
  `// pub mod tls_verification;` (with the note "Dead code: tls_verification was never wired
  in."). The file `foundation_netio/src/native/tls_verification.rs` still exists and is never
  declared, so none of its types are compiled.
- **Confirm:**

  ```bash
  rg -n "tls_verification" backends/foundation_netio/src
  ```

  The only hits are the commented line in `native/mod.rs` and the file's own contents. No
  code declares or references the module. The entry is still open.

## E1. Delete the dead module

- **Where:** `foundation_netio/src/native/tls_verification.rs` (the whole file) and
  `foundation_netio/src/native/mod.rs` lines 14–15 (the comment and the commented-out
  declaration).
- **Fix (decision rule):** a module is kept only when a caller needs it. Nothing references
  this module. Its inline tests were already removed (feature 11), and its imports are stale
  (`crate::netcap::{Endpoint, EndpointConfig, SocketAddr}` are no longer re-exported from
  `netcap`). So delete the file and the two commented lines. Do not wire it in.
- **Tests:** none. This is a deletion; the module was never compiled, so the crate's test
  suite is unchanged. The `shared/errors.rs` `TlsVerificationError` (line 127) is a separate,
  exported public type (re-exported through `netcap::errors`), not the dead module's type;
  leave it alone.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_netio --profile dev
! rg -n "pub mod tls_verification" backends/foundation_netio/src
! test -e backends/foundation_netio/src/native/tls_verification.rs
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] `foundation_netio/src/native/tls_verification.rs` is deleted.
- [ ] The commented-out declaration and its note are removed from `native/mod.rs`.
- [ ] The backlog entry is deleted.
- [ ] CI green.

## Review checklist (main session)

- [ ] No `tls_verification` reference remains in `foundation_netio` (the
      `shared/errors.rs` `TlsVerificationError` is unrelated and stays).
- [ ] No other module became unreachable by the deletion.
- [ ] The PR body states the decision rule (delete because nothing references it).
