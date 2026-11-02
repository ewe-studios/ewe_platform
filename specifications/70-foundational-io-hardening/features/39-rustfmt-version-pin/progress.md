# Feature 39 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** continuous integration (CI); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Landed

- E1 (pin one rustfmt version and format the repo):
  - `rust-toolchain.toml` names the pinned channel `nightly-2026-09-20`. That
    channel gives rustfmt `1.10.0-nightly (feaadeeaca 2026-09-19)`. This is
    the same rustfmt the CI image uses. The file now also lists `clippy` in
    the `components`.
  - One repo-wide `cargo fmt --all` reformats 1377 tracked Rust files (7292
    diff hunks). The standalone `crates/channels` workspace is formatted too
    (`broadcast.rs`, `executor.rs`, `mspc.rs`).
  - `cargo fmt --all -- --check` prints nothing on the host. The standalone
    `crates/channels` and `crates/domain` checks print nothing too.
  - The §16 "`cargo fmt --check` diverges between local and CI" backlog entry
    is deleted.
