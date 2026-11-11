# Releasing crates

Crates are versioned and published by [release-plz](https://release-plz.dev),
configured in `release-plz.toml` and run by `.github/workflows/release-plz.yaml`.

## How a release happens

1. Merge PRs to `master` as usual. Don't bump crate versions by hand.
2. When the **Audit: Checks & Tests** workflow (`check.yaml`) passes on `master`, the **Publish crates to
   crates.io** workflow runs. Find it under **Actions**; it does not show
   among a PR's checks, because it starts after Audit: Checks & Tests finishes on master.
   It has two jobs:
   - **Publish to crates.io** publishes every crate whose version is not on
     crates.io yet (normally none, unless a release PR was just merged), and
     tags each one `<crate>-v<version>`.
   - **Open or update the release PR** opens or updates one
     **"chore: release"** PR. That PR bumps the version of each crate whose
     code changed since its last crates.io release, updates the version
     requirements of the crates that depend on it, and adds `CHANGELOG.md`
     entries generated from the commit messages.
3. Review the release PR (edit the changelog or versions on its branch if
   needed) and merge it. The next green **Audit: Checks & Tests** run on `master` publishes
   those versions, in dependency order.

A publish that fails partway picks up where it stopped on the next run:
release-plz skips versions that are already on crates.io.

Tagging `ewe_platform-v*` (release-plz does this when it publishes
`ewe_platform`) also triggers `release.yaml`, which builds the platform
binaries and attaches them to that GitHub release.

## Repository settings

- `CARGO_REGISTRY_TOKEN`: crates.io API token with publish rights for every
  crate in the workspace. Required.
- The GitHub App behind `EWE_RUNNER_REGISTRA_CLIENT_ID` and
  `EWE_RUNNER_REGISTRA_PRIVATE_KEY` (the one that registers VM runners). The
  release job mints a short-lived token from it, so the app needs
  **Contents: Read and write** and **Pull requests: Read and write** on this
  repository, besides the Administration permission the runner jobs use.
  If minting fails, the job warns and falls back to `GITHUB_TOKEN`, and GitHub
  then runs no workflows for what that token creates: the release PR would
  get no CI, and the `ewe_platform-v*` tag would not start `release.yaml`.

## Notes

- Every bump goes through the release PR, so ordinary PRs never publish
  anything.
- `semver_check` is off: most crates are 0.0.x, where every bump is already
  breaking.
- The **Publish dry-run** job in `check.yaml` runs
  `scripts/ci/package_workspace.py` on every push to `master`. It packages
  each publishable crate against a local `[patch.crates-io]` table. Keep it
  green, because the release job publishes the same packages.
- A dev-dependency on a workspace crate must not have a `version` when that
  crate depends back on this one (a publish cycle). Example:
  `foundation_core` depends on `foundation_nostd`, so the `foundation_core`
  dev-dependency of `foundation_nostd` is path-only. Cargo removes a
  path-only dev-dependency from the published manifest. With a `version`,
  `cargo package` must find the dependent on crates.io first, and the
  publish fails. The patch table hides this, so `package_workspace.py`
  checks for these cycles before it packages.
- Running release-plz locally inside a `git worktree` is not supported: it
  copies the repository to a temp dir whose `.git` still points at the
  worktree, so its checkouts move the worktree's `HEAD`. Use a normal clone.
- Releases build on **stable**, pinned in `ci/release-rust-toolchain` (bump
  deliberately). The release jobs and the **Publish dry-run** job install that
  toolchain; `rust-toolchain.toml`'s pinned nightly is for development only.
  Keep the workspace loadable on stable: no nightly-only keys in `Cargo.toml` or
  `.cargo/config.toml`. Dev's Cranelift backend (`codegen-backend`, nightly
  only) is set through mise's `[env]` instead (`CARGO_PROFILE_DEV_CODEGEN_BACKEND`
  and `CARGO_UNSTABLE_CODEGEN_BACKEND`), so stable cargo never sees it.
- One exception: the **Open or update the release PR** job uses the pinned
  nightly from `rust-toolchain.toml`. release-plz runs `cargo package` at the
  commit of each crate's last release. Commits before `df86d7e6`
  (21 October 2026) have the nightly-only `codegen-backend` key in the root
  `Cargo.toml`, and stable cargo refuses to load them. That job only
  computes versions and changelogs and publishes nothing. The **Publish to
  crates.io** job stays on stable. When every crate has a release after
  `df86d7e6`, the release PR job can move back to stable.
