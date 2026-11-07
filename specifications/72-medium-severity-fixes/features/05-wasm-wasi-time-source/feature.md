---
feature: "The WASI host takes its instant from a TimeSource, not the real clock"
status: "in-review"
priority: "medium"
depends_on: []
crates: ["foundation_wasm"]
issues: [219]
model: "sonnet"
branch: "spec72/f05-wasm-wasi-time-source"
---

# Feature 05: WASI time source

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** WebAssembly (Wasm); WebAssembly System Interface (WASI);
application programming interface (API); AeroSpace and Defence Industries Association of Europe
(ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Remove the last direct `Instant::now()` read in `foundation_wasm`, which sits at the WASI
extern-C ABI boundary. Close issue
[#219](https://github.com/ewe-studios/ewe_platform/issues/219).

## 2. Problem

`backends/foundation_wasm/src/wasi_host.rs:41` reads `Instant::now()` directly, marked with
`// time-source:`. The issue calls this an extern-C ABI boundary; the real code is a plain
private Rust helper whose callers are ordinary Rust functions. Only the `wasi_*` exports in
`host_runtime.rs` cross the ABI, and an embedder, not our code, calls those.

## 3. Requirements

1. The host environment holds a `TimeSource` (or the crate's equivalent clock trait) that the
   host installs before the module runs (root rule 11).
2. The extern-C function reads the instant from that source. It does not read the real clock.
3. A source that is not installed is a typed error or a documented default chosen by the host
   builder. Never a hidden global.
4. The `// time-source:` marker becomes a normal comment that names the injected source.
5. If the ABI can carry an explicit instant argument without breaking a caller, prefer that.
   Record the choice and the reason in `progress.md`.

## 4. Design

Two shapes are acceptable:

- **Host installation.** `WasiHost` gains `with_time_source(src)`. The extern-C function reads
  the source from the host state it already receives. This is the preferred shape because it
  needs no ABI change.
- **ABI argument.** Add an `now: u64` parameter only if the caller is ours and the WASI ABI
  permits it.

The time source is a trait object owned by the host environment struct, not a static.

## 5. Tasks

- [x] Read `wasi_host.rs` and the host environment struct. Record how the clock read reaches
      host state.
- [x] Add the time source and the builder method.
- [x] Replace the direct read. Remove the marker.
- [x] Add the regression tests.
- [x] Run the verify commands. Record the results.
- [x] Close #219 in the PR body.

## 6. Tests

In `backends/foundation_wasm/tests/`:

1. `the_wasi_clock_reads_the_installed_source`: install a fixed simulated source, call the
   function, and assert the returned instant is the simulated one. On the old code the value
   is the real clock, so the assertion fails.
2. `a_missing_source_is_a_typed_error_or_the_documented_default`: assert the chosen
   behaviour.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_wasm
cargo clippy --profile uat -p foundation_wasm --all-targets -- -D warnings
cargo test --profile uat -p foundation_wasm
cargo check -p foundation_wasm --target wasm32-unknown-unknown
```

## 8. Done when

- `rg -n 'Instant::now|SystemTime::now' backends/foundation_wasm/src` finds no core read.
- The new test fails on the old code and passes on the new code.
- Issue #219 is closed by the PR.
- `progress.md` and `learnings.md` record the choice and the reason.
