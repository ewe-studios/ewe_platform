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

Always pass `--profile uat` for local Cargo builds, runs, tests, and Clippy checks on every platform.
The repository configures LLVM for this profile. [Repository build settings](../../mise.toml#L29)

### Shell setup

Start in the repository root. On macOS, load `.envrc` before you invoke Cargo.
Use `cargo metadata` to check that `target_directory` is the shared target path for your host.

```sh
. ./.envrc
cargo metadata --locked --no-deps --format-version 1 --manifest-path examples/gpui/Cargo.toml
cargo run --locked --profile uat --manifest-path examples/gpui/Cargo.toml
```

On Linux, use the same commands with the host configuration for shared Cargo storage.
Load `.envrc` only if it contains paths for that host.

On Windows, open PowerShell from the configured local build environment.
Use the Windows values for `CARGO_HOME` and `CARGO_TARGET_DIR`. Check that Cargo reports the configured shared target.

```powershell
if (-not $env:CARGO_HOME -or -not $env:CARGO_TARGET_DIR) {
    throw "Load the configured Windows Cargo home and target directory."
}
(cargo metadata --locked --no-deps --format-version 1 --manifest-path examples/gpui/Cargo.toml | ConvertFrom-Json).target_directory
cargo run --locked --profile uat --manifest-path examples/gpui/Cargo.toml
```

Cargo runs the executable from the effective target directory. For direct launch, use the executable path that Cargo reports.

The window shows **Count: 0**. Click **Add one** twice to see 1 and 2.
Use Tab to focus the button. Press Enter or Space to activate it.
Close the window and run the command again to start a new counter at zero.

Use these commands to inspect the graph, build the executable, compile the test target, and run Clippy.

```sh
cargo tree --locked --manifest-path examples/gpui/Cargo.toml
cargo build --locked --profile uat --manifest-path examples/gpui/Cargo.toml
cargo test --locked --profile uat --manifest-path examples/gpui/Cargo.toml
cargo clippy --locked --profile uat --manifest-path examples/gpui/Cargo.toml --no-deps
```

See [the evaluation](../../specifications/92-desktop-renderer-decision/features/01-backend-evaluation/feature.md#standalone-upstream-gpui-example) for observed platform results.
