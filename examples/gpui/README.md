# Zed GPUI counter

This standalone example opens a desktop window with the official Zed GPUI crates.
It uses the [reviewed Zed revision](https://github.com/zed-industries/zed/commit/46375fbf64574f01dc08fd2d2998aa7bbb7f0377).
The independent Cargo manifest and lockfile keep the example's dependencies in this package.
Tauri is the first product desktop path in [Specification 92](../../specifications/92-desktop-renderer-decision/requirements.md).

[Start with the code walkthrough](../../specifications/92-desktop-renderer-decision/gpui-counter-walkthrough.md) to follow the example from build to input.

## Build requirements

- Install Rust and Cargo. The upstream checkout declares Rust 1.100.0.
- On macOS, install the Xcode Command Line Tools. The example uses the Metal graphics backend.
- On Windows, use the Microsoft Visual C++ (MSVC) Rust target and install the Windows Software Development Kit (SDK).
- On Linux, use a Wayland or X11 desktop session. Install the compiler, windowing and graphics libraries for your distribution.

Use the [upstream GPUI guide](https://github.com/zed-industries/zed/blob/46375fbf64574f01dc08fd2d2998aa7bbb7f0377/crates/gpui/README.md) for platform features.
See the [upstream Linux dependency list](https://github.com/zed-industries/zed/blob/46375fbf64574f01dc08fd2d2998aa7bbb7f0377/script/linux) for package names.

## Run

Run this command from the repository root. Use `uat` from the start for native code and any code Cranelift cannot compile.
The repository development environment uses Cranelift for its normal `dev` profile and LLVM for `uat`.
[Repository build settings](../../mise.toml#L29)

```sh
cargo run --locked --profile uat --manifest-path examples/gpui/Cargo.toml
```

The window shows **Count: 0**. Click **Add one** twice to see 1 and 2.
Use Tab to focus the button. Press Enter or Space to activate it.
Close the window and run the command again to start a new counter at zero.

Use these commands to inspect the resolved graph and compile the example test target.

```sh
cargo tree --locked --manifest-path examples/gpui/Cargo.toml
cargo test --locked --profile uat --manifest-path examples/gpui/Cargo.toml
```

See [the evaluation](../../specifications/92-desktop-renderer-decision/features/01-backend-evaluation/feature.md#standalone-upstream-gpui-example) for observed platform results.
