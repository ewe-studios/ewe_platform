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
2. **Package jobs.** Each selected package runs `rustfmt`, `cargo build`
   (all targets), `clippy` and `cargo test` as separate steps, so the Actions
   page shows whether a crate failed to build or failed its tests. They use
   the `uat` profile (dev with LLVM, which is what the test docs use).
   Every package job runs **inside the CI image** (`container:`), which
   already has the pinned toolchain, the system libraries (GTK/WebKit,
   Vulkan, Mesa, ...), the tools and the submodules, so jobs install
   nothing. The submodules are linked into the checkout from the copies
   baked into the image (`ci/platform-images/actions/link-submodules`).
   Before the package jobs, the `image` job makes sure the image for this
   commit's inputs exists, and builds and pushes it if not. See
   [`ewe-platform-image/README.md`](ewe-platform-image/README.md) and
   [`platform-images/README.md`](platform-images/README.md).
3. **Checks passed.** One summary job that is green when every selected package
   passed (or none needed testing). Make this the required status check in
   branch protection: it stays the same name however many packages run.

`master` pushes and the nightly run also do a macOS build and a
`cargo publish --dry-run`.

## Per-package settings

`ci/packages.toml` holds only what differs from the defaults: features a
crate's tests need, extra test runs, timeouts, and whether `fmt`, `build`,
`clippy` or `test` failures fail CI (`enforce`), only warn (`report`), or don't run
(`off`). A package can also be skipped with a reason. New crates need no entry.

## Running it locally

```sh
make ci-list                          # every package and its settings
make ci-plan                          # what your branch affects (BASE=origin/master)
make ci-package PACKAGE=foundation_ai # fmt + build + clippy + tests, as CI runs them
make ci-package PACKAGE=foundation_ai STEP=test
```

The toolchain is pinned in `rust-toolchain.toml`. Bump it deliberately, and
run the affected packages when you do.

To run exactly what CI runs, in the same environment, use the image:

```sh
docker run --rm -it -v "$PWD:/w" -w /w ewestudios/ewe-platform-ci:latest bash
nu ci/platform-images/scripts/link-submodules.nu   # inside the container
python3 scripts/ci/ci.py run foundation_ai
```

## Other workflows

- **Platform images (publish)** (`platform-images-publish.yaml`): builds and
  smoke-tests both image flavors (default and CUDA), tags `latest`, and
  rebuilds weekly for security updates.
- **Release binaries** (`release.yaml`): on `v*` tags (and by hand), builds
  release binaries inside the image and attaches them to the release.
