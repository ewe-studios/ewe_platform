---
feature: "cargo fmt --check diverges between local and CI (rustfmt version drift)"
status: "done"
note: "E1 landed; rustfmt pinned to nightly-2026-09-20 (rustfmt 1.10.0-nightly feaadeeaca, matches the CI image); repo-wide cargo fmt --all reformat committed; §16 entry deleted"
priority: "medium"
depends_on: []
branch: "fix/io-39-rustfmt-version-pin"
crates: ["foundation_auth", "foundation_netio", "foundation_connectrpc"]
backlog: "docs/bug-backlog.md §16 (1 entry)"
---

# Feature 39: pin one rustfmt version and format the repo

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); continuous integration (CI); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## T0. Confirm the backlog entry is still open

- **Where:** `docs/bug-backlog.md` §16 "cargo fmt --check diverges between local and CI".
- **Confirm:** run the fmt check in the CI image and on the host, and count the files each one
  flags:

  ```bash
  cargo fmt --all -- --check 2>&1 | rg '^Diff in' | wc -l
  scripts/ci/linux.sh all >/dev/null 2>&1 || true
  ```

  The host count and the CI count differ. The CI image reformats whole crates differently.
  The backlog names 64 files in `foundation_auth` and 91 in `foundation_netio`; the
  `foundation_connectrpc` CI run (2026-11-01) added 64 more files there.

## E1. Pin one rustfmt version

- **Fix:** add a `rust-toolchain.toml` at the repo root that pins a toolchain with the
  `rustfmt` component. Pick the version the CI image uses (read it with
  `docker run --rm ewestudios/ewe-platform-ci:latest rustfmt --version`), so local and CI
  format the same way. Then run `cargo fmt --all` once and commit the result.
- **Trade-off to record in the PR body:** a pinned toolchain is a hard dependency for every
  contributor, but it is the only way to stop the drift. Say which version and why that one.
- **Tests:** none. This changes formatting only. The verification below proves the repo is
  fmt-clean under the pinned toolchain.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
rustup show active-toolchain          # the pinned toolchain, with rustfmt
cargo fmt --all -- --check            # must print nothing
python3 scripts/ci/ci.py plan         # CI still plans all packages
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] `cargo fmt --all -- --check` prints nothing on the host and in the CI image.
- [ ] The `rust-toolchain.toml` is committed and names the CI image's rustfmt version.
- [ ] The backlog entry is deleted.
- [ ] CI green.

## Review checklist (main session)

- [ ] The pinned version equals the CI image's version (stated in the PR body).
- [ ] The reformat commit is separate from any code change (no logic edits mixed in).
- [ ] No `#[rustfmt::skip]` was added to hide a case; each skip, if any, has a reason.
