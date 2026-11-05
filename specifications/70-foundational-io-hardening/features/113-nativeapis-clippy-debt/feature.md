---
feature: "`foundation_nativeapis` clippy debt reaches zero"
status: "pending (CI proof)"
lane: "after 112"
model: "haiku"
branch: "spec70/f113-nativeapis-clippy-debt"
crates: ["foundation_nativeapis"]
catalogue_rows: []  # from docs/bug-backlog.md §35; not a §8 catalogue row
---

# Feature 113: `foundation_nativeapis` clippy debt

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05, PR #179 CI fix pass)

`cargo clippy --profile uat -p foundation_nativeapis --all-targets --no-deps -- -D warnings`
reports 191 warnings. The same command with `--features vfs-preload` reports 465. The
`vfs-preload` code adds most of the difference. The main kinds are
`clippy::non_std_lazy_statics` (the `once_cell::sync::Lazy` statics in
`src/native/vfs/shim/mod.rs`), `clippy::doc_markdown`, and raw-pointer lints
(`borrow_as_ptr`, `ptr_as_ptr`) in the `epoll` and VFS code.

This package does not set `clippy-deny-warnings = true` in `ci/packages.toml`, so the output
does not fail a run. The project wants zero warnings, so the debt is recorded in
`docs/bug-backlog.md` §35 and scheduled here.

## Design

1. Run the command above once with `--features vfs-preload`. Work file by file, and fix the
   cause of each warning. Do not add a blanket `#[allow]` to the crate root.
2. `non_std_lazy_statics`: replace `once_cell::sync::Lazy` with
   `std::sync::LazyLock` where the code already needs `std`. If a `once_cell` dependency
   stays for another reason, keep it and record why.
3. `doc_markdown`: add backticks to identifiers in doc comments.
4. Raw-pointer lints: use `&raw const` and `&raw mut`, or `ptr.cast::<T>()`, as clippy asks.
5. When the command above exits 0, add `clippy-deny-warnings = true` to the
   `[packages.foundation_nativeapis]` entry and add `vfs-preload` to `clippy-runs`, so the
   debt cannot return.
6. Delete the §35 clippy entry in `docs/bug-backlog.md` in the same change.

## Tests

- `cargo clippy --profile uat -p foundation_nativeapis --all-targets --no-deps -- -D warnings`
  exits 0.
- `cargo clippy --profile uat -p foundation_nativeapis --all-targets --features vfs-preload
  --no-deps -- -D warnings` exits 0.
- The package tests still pass, including the `vfs-preload` run that builds the shim and runs
  `tests/vfs_shim_vfs`.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_nativeapis --all-targets --no-deps -- -D warnings
cargo clippy --profile uat -p foundation_nativeapis --all-targets --features vfs-preload --no-deps -- -D warnings
cargo test --profile uat -p foundation_nativeapis --no-fail-fast
cargo test --profile uat -p foundation_nativeapis --no-fail-fast --features vfs-preload
```

## Result (2026-11-05)

Both clippy commands exit 0 and print no warning.

| Run | Before (unique diagnostics) | After |
|---|---|---|
| default features | 245 | 0 |
| `--features vfs-preload` | 384 | 0 |

The raw diagnostic counts, where the library target and its test target report
the same library warning twice, were 431 and 723.

Lint kinds fixed: `doc_markdown` (95 unique), `missing_errors_doc` (158),
`missing_panics_doc` (35), `missing_safety_doc` (27), `must_use_candidate`,
`return_self_not_must_use`, `uninlined_format_args`, `ptr_as_ptr`,
`borrow_as_ptr`, `cast_possible_wrap`, `cast_possible_truncation`,
`cast_sign_loss`, `cast_lossless`, `unnecessary_cast`, `io_other_error`,
`non_std_lazy_statics`, `single_match_else`, `match_same_arms`,
`map_unwrap_or`, `ignored_unit_patterns`, `implied_bounds_in_impls`,
`manual_let_else`, `items_after_statements`, `missing_fields_in_debug`,
`unnecessary_debug_formatting`, `struct_field_names`, `format_collect`,
`manual_c_str_literals`, `len_zero`, `empty_line_after_doc_comments`,
`used_underscore_binding`, `collapsible_if`, `new_without_default`,
`unnecessary_wraps`, `type_complexity`, `elidable_lifetime_names`,
`needless_pass_by_value`, `ref_option`, `struct_excessive_bools` and
`too_many_lines`.

`std::sync::LazyLock` replaces `once_cell::sync::Lazy` in the shim. The
`once_cell` dependency stays for the `ipc` feature, which still uses it in
`src/shared/ipc/version.rs`; `vfs-preload` no longer names it.

`ci/packages.toml` sets `clippy-runs = ["", "vfs-preload"]` and
`clippy-deny-warnings = true` for `[packages.foundation_nativeapis]`. The other
feature trees in `runs` are not linted yet and stay out of `clippy-runs`.

Three narrow `#[allow]` attributes remain, each with a one-line reason:

1. `clippy::ref_option` on the serde `with` serializer. Serde requires the
   callback to take `&Option<T>`; `Option<&T>` does not fit.
2. `clippy::struct_excessive_bools` on `VfsCapabilities`. The struct is a set of
   independent capability flags, so `bool` is the domain type.
3. `clippy::too_many_lines` on the `vfs_shim` example `main`. The example walks
   one end-to-end scenario in order.

The `vfs-preload` cast fixes keep the 64-bit behaviour: the shim is Linux-only
and built for 64-bit targets. One latent bug is recorded in backlog §35 (a
negative `ftruncate` length still becomes a huge allocation instead of
`EINVAL`); the lint pass does not change that behaviour.

Verification in the Linux CI image (`linux/amd64`, `--profile uat`):

- default tests: 15 test binaries, 48 passed, 0 failed, 2 ignored doc tests.
- `vfs-preload` tests: 24 test binaries, 224 passed, 0 failed, 5 ignored doc
  tests. The `vfs_shim_vfs` suite built the shim and ran 2 tests, both passed.
- feature 111 dispatch tests: 4 passed, 0 failed, 30 filtered out.
- `fork_child_first_stop`, `the_traced_child` and `test_spawn` were skipped.
  The ptrace spawn tests fail under OrbStack amd64 emulation, where
  `PTRACE_GETREGS` returns `EIO`. GitHub CI on a native amd64 kernel is the
  proof for them.

