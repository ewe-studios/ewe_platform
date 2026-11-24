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

Feature 1 passes the complete verification set. Both installed command names run the public runtime task.
The central executable suite passes three tests. Strict Clippy passes for the optional build and task features.
The packaged mobile release record now matches version 0.1.1 in the canonical record.

### Feature 3 resource staging and reads

The foundation_resources package reads package-relative declarations and generated inputs.
It stages a complete target-specific set and protects other destination files.
Concurrent staging, name conflicts, a failed replacement, and stale file removal pass the native tests.
The application context exposes resources.open(name). The browser reader checks the response before streaming its body.
The WebAssembly browser target compiles. Mobile packaging and application integration continue.

Feature 3 native resource streams can move between worker threads.
The macOS resource suite passes four staging and runtime tests with the UAT profile.

Todo now declares its shared icon, browser script, and generated version in resources.toml.
Its Cargo hook stages those files after generation. Web package verification is in progress.

The moved Todo development server serves an edited icon on the next request.
The embedding path also serves the icon from a moved executable without a public directory.
Gradle assembles the Android debug package with the shared icon and version file.
The package excludes the browser script. Xcode copies the same resources to the simulator bundle.
The generic simulator build cannot link the local Arm-only framework for x86-64.
The Arm simulator build and installed host reads remain open.

NativeCore accepts a package reader from the host before the first action.
Swift opens a file in the application bundle. Kotlin opens an Android asset stream.
Binding generation and integration checks are in progress.

The native reader test passes for normal completion, a later read failure, and early stream disposal.
The Swift and Kotlin binding generators both export the host reader and chunked read operation.

Five macOS staging and runtime tests pass. Empty managed directories now disappear after removal.
A nested user file survives a managed directory removal.
The separate phone tests compare the installed icon digest and check missing, invalid, and browser-only names.

Issue #730 fixes the embedded browser test host. It now supplies the staged directory explicitly.
Chrome completes application startup. The separate action error expectation remains under investigation.

Issue #731 fixes two callback names that conflicted with generated native code.
Clang accepts the generated C header. Gradle compiles the Kotlin binding and user interface modules.

Chrome, the iPhone 17 simulator, and the Android Arm emulator open art/todo.svg through the shared Rust resource service.
Each returns the same SHA-256 digest, bf701db9f85dc15a8c1f4fd4cac38bf1aca38a8e9a5cf3f6db6bbb8bc4809294.
The iPhone test runs once with no failures. Android instrumentation returns RESULT_OK.
Both native tests check missing host, invalid name, and the absence of browser-only app.js.
The Todo browser integration passes. A separate Chrome check covers a server error before reading its body.
The platform build task and build-module.sh now prepare the browser resource module.

Issue #733 fixes repeated Cargo invalidation after a package build.
An unchanged build becomes fresh. A missing managed file is restored.
Apple and Linux exchange whole resource directories atomically. A concurrent reader test passes on macOS.
Eight resource tests pass. Native and browser resource code pass scoped strict Clippy checks.
The CI package configuration selects staging tests on all three desktop operating systems.
Issue #734 records existing strict lint failures in the browser time backend dependency.

A production release build runs alone outside the checkout. It serves the icon, generated version, and browser script.
A generated Cargo fixture verifies a custom target directory and profile, renamed and deleted inputs, and recovery after failed staging.
A Chrome test now covers failed HTTP status, a later body read failure, and early cancellation.
The Windows resource test target cross-check passes. This run checked compilation only.

The web and static HTML starters now stage their public index before serving or embedding it.
Both mobile starters declare shared, generated, and browser-only resources.
The Android build registers a producer per variant. The Apple project registers one folder in its resource phase.
Both packaging systems select the Cargo profile from their build configuration.
The generated mobile layout test passes for server and on-device apps.
The complete generated application build checks are in progress.

Generated Android and Apple server starters both build against the local SDK candidate.
Their packages contain the shared title and generated version 0.1.0. They exclude browser.js.
Both web starter variants pass their generated Cargo project checks.
The Todo Android debug and release packages and unsigned Apple device package also pass inspection.
Linux passes all nine resource tests. This run supplied Linux execution evidence.
Issue #735 records a diagnostic from a legacy Cargo template during Git dependency discovery.
Issue #736 removes the staging index from the Apple resource payload.
Feature 2 remains proposed. Automatic installation into arbitrary existing Cargo packages awaits that separate approval.

The rebuilt Apple starter contains exactly `app/title.txt` and `app/version.txt` in its resource payload.
The native and browser resource targets pass scoped strict Clippy checks after the staging path cleanup.
All nine resource tests pass on macOS and Linux with both `build` and `server` enabled.
The additional Windows cross-check with `server` stopped because the host lacks `x86_64-w64-mingw32-gcc`.

The packaging contract assigns Todo browser module production, validation, and staging to the build. Runtime migration remains pending.

### 2026-11-24. Windows resource verification

[Specification 91](../91-windows-resource-staging/requirements.md) now owns the native Windows resource test run.
The owner deferred this run until the project has a Windows virtual machine.

## 2026-11-24. JavaScript package release

The owner approves Features 1 through 3. Feature 4 defines module imports, two classic scripts, and npm publication for each browser runtime release.

## 2026-11-24 master integration

The integration combines master at `16cc3e065` with the combined checkpoint at `32379fa98`.
The native tasks use ZIP 9.4.1 for archive creation and signature verification.
All 20 native integration tests and one documentation test pass on macOS. One documentation test remains ignored.
All 151 continuous integration runner tests pass. Cargo resolves all 29 tracked lockfiles.
The WebAssembly test packages for HTTP, ConnectRPC, and the Cloudflare network client pass the compiler checks.

The next merge includes the Todo checkpoint at `78f67835e`.
The database version is 0.0.11. The WebAssembly user interface version is 0.1.12.
All 29 lockfiles resolve. The JavaScript runtime passes 159 tests. The loader passes 78 tests.
The WebAssembly user interface compiles with browser storage enabled.

## Feature 2. Cargo build hook installation

The installer edits the package manifest and the target build script.
Three integration tests pass on macOS.
A temporary Cargo consumer rebuilds generated output after an input change.
The other tests check renamed dependencies, custom scripts, hook order, and rollback after a write failure.
The full task suite and the browser provider checks are in progress.

All 21 task tests pass with the installer enabled.
Strict Clippy passes for all task targets.
The platform command recognizes the existing platform demo hook.
It installs the browser runtime hook in a temporary Cargo package with a renamed dependency.
The package compiles and includes both generated scripts.
The browser package passes 115 tests. A separate provider test verifies transitive input tracking.

The application bundler passes its first lazy import test with readable and compact scripts.
The test checks that two requests receive the same module after one evaluation.
The packaged resource handler and its generated resource index are in progress.

The browser packaging hook now builds the application, Connect transport, and resource module from Cargo.
Todo uses this hook. Its development public directory contains one JavaScript file and three WebAssembly files.
A moved development executable serves all three binaries and the root index.
An edit to the moved icon appears in the next response.

An external consumer mounts the generated resource set with the same statement in both profiles.
The embedded profile test uses debug build dependencies.
A native HTTP test checks binary bytes, the media type, the root index, live edits, and a missing file.

The release Todo executable serves all seven assets after it moves to an empty directory.
Their bytes match the packaged assets. The compact script contains 332,866 bytes.
The external application builds its own browser module in both profiles.
The browser tests pass for Todo host actions, resource access, and offline storage.
The resource crate passes strict Clippy with the browser package and server features.

The Rust continuous integration planner matches the previous package list and full matrix output.
It also matches the previous affected-package plan across the master merge.
The Rust runner verifies the manifest schema, six fixtures, and the consumer copies.
The main workflow, virtual machine workflows, Make, and local Linux runner now call the Rust package.
Native toolchain and deployment callers still need migration.

The Linux container task now starts and cleans up its own container.
The mise command calls the registered Rust task. The previous shell launcher is removed.
A temporary application ran in a real Linux container for one package and all packages.
A failed guest command returned a failure and the task removed the container.
All nine planner tests pass. The full native HTTP suite passes on macOS.

The registered lockfile task verifies all 28 nested lockfiles on macOS.
Its test uses a directory with spaces and detects a stale package version.
The Rust toolchain check now owns version, platform, and installed Android package checks.
The previous toolchain check script is removed.
Its existing fixture passes all four scopes against the registered command.

All 21 task tests pass in the Linux amd64 container with the installer enabled.
Chromium opens the Todo add dialog from a moved release executable.
The browser requests only the bundled script.
The registered wire toolchain proof produces Rust, TypeScript, Swift, and Kotlin from a test schema.
The wire generator and its comparison with the original output are in progress.

The registered wire generator builds Rust, TypeScript, Swift, Kotlin, and the browser script.
The Rust and previous shell checks both match the checked-in files.
The workflows and mise now call the independent Rust command.
The obsolete wire launchers and Python source adapters are removed.

An unchanged Todo Cargo build now finishes in 1.41 seconds on macOS.
Cargo reports the server as fresh and does not rerun the browser producers.
The external resource installer test checks the generated hook with a renamed dependency.
It compiles the package and reads a staged file.
The workspace archive checker has a Rust implementation. Its regression tests are in progress.

The workspace archive task passes all 13 continuous integration tests.
It rejects indirect release cycles and builds an isolated crate from its archive with workspace settings removed.
The release workflow now runs this Rust command.
Both Linux workflow paths check the native tools with Rust.
The Android package installer passes its fixture for license failure, missing tools, and repeat installation.
The native suite passes 21 integration tests, including immutable tag and package checks.
The previous archive, signing, staging, publication, and native shell launchers are removed.
The action and device test programs stay in place for the Rust tasks.

The Android toolchain task built and verified the signed sample package with Android Gradle Plugin 9.5.1 and platform 36.
The Apple task generated and compiled Swift with Xcode 27.2 and the iOS simulator 27.2 development kit.
The proof fixture checks full and Apple-only runs, failure cleanup, and repeat runs.
The old Android build and Apple proof launchers are removed.
The release retry task is under local fixture verification.

All seven release retry scenarios pass against a fake publisher and tag counter.
The release workflow starts the Rust task. The task streams command output and keeps only enough bytes to recognize a retry hint.
A local Docker Compose test verifies the virtual machine host task, console connection, shared mount error, and diagnostics.
It uses a Linux fixture container. Execution inside actual guest operating systems remains open.

The image task stages seven Linux and virtual machine configurations with matching content hashes.
The registry fixture verifies retry, missing images, access denial, error status, and tag promotion.
The continuous integration package passes all 16 integration tests and strict package Clippy.
The image workflows now use the Rust tasks. The obsolete image planner and submodule linker are removed.
The migration inventory lists the remaining release, host, and guest operations.

The release task uses the executable paths that Cargo reports. The independent package test verifies repeated archive bytes and the checksum.
The attachment task checks every archive before contacting the release service.
A local substitute verifies access errors, draft creation, and recovery after an interrupted upload.
The Android lock task restores the real Linux tool archive checksum from Google's manifest.
The lock matches the original bytes. A repeat run makes no change.
The four source guards pass all 32 existing fixtures through the Rust command.
Linux amd64 passes 22 task tests and 17 continuous integration tests from the image and binary archive checkpoint.

The previous Python planner now serves as a test reference. The Rust command matches its complete matrix for 126 packages.
All 139 Python regression tests pass. The native suite passes 22 tests, including a reproduced Swift release revision.
The installed Rust command runs outside the checkout. All 28 tracked nested lockfiles pass the offline check.

The image task exports a real Docker archive and an Open Container Initiative (OCI) layout.
A disposable loopback registry verifies image creation, digest reuse, and tag promotion.
The source and promoted tags resolve to the same digest. All 74 checks pass in the existing Linux image.

The release workflow records tags before publication. It sends a binary build request for each new eligible tag.
A local Git remote and dispatcher verify tag selection and recovery after a partial failure.
The release tool task checks the cached version and selects the compiler for each release mode.
The central platform executable passes the locked offline Cargo check.

Make and mise now use the same registered tasks for workspace builds, checks, formatting, audits, and documentation.
The single browser target and four-target mise build retain their original Cargo arguments.
The manual publication task checks all package names before it calls Cargo to publish.
The existing selection fails locally on `foundations_ext`. Issue #793 tracks the replacement selection.
All 24 continuous integration package tests pass on macOS at this checkpoint.

Linux amd64 passes all 24 continuous integration tests, all 17 default task tests, and all five build-hook installer tests.
The source and caller regression suite passes 140 tests. Strict Clippy passes for the task macros, task runner, and continuous integration package.
The Windows GNU target compiles the task and continuous integration packages. Native Windows execution remains unverified.

The Google Cloud batch scripts now validate inputs before writing. Both stop with an error and preserve existing output on real inputs.
The canonical generator rejects the checked-in Google Discovery documents. Issue #794 tracks normalization before migration can finish.

## Application generation command

Feature 5 defines the proposed ewe_ui command, interactive target selection, task ownership, and separate human and agent help. Owner review precedes implementation.

Feature 5 assigns reusable operations to libraries and custom workflows to optional project-local tasks. The generic native tasks still need migration.

Feature 5 now specifies package-owned embedded scripts, platform selection, and installed-binary checks.

Feature 5 specifies Rust task implementations by default. Embedding applies only to scripts with a documented execution requirement.

The owner approves Feature 5 for implementation. The Specification 90 agent resumes work on ewe_ui.
## Feature 4 publication credentials

The owner approves Feature 4. The plan specifies token lookup and secret handling for package publication.

## Feature 4 package and release checks

The runtime package exports three JavaScript functions and two classic scripts.
Independent pack runs produce the same 19-file archive. The compact script is smaller.
A clean npm installation and Chromium verify the module consumer and both classic files.
Each variant makes one script request, dispatches callback 7, and renders the updated text.
A loopback registry verifies the exact uploaded archive, both credential sources, missing credentials, and matching and conflicting releases.
The crate release workflow dispatches the browser workflow for new runtime tags. The workflow also accepts an explicit retry at the tag.
Live npm publication and installation of the first public version require an authorized release.

## Feature 5 application generation

The shared generator renders all seven selections of web, Android, and iOS targets.
Generator tests check shared resources, invalid input, cancellation, and competing writes.
A terminal test compares interactive output with a command that uses explicit arguments.
Chromium loads the generated web application and increments its Rust counter.
This test uses a local registry with the verified runtime archive and local Rust package sources.
The installed executable passes the wizard test outside the checkout. It lists Android virtual devices and iOS simulators.
An offline build produces the same web files from the local cache.
Native artifact checks, local task registration, and example migration remain in progress.

## Project-local task commands

Two independent projects register Rust task packages with the installed command.
The guide lists each task's arguments. The runner preserves values with spaces and returns child exit status 19.
All five generator checks and all 17 task runner checks pass.
The generated web build also passes through the installed command. It produces identical files with its registry offline.

## Native package task migration

The Todo package owns bridge preparation, Android binding synchronization, device checks, and starter screen checks.
The established repository commands delegate to this package.
A Rust client sends two distinct Todo requests and receives confirmation frames from the real server.
The Android provider tests verify both architectures and the example and framework library destinations.
The shared launch operation serves the generated command and an application-specific simulator test.
The public iOS version and native Rust tag remain unavailable. Local candidate builds are in progress.

## Installed command guide

The guide includes build, device, and task examples for a generated project.
A test follows the installed command tree. All 41 command paths and their options appear in the guide.
Two independent applications list their custom task only after its declaration. Both return the expected child status.

The composed mobile core uses the application crate for resources. Standalone mobile projects retain their own resource provider.
All six generator checks and the legacy mobile template suite pass after the change for issue #797.

## Package-owned interpreter scripts

The task crate can select a host or device script from an embedded directory.
Installed development and release binaries run with the source directory hidden.
An incremental development rebuild picks up new files, changed files, and deleted files.
The external fixture checks portable fallback, unsupported targets, companion lookup, failure status 23, and cancellation status 130.

The detached process tests pass with the full task runner suite. All 23 tests pass.

Native task checks pass for 22 release and device fixtures and seven application checks. Strict lint checks pass across all five affected packages.

The installed command builds a signed Android App Bundle and verifies its signature on Java 27. Both native architectures and the application title match.

The installed command builds and launches CedarBoard on Android and iOS with local release inputs.
Both installed applications contain the same shared title. The Android screen shows the server count and refresh action.

The installed native command parks while a child runs. Its processor time fell from 4.744 seconds to 0.073 seconds for the same wait.
The browser still enables its JavaScript event loop.

CedarBoard passes the three-target check from one generated project outside the checkout.

| Target | Installed command result |
|---|---|
| Web | The online and offline builds produce identical JavaScript, WebAssembly, and title files. |
| Android | The emulator displays the server count and refresh action. The installed application owns the focused window. |
| iOS | The simulator launches the application. Its installed bundle contains the shared title. |

The checks use local release candidates. The public native release tag, Maven package, browser package, and shared resource crate are unavailable.

Seven Cloudflare operations now run through typed Rust tasks. All 26 continuous integration tests pass. The platform command compiles with the same tasks.
The local database migration uses Wrangler's supported apply command. Issue #805 tracks the remaining object listing.

The installed command builds and launches the Android on-device starter. A tap changes its local Rust counter from zero to one.
The application archive contains one application library and one framework library for each selected architecture.

The installed Android launch command waits for the requested activity to own the focused window.
CedarBoard started on the emulator and displayed its server data. Grove displayed its counter and accepted a tap.
An installed command test supplied two empty focus reports before the requested activity appeared.

The Cloudflare task provider lists bucket objects with the generated storage client.
It writes one object per line and follows each response cursor. Browser login passes Wrangler options through.

The Android and iPhone Todo Server examples displayed the same list from the local server.
Adding “Review the mobile layouts” on iPhone updated the running Android list. Both applications displayed two open items.
The optimized installed generator also created a three-target project outside the checkout. Its guide covered all 41 commands.

Xcode builds the generated on-device Grove application with the local Apple release inputs.
The application archive supplies the Rust library for each target. The application imports the module map from its own header directory.
Issue #811 retains the simulator launch check. The owner uses the iPhone simulator for the Todo Server example.

The Rust host setup tasks select Debian, Ubuntu, and Arch packages from an explicit choice or the operating system release file.
Seven mise commands use these tasks for testbed installation, Quickemu installation, and macOS image download.
Tests use local package manager commands and check failure propagation. They leave the host package database unchanged.

The Cloudflare service controller owns one local worker per project. Stop requests reach that worker through its instance directory.
The package tests cover independent projects, restart, repeated stop, failed suites, and child cleanup.
Wrangler 4.156.2 returned HTTP 200. The local suites passed six object store tests, five database tests, and eighteen file system tests.
Both suites released the local port after completion.

The native Network File System (NFS) task selects both Cargo features required by its test target.
It writes the combined process output to the requested log and returns Cargo's failure code.
All 13 real NFS tests passed. A command fixture verified empty runs and a failed test with diagnostic output.

The native Apple release task built the complete 254 MiB XCFramework archive on macOS.
Its libraries cover arm64 iOS, arm64 and x86_64 iOS simulators, and arm64 and x86_64 macOS.
All archive members passed integrity checks. Swift Package Manager reported the same digest as an independent file check.
The local archive has SHA-256 digest `3755f61952e2cc1e87cc8d7d001e50999997ffc459797552559c3e559d30ea49`.

Cargo now includes all 68 generator templates in a clean source package.
The package includes both Android ignore files and the Cargo configuration files for the web and API templates.
A CLI installation built from the listed package inputs generated an independent Android project with its ignore file.

The virtual machine host changes are in progress. Cargo checked the Rust start and stop tasks.
Nushell passed the storage path, image, ports, and key path to the independent runner.
A failing runner returned exit code 42 through the existing Nushell command.
The Rust integration test and strict Clippy checks passed.
Docker started, inspected, stopped, and removed two isolated containers with loopback ports.
This host check used test device mappings. Guest boot verification still needs a Linux host with hardware acceleration.

The linked checkout package inventory limitation is tracked in issue #818.
The clean source export passed the registry input check and the independent generator installation.

The guest runner status and wait commands now use typed tasks.
Both Nushell commands returned status 255 when the local Secure Shell (SSH) port refused the connection.
The integration test checked both guest probes, a busy timeout, a completed runner, and four failure statuses. Strict Clippy passed.
