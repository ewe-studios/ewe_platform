# Specification 90 learnings

## 2026-11-22

Cargo builds the current command as `ewe_platform`. Its help text uses `platform`.
The release workflow packages each executable target that Cargo reports.
The Makefile also names the current executable directly.
These callers require the compatibility entry. Both executable names can share one source.
[Manifest](../../bin/platform/Cargo.toml) · [Release workflow](../../.github/workflows/release.yaml) · [Makefile](../../Makefile)

The WebAssembly build and bundle commands already delegate to package libraries.
Each library can expose its existing operations through a Rust task set.
[Build adapter](../../bin/platform/src/wasm_bins/mod.rs) · [Bundle adapter](../../bin/platform/src/wasm_bundle/mod.rs)

The deployment package also supplies a standalone command.
Its separate entry demonstrates why a package needs control over runtime setup.
[Deployment entry](../../backends/foundation_deployment_platform/src/bin/testbed.rs)

The command table omitted `gen_api normalize`. The current parser registers it, so both invocation forms retain it.
The old platform handlers already parse each command's arguments. The adapter reuses the same command definition and calls the same leaf.
The build helper records directory dependencies because file dependencies alone cannot detect added files.

An upload can reach Maven Central before the client receives its deployment identifier.
A durable attempt record prevents a retry from starting a second upload.
A matching receipt allows the task to continue.
The fake registry reproduces both cases.

Unix assigns each child to a new process group at launch.
Windows starts the child suspended, assigns its job, and resumes the primary thread.
Both mechanisms let the process backend stop descendants as part of cancellation.

A local attempt record does not survive a fresh continuous integration (CI) runner.
The draft release stores a matching attempt asset before the Central upload.
The fake registry test deletes the local record to verify that the next runner still stops.

Valtron can provide one background worker. It cannot block that worker on one pipe while a second pipe fills.
The replacement uses Valtron to schedule child startup.
The child supervisor owns independent operating system workers for the blocking pipes and the child wait.
The native signal task cancels the command through Valtron.
Review wakeups, sink failure, and completion ordering before integration.
