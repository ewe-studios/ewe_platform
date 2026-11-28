# Run tests in native, WebAssembly, and browser hosts

Run these commands from the repository root.

```sh
cargo test --locked --profile uat -p foundation_macros --test foundation_macros_tests
```

```sh
cargo test --locked --profile uat -p foundation_core --features multi --test mod valtron_macro_tests
```

```sh
cargo test --locked --profile uat -p foundation_core --features multi --test mod timeout_guard_unwind
```

```sh
cargo test --locked --profile uat -p foundation_browser --test smoke macro_drives_a_served_page
```

```sh
mise run test:wasm-testbed:deno
```

```sh
mise run test:wasm-bindgen:core
```

The following fixtures give the setup and assertions.

- [Compile a named WebAssembly entry point](../../tests/units/foundation_macros_wasm_entrypoint_tests.rs)
- [Run asynchronous and result-returning Valtron bodies](../../../foundation_core/tests/valtron/valtron_macro_tests.rs)
- [Check the timeout guard during unwinding](../../../foundation_core/tests/valtron/timeout_guard_unwind.rs)
- [Run a browser and test server together](../../../foundation_browser/tests/smoke.rs)
- [Export and run a WebAssembly test](../../../foundation_core/integration/wasm_tests/src/lib.rs)

The browser scenario needs Chromium. The WebAssembly scenarios use the pinned host tools.
The Feature 12 map keeps Worker logging as a separate runtime proof gap.
