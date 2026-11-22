# Android package starter

This application displays a remote view with the published Ewe UI Compose renderer.
The example calls `NativeCore.remote` to register the server action `refresh_count`.
It contains its own activity and application resources. Gradle resolves the renderer, native bindings and Rust libraries through the Maven dependency.

Set `eweVersion` in `gradle.properties` to a released version.
The example server exposes its first view at `/open-count`. The **Refresh** button requests the current count.

For local staging, start the example server from the monorepo and set the staging repository path.

```sh
cargo run --profile uat -p ui_todo -- 127.0.0.1:8180
```

From a second terminal in the monorepo, use the wrapper and these commands.

```sh
mise exec -- mobile/android/gradlew -p examples/mobile_starter/android -PeweRepository="$PWD/target/native-ui/maven" assembleDebug
adb reverse tcp:8180 tcp:8180
adb install -r examples/mobile_starter/android/app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n ewe.starter/.MainActivity --es server http://127.0.0.1:8180/
```

The emulator uses HTTP for local debugging. Supply an HTTPS server for a release build.

[Wire format, fixtures and Kotlin package API](../../../mobile/android/README.md)
