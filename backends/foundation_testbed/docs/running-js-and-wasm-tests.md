# Running JS & wasm tests on the embedded runtime

This is the hands-on guide: write `#[wasm_test]` cases, run them in-process, and —
when you need it — drive raw JS / ES modules through the embedded execution core
directly from Rust.

Prerequisites: the `wasm32-unknown-unknown` target installed, and you build/test
with the **`uat` profile** (this crate's dev profile uses Cranelift, which crashes
compiling it) and the **`wasm-embedded-js`** feature (it links V8).

## 1. Write a `#[wasm_test]` case

Scaffold a starter cases file into your crate:

```bash
cargo run -p foundation_testbed --no-default-features --features wasm-embedded-js \
  --profile uat --bin wasm-testbed -- init owned ./path/to/your-crate
```

That writes `src/wasm_tests.rs`. A case is a plain function annotated with
`#[wasm_test]` (from `foundation_macros`); the macro emits a `__fwt_<name>` export
the harness discovers. The crate must be `crate-type = ["cdylib"]` and depend on
`foundation_wasm` (feature `"web"`) + `foundation_macros`.

```rust
use foundation_macros::wasm_test;

#[wasm_test]
fn passes_simple() {
    assert_eq!(2 + 2, 4);
}

#[wasm_test(should_panic)]
fn panics_as_expected() {
    panic!("boom");           // the expected outcome IS the panic
}

#[wasm_test(ignore)]
fn ignored_case() {
    unreachable!();
}

// async cases get a yield point; the runner awaits the test report.
#[wasm_test]
async fn async_completes_after_yield() {
    foundation_wasm::r#yield().await;
    assert!(true);
}
```

Flags the macro understands: `should_panic` (verdict is inverted — under wasm32
abort semantics the module can't observe its own panic, so the *runner* infers it),
`ignore` (discovered but not run).

## 2. Run them in-process

```bash
# all cases:
cargo run -p foundation_testbed --no-default-features --features wasm-embedded-js \
  --profile uat --bin wasm-testbed -- deno ./path/to/your-crate

# filter by substring:
… -- deno ./path/to/your-crate --filter passes

# or via mise (the platform's own owned-harness e2e):
mise run test:wasm-testbed:deno
```

Output is the familiar per-case + summary lines; the process exit code is `0` when
every case passes, `1` otherwise:

```
ok      passes_simple
ok      panics_as_expected
ok      async_completes_after_yield
ignored ignored_case
FAILED  fails_with_assertion
        fails_with_assertion: panicked at src/lib.rs:39:5:
test result: FAILED. 3 passed; 1 failed; 1 ignored
```

> Same crate, real browser instead of V8: swap `deno` for `web` (`wasm-testbed web
> <crate> --headless`). That path uses Playwright + `foundation_browser`; it does
> need a browser installed. The embedded `deno` path needs nothing.

## 3. Drive it from Rust (the execution-core API)

Everything the CLI does is a thin wrapper over `foundation_testbed::wasm`. Use these
directly in your own Rust tests/tools (all behind `wasm-embedded-js`).

### Run a staged harness → structured result

```rust
use foundation_testbed::wasm::{fwt, fwt_runner, embedded_js};

// You have a built module (e.g. a prebuilt fixture) + a temp dir:
let cases = fwt::discover_cases(&wasm_path)?;            // Vec<FwtCase>
fwt_runner::stage_from_wasm(&wasm_path, &cases, dir)?;   // write runner.mjs + module.wasm + …
let report = embedded_js::run_staged_harness(dir)?;      // HarnessReport

assert_eq!(report.passed, 3);
assert_eq!(report.failed, 1);
assert_eq!(report.exit_code(), 1);          // 0 iff failed == 0
println!("{}", report.output);              // the per-case + summary lines
```

`HarnessReport`:

```rust
pub struct HarnessReport { pub passed: u32, pub failed: u32, pub ignored: u32, pub output: String }
impl HarnessReport { pub fn exit_code(&self) -> i32; }  // 0 when failed == 0, else 1
```

### Full build → run in one call

`fwt_runner::run_deno(&OwnedRunArgs)` does build → discover → stage → embedded run
and returns a `RunOutcome { output, exit_code, cases }`. This is exactly what the
`deno` subcommand dispatches to.

### Run arbitrary JS / an ES module in-process

For non-harness JS (handy in tests and tools), two lower-level entry points:

```rust
use foundation_testbed::wasm::embedded_js;

// (a) a self-contained runtime you can script yourself:
let mut rt = embedded_js::build_runtime()?;             // console, TextEncoder/Decoder, timers, URL ready
let value = rt.execute_script("<inline>", "1 + 2")?;    // synchronous eval

// (b) load + evaluate an ES module file, draining the event loop
//     (so setTimeout / microtasks / async imports settle):
embedded_js::run_module(std::path::Path::new("/abs/path/entry.mjs"))?;
```

`run_module` resolves relative imports from disk via `FsModuleLoader`, so an
`entry.mjs` that does `import { x } from "./util.mjs"` just works. The globals
installed by the bootstrap (`TextEncoder`, `TextDecoder`, `setTimeout`,
`setInterval`, `URL`, `fetch`/`Request`/`Response`/`Headers`, plus deno_core's
`console`) are available; `WebAssembly` is native to V8.

### The smallest possible proof (W1 spike)

If you only want to confirm the engine itself links and runs on your toolchain:

```rust
use foundation_testbed::wasm::embedded_js::{spike_eval_addition, spike_wasm_instantiate};
assert_eq!(spike_eval_addition(), 2.0);     // V8 evaluated 1 + 1
assert!(spike_wasm_instantiate());          // V8 compiled + instantiated a wasm module
```

## 4. Writing a Rust integration test that runs the harness

The pattern in `tests/fwt_runner_tests.rs` — gate on the feature, build the sample,
assert on the outcome:

```rust
#![cfg(feature = "wasm")]

#[cfg(feature = "wasm-embedded-js")]
#[test]
fn my_crate_is_green() {
    use foundation_testbed::wasm::cli::{Browser, OwnedRunArgs};
    use foundation_testbed::wasm::fwt_runner::run_deno;

    let args = OwnedRunArgs {
        crate_path: "path/to/crate".into(),
        release: false, features: None, filter: Some("passes".into()),
        browser: Browser::Chrome, headless: true,   // unused by the deno runner
    };
    let outcome = run_deno(&args).expect("embedded runner executes");
    assert_eq!(outcome.exit_code, 0);
}
```

No `node`/`deno` availability check, no skip — the runtime is in the binary. The
only external requirement is the `wasm32` target for the inner `cargo build`.

## Gotchas

- **Profile:** always `--profile uat`. The dev (Cranelift) profile crashes
  compiling this crate.
- **Feature:** the `deno` runner only exists with `wasm-embedded-js`. Without it,
  `run_deno` returns `EmbeddedRuntimeDisabled` with an actionable message; build it
  in or use the `web` runner.
- **First build is heavy:** V8 is a large prebuilt static lib — the first compile
  links hundreds of MB and takes a while. Subsequent builds are cached.
- **No `crypto.subtle`** yet — see
  [extending the runtime](./extending-the-runtime.md#adding-web-crypto-cryptosubtle).
