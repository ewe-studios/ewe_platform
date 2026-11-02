# Feature 39 learnings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** continuous integration (CI); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## E1. Pin one rustfmt version and format the repo

- The nightly channel name and the rustfmt commit date differ by one day. The
  CI image's rustfmt is `1.10.0-nightly (feaadeeaca 2026-09-19)`. The channel
  that gives that rustfmt is `nightly-2026-09-20`. The date in the version
  string is the commit date, not the channel name. Pin the channel, not the
  commit date.
- `cargo fmt --all` at the workspace root does not touch `crates/*` (they are
  excluded). Each standalone crate there needs its own `cargo fmt`. Only
  `crates/channels` had drift (three files).
- rustfmt needs two passes on a few long `format!` calls inside `match` arms.
  The first `cargo fmt --all` left one file flagged. A second `cargo fmt --all`
  settled it. Run `cargo fmt --all` and then `cargo fmt --all -- --check`
  until the check prints nothing.
- The first `cargo fmt --all -- --check` counted 7292 `Diff in` lines. That is
  diff hunks, not files. It flags 1372 files. Count distinct `Diff in` paths
  when you want a file count.
- The feature 08 learning ("format only the files you changed, the repo is not
  fmt-clean") is now resolved. The repo is fmt-clean under the pinned
  toolchain.
