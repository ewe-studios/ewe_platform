# Create an Ewe user interface application

[Operations manuals](docs/README.md) cover running, building, and publishing.

Run `ewe_ui new` in a terminal. Enter the application name and select its targets.
Use the arrow keys to move. Press Space to select a target. Press Escape to cancel.

Use complete arguments in automation.

```sh
ewe_ui new --name FieldNotes --target web --target android --target ios --app-id com.example.fieldnotes --host server --output .
cd FieldNotes
ewe_ui build --target web
ewe_ui build --target web --offline
ewe_ui devices list --target android
ewe_ui devices list --target ios
```

The application directory contains one `application` crate and one directory for each selected target.
The `ewe-ui.toml` manifest records the application identity, shared directory, and native project settings.
The on-device host also generates one Rust library for both mobile targets.

## Build and run

- Web needs Cargo, its WebAssembly target, the wasm-bindgen tools, and npm. The web build writes `web/dist`.
- Android needs the Android software development kit, Java, and the generated Gradle wrapper.
- iOS needs Xcode on macOS. Install the target simulator runtime in Xcode.
- On-device projects need the selected Rust targets. Android also needs the native development kit and cargo-ndk.

Run `ewe_ui build --target android` to build a debug application package.
Run `ewe_ui build --target ios` to build for the iOS simulator.
Supply a device identifier from the device list when you start or run the application.

```sh
ewe_ui devices start --target android --device DEVICE_ID
ewe_ui run --target android --device DEVICE_ID
ewe_ui run --target ios --device SIMULATOR_ID
ewe_ui task run native:build-ios --help
```

For a signed Android release, set `EWE_ANDROID_KEYSTORE`, `EWE_ANDROID_STORE_PASSWORD`, `EWE_ANDROID_KEY_ALIAS`, and `EWE_ANDROID_KEY_PASSWORD`.
Then run `ewe_ui build --target android --release`.

## Add an application task

Create a Rust package with a library and a thin executable under `tools/tasks`.
The library exports a `foundation_tasks::TaskSet`. Its executable registers the set with a `TaskRunner`.
Declare the same namespace, commands, and arguments in the application manifest.

```toml
[tasks]
manifest = "tools/tasks/Cargo.toml"
namespace = "fieldnotes"

[[tasks.commands]]
name = "check-record"
description = "Check the specified record"

[[tasks.commands.arguments]]
name = "record"
description = "Record identifier"
required = true
```

An argument accepts a long option by default. Set `kind` to `flag` or `positional` when needed.
Set `repeat = true` for a repeated option or final positional argument.
The executable runs from the application root. Each argument reaches Cargo as a separate value.
The command returns the task's exit status.

```sh
ewe_ui task list
ewe_ui task run fieldnotes:check-record --record example
ewe_ui help --llm
```

The guide includes all registered commands, options, and application tasks.
