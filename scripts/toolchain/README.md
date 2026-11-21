# Toolchain checks for the user interface

Run these commands from the repository root. [mise.toml](../../mise.toml) pins the tools.
[mise.lock](../../mise.lock) records the download checksums.

## Terms

| Term | Meaning |
|---|---|
| SDK | Software development kit. The tools and files for an operating system. |
| NDK | Native development kit. The tools that build native code for Android. |
| APK | Android application package. The file that an Android device installs. |
| CI | Continuous integration. The service that runs repository checks. |
| UniFFI | Unified Foreign Function Interface. The generator that connects Swift and Kotlin code to Rust. |

## Install and check

Install the pinned versions. Start with the wire check if you only need the schema compiler.

```sh
mise install --yes
mise run toolchain:wire
```

The wire check prints the `flatc` version. The full check also verifies the Android build tools.
On macOS, install the Android SDK packages before the full check. It also checks Xcode.

```sh
mise run toolchain:android
mise exec -- scripts/toolchain/check.sh
```

On Linux the full check skips Xcode and Android SDK package checks.
Run the Android scope explicitly to check the installed packages on Linux.

```sh
mise exec -- scripts/toolchain/check.sh --scope android
```

CI can check Android tool versions before it installs the platform packages.

```sh
mise exec -- scripts/toolchain/check.sh --scope android --without-android-packages
```

## Build an Android example

```sh
mise run toolchain:android-build
mise run toolchain:android
```

The build task provisions the platform, build tools, platform tools and NDK.
It builds a temporary Kotlin app with one launcher activity.
It checks the APK manifest, compiled classes and signature, then removes the temporary app.
The task prints `PASS android build` on success.
The second command prints `All Android SDK packages are present.` when provisioning is complete.

## Verify Apple bindings

Install full Xcode and its iOS simulator SDK on macOS.
Set `DEVELOPER_DIR` if your Xcode is outside the standard application path.

```sh
mise run toolchain:apple
```

This task checks Xcode, generates a Swift binding from the minimal UniFFI project, and compiles it for the simulator.
The proof sources are in [proof/](proof/). The task prints the compiler and SDK versions.

## Validate changes to the toolchain

```sh
bash scripts/toolchain/check_test.sh
bash scripts/toolchain/android_test.sh
bash scripts/toolchain/lock_test.sh
bash scripts/toolchain/drift_test.sh
bash scripts/toolchain/proof_test.sh
```

The shell fixtures exercise missing tools, version mismatches, repeated package setup and binding generation.
The lock test checks the platforms and checksums for the pins.
The drift test checks the Gradle wrapper and Kotlin catalog when the Feature 15 project exists.
