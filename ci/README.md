# CI

CI runs **one job per workspace package** instead of one job for the whole
workspace, so a failure points at the crate that broke and an unrelated crate
can't hold up your PR.

## How a run works

1. **Plan.** `scripts/ci/ci.py plan` reads `cargo metadata` and picks the
   packages to test:
   - **Pull requests:** the packages whose files changed, plus every
     workspace package that depends on them. Markdown-only changes select
     nothing. A change to a workspace-wide file (`Cargo.toml`, `Cargo.lock`,
     `rust-toolchain.toml`, `.cargo/`, `ci/`, `scripts/ci/`, the workflow
     itself) selects everything.
   - **Push to `master`, the nightly schedule:** every package.
   - **Manual run** (Actions → Checks → Run workflow): `all`, or the packages
     you name.
2. **Package jobs.** Each selected package runs `rustfmt`, `clippy` and
   `cargo test` with the `uat` profile (dev with LLVM; the `dev` profile's
   Cranelift backend isn't available in CI). Packages that depend on llama.cpp
   fetch the `tools/llama.cpp` submodule; nothing else does.
3. **Checks passed.** One summary job that is green when every selected package
   passed (or none needed testing). Make this the required status check in
   branch protection: it stays the same name however many packages run.

`master` pushes and the nightly run also do a macOS build and a
`cargo publish --dry-run`.

## Per-package settings

`ci/packages.toml` holds only what differs from the defaults: features a
crate's tests need, extra test runs, timeouts, and whether `fmt`, `clippy` or
`test` failures fail CI (`enforce`), only warn (`report`), or don't run
(`off`). A package can also be skipped with a reason. New crates need no entry.

## Running it locally

```sh
make ci-list                          # every package and its settings
make ci-plan                          # what your branch affects (BASE=origin/master)
make ci-package PACKAGE=foundation_ai # fmt + clippy + tests, as CI runs them
make ci-package PACKAGE=foundation_ai STEP=test
```

The toolchain is pinned in `rust-toolchain.toml`. Bump it deliberately, and
run the affected packages when you do.
