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

## Valtron process verification

Branch `resume/90-tasks` starts at `622c8b71b` in an external worktree.
The first macOS build stops at the selection macro import.
The repair uses the public macro and reads the child exit status once.
The process tests now use Valtron. They check private input, bounded output, child status, and cancellation.
Issue [699](https://github.com/ewe-studios/ewe_platform/issues/699) tracks verification.

The macOS native task suite passes 11 integration tests and one documentation test. One documentation example stays ignored.
The native task executable receives a termination signal while it waits for a test tool.
It returns status 130 and reaps the tool process.
The Windows cross-check compiles the runner and its tests. Windows execution still needs a Windows host.

Review adds two tests for dropped futures. Both tests expose a surviving child before the repair.
The supervisor now waits for the owner before it reaps the Unix group leader.
The final macOS run passes all 14 integration tests. The final Clippy run reports no crate warnings.
The Windows cross-check passes with the final manifest. Issues 550 and 596 track three workspace manifest warnings.

The runner uses the workspace display and error derives. Its error variants and exit codes stay compatible.
All 15 macOS integration tests pass after this change. Clippy reports no crate warnings.
The Windows test cross-check passes. Both macOS platform command tests pass.

Linux passes all 15 task runner integration tests with the final manifest.
Cargo installs both macOS executables from the local package.
Each installed executable lists the same 25 tasks. Each generates the same four files for a sample iPhone project.
The installed commands also show help for project generation and the native Android build.

The platform suite passes four integration tests, including three template builds.
The macro run passes 24 unit tests and 54 of 55 integration tests on this branch.
Issue [712](https://github.com/ewe-studios/ewe_platform/issues/712) tracks the external target directory failure.
The combined checkout passes the repaired migration-gap test with a fresh external target.
Both combined executable names list 25 tasks and generate identical iPhone projects.

### Dependency versions after integration with master

The combined branch aligns 49 local dependency requirements in 27 manifests.
The Cloudflare, DigitalOcean, and Hetzner deployment crates require `foundation_codegentools` version 0.1.7.
The interface web handler requires `foundation_wasm_ui` version 0.1.8.
Cargo resolves all 119 workspace packages with the root lockfile.
Issue #706 tracks workspace inheritance for packages outside the root workspace.
Eighteen standalone lockfiles now include the updated local crates. Cargo resolves each with its lockfile and without network access.
Issue #707 tracks the wire text update example. Its old lockfile also fails before this merge.

## Build and deployment task ownership

The owner requests task ownership of all build and deployment operations. The revised scope and acceptance checks are in migration.md. The owner approves implementation and the foundation_ci package.

The proposed library exposes reusable synchronous build and code generation functions to Cargo build scripts and existing tasks. The dependency probe passes.

The owner approves Feature 1. It combines the shared build helper, the Cargo build script, and the existing task runner entry.

The browser package owns and exports the runtime bundle task set. The platform executable imports and registers that set.

Feature 2 proposes a reusable build hook installer for new and existing Cargo packages. It awaits owner approval.

### Feature 1 browser runtime

The `javascript` feature of `foundation_codegen` builds a readable script and a minified script from one entry.
The Todo server build records each imported file as a Cargo input. The browser UI package exports the runtime task.
The two scripts pass the Chrome parity fixture after its cursor reset uses the current view. Issue #721 records the former fixture failure.
Both scripts deliver the same 312-byte frame through WebAssembly and server-sent events.
The host dependency graph excludes task and asynchronous runtimes. The browser artifact graph excludes the JavaScript build tools.
Two checkout paths produce identical bytes. Check mode rejects a changed readable or compact script without writing either file.
The current readable script has 590,242 bytes. The compact script has 275,965 bytes.
A transitive vocabulary edit makes Cargo rebuild both scripts. Both rebuilt files contain the new code.
The code generator passes 74 tests. All 11 build tool tests pass after the path assertion fix in issue #724.

### Native continuous integration planner

The Rust runner passes eight fixture tests on macOS. The tests cover group directories, invalid references, package selection, and shard failure summaries.
The real package listing and full plan match the Python runner. The selected example and WebAssembly groups also match across all three platform plans.
Historical comparisons match every affected package and reason. The example change selects 105 packages. The WebAssembly change selects 104 packages.

### 2026-11-22. Native release and device tasks

The native task crate now has Rust operations for archive, stage, sign, publish, and device checks in the combined worktree.
The release workflow calls the native task command. Its workflow syntax check passes.
The macOS task-enabled integration run passes 17 tests, including five offline publication and receipt tests.
The shared host timer passes through real task dispatch.
The complete device run and clean release build are pending. The publication tests used fake registries.

The three Rust source guards pass. The clock guards report 13 direct reads and 27 clock parameter uses.
The execution tests cover package deadlines and browser cache selection.
Strict Clippy passes for every runner target. Cargo installs the runner offline from its lockfile. The installed executable lists nine tasks.

Feature 3 specifies automatic resource staging and platform packaging through one Cargo build hook.
The owner approves the design and shared runtime interface. The tasks agent starts after its current assignment. Specification 69 Feature 27 supplies the first consumer.

### Work queue

Complete Feature 1 and the continuous integration runner checks first.
Then implement approved Feature 3 from `features/03-platform-resource-manager/feature.md`.
Feature 3 covers automatic resource staging, streaming reads, and typed errors on browser, Apple, and Android hosts.
Coordinate the Todo integration with its implementation owner.
