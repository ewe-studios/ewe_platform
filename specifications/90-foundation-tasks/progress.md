# Specification 90 progress

## 2026-11-22

The owner approved the central runner, package task sets, command compatibility, and script migration plan.
The specification records all eight enabled platform command groups and their proposed task names.
The design includes release verification and repeatable publication tests.

The implementation checklist starts with the shared registry and the existing platform command tree.
The plan and acceptance criteria are in [requirements](requirements.md).

The runner, task context, command adapters, process backend, and entry attributes are implemented in this worktree.
The central executable registers all existing command leaves, including `gen_api normalize`.
The `platform` and `ewe_platform` targets share one entry point.
The native library exports its developer task set under the optional `tasks` feature.
The Android build and release preparation use Rust orchestration.

Rust also assembles Apple archives, generates and synchronizes bindings, and stages the Apple Todo package.
Release verification checks candidate digests before it starts a tool.
Publication rejects conflicting digests and reconciles incomplete uploads before another submission.
The example application tests keep their existing device automation scripts.

The Foundation tasks integration suite passed on macOS.
The suite verifies directory changes, command aliases, output limits, exit status, and child-tree cancellation.
The native host tests passed against a local candidate and an injected process backend.
The standalone native executable listed its tasks and showed argument help.
Four publication tests passed with a fake registry.
The Apple archive fixture passed. It verifies three platform slices and rejects an incomplete archive.
A local signed bundle passed OpenPGP verification with a temporary key.
The runner passed a Windows cross-check. Windows execution still needs a Windows host.
No artifact was published.

## Owner checkpoint

The owner requested Valtron as the only asynchronous runtime.
The task runner manifest and lock entry have no direct Tokio dependency.
Production source switches to Valtron background startup and the native signal adapter.
The process tests still reference Tokio. The replacement has no completed compile or test run.
Process completion and cancellation cleanup still need review.
Linux execution and Windows execution of the replacement remain unverified.

Before the runtime change, both platform binaries passed listing, help, and project generation checks.
Both binary names also passed after archive extraction.
The full platform and macro test command stopped during the template build test.
The Cargo installation check remains pending.

### Dependency versions after integration with master

The combined branch aligns 49 local dependency requirements in 27 manifests.
The Cloudflare, DigitalOcean, and Hetzner deployment crates require `foundation_codegentools` version 0.1.7.
The interface web handler requires `foundation_wasm_ui` version 0.1.8.
Cargo resolves all 119 workspace packages with the root lockfile.
Issue #706 tracks workspace inheritance for packages outside the root workspace.
Eighteen standalone lockfiles now include the updated local crates. Cargo resolves each with its lockfile and without network access.
Issue #707 tracks the wire text update example. Its old lockfile also fails before this merge.
