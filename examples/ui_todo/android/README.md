# Android todo apps

Both apps use the [Ewe UI Android library](../../../mobile/android/README.md) and the same Rust todo actions and views.

| App | Gradle project | Task store |
|---|---|---|
| [Todo Server](todo_server/) | `todo_server` | Store in the Rust server |
| [Todo On Device](todo_on_device/) | `todo_on_device` | Store in the Android app |

## Build

Run these commands from the repository root. The applications build independently and can install on the same emulator.

```sh
mise run toolchain:android
scripts/ui_native/sync-android.sh
scripts/ui_native/build-android.sh --with-todo
repo="$PWD/target/native-ui/maven"
mise exec -- mobile/android/gradlew -p mobile/android publishToMavenLocal "-Dmaven.repo.local=$repo"
for app in todo_server todo_on_device; do
  mise exec -- mobile/android/gradlew -p "examples/ui_todo/android/$app" "-PeweRepository=$repo" :app:assembleDebug
  adb install -r "examples/ui_todo/android/$app/app/build/outputs/apk/debug/app-debug.apk"
done
```

Select one emulator in Android Debug Bridge (ADB) before installing. Run the example commands with `mise exec -- adb` when needed.
The native build copies the application library to each app. Both apps compile the same generated application bindings from `generated/`.

## Test on a device

The device test installs both apps on one emulator. It starts the local app before it starts a fresh Rust server.

```sh
mise exec -- scripts/ui_native/test-android-todo.sh --maven "$repo"
```

Set `EWE_ANDROID_SERIAL` to reuse an existing emulator. Otherwise the script starts and stops a private emulator.
