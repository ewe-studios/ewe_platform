# Build an Ewe application

Run application build commands from the directory that contains `ewe-ui.toml`.
Declare the selected target in that file. The command prints the target, profile, and artifact path on success.

| Target | Required tools | Output |
| --- | --- | --- |
| Web | Rust with the `wasm32-unknown-unknown` target, matching wasm-bindgen tools, and npm | `web/dist` with the browser files |
| Android | Java Development Kit (JDK), Android Software Development Kit (SDK), and the project Gradle wrapper | Debug Android application package (APK) or signed release Android App Bundle (AAB) |
| iOS | Xcode on macOS with an installed iOS simulator runtime | Debug or Release simulator application under `ios/build/Build/Products` |

Use the repository tool versions for work in this checkout. For an on-device Android host, install the Native Development Kit (NDK), cargo-ndk, and the Rust targets for the configured architectures. For an on-device iOS host, install the Rust targets for the simulator and device libraries.
[Tool versions](../../../../mise.toml) · [Rust toolchain](../../../../rust-toolchain.toml) · [Native library preparation](../../../../backends/foundation_ui_native/src/tasks/project/libraries.rs)

## Build generated targets

```sh
ewe_ui build --target web
ewe_ui build --target web --release
ewe_ui build --target web --offline
ewe_ui build --target android
ewe_ui build --target ios --release
```

Use `--offline` after Cargo and npm dependencies are in the local caches. The option applies to web builds.
The iOS release flag selects the Xcode Release configuration for the simulator.
For a signed Android release, set `EWE_ANDROID_KEYSTORE`, `EWE_ANDROID_STORE_PASSWORD`, `EWE_ANDROID_KEY_ALIAS`, and `EWE_ANDROID_KEY_PASSWORD`.
Then run `ewe_ui build --target android --release`.
Use the module declared in the manifest to find the output under `android/<module>/build/outputs`.
The build checks the native library entries in the archive and verifies the release signature with `jarsigner`.
The iOS build checks for the simulator application's `Info.plist`.
[Command implementation](../../src/operations.rs) · [Native build and verification](../../../../backends/foundation_ui_native/src/tasks/project/mod.rs)

Use native preparation before a manual Xcode or Gradle build when you need to stage the on-device libraries and bindings.

```sh
ewe_ui task run app:prepare --target ios
ewe_ui task run app:prepare --target android
```

Add `--release` to stage release inputs. Use the generated tool projects and their declared build configurations.
Native application build aliases `native:build-ios` and `native:build-android` select the target in the current application.
Use the checkout's `native-task` command for framework builds.
[Application task registration](../../src/operations.rs) · [Native release tools](../../../../scripts/ui_native/release/README.md)

## Build and validate the Todo example

From the repository root, build the example server. Run the browser test recipe when the browser test tools are available.

```sh
cargo build --locked --profile uat -p ui_todo --bin ui_todo
cargo test --profile uat -p ui_todo --features browser-tests --test todo_browser_tests
```

From `examples/ui_todo`, use the application task commands for native hosts. Use each platform's required tools.

```sh
ewe_ui task run todo:prepare-ios
ewe_ui task run todo:sync-android --check
ewe_ui task run todo:build-android --with-todo
ewe_ui task run todo:test-ios
ewe_ui task run todo:test-android --maven /absolute/path/to/local/maven
```

Replace the Maven path with the repository that contains the staged framework packages.
The Android guide gives the build and staging sequence. The automated tests run both native Todo hosts on the selected platform.
Use the browser and device checks to confirm that the application starts and handles Todo actions.
[Todo test requirements](../../../../examples/ui_todo/README.md) · [Android build and staging sequence](../../../../examples/ui_todo/android/README.md) · [Task arguments](../../../../examples/ui_todo/ewe-ui.toml)
