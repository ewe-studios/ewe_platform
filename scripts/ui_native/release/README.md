# Release the native user interface packages

The native user interface (UI) release contains a Swift package and three Maven modules.
Both packages use the same Rust core and the same wire schema.
A candidate is the set of files that the release job verifies before publication.

| Term | Meaning |
|---|---|
| Swift Package Manager (SwiftPM) | The Apple tool that resolves the Swift repository tag. |
| XCFramework | An Apple binary package with a library for each supported platform and processor. |
| Maven coordinate | The group, artifact name and version that Gradle resolves. |
| Secure Hash Algorithm 256 (SHA-256) | The digest that identifies each release file. |
| OpenPGP | The signing format that Maven Central accepts. |

## Build and verify a candidate

Use a macOS host with Xcode. Install the tools listed in the repository toolchain file.
Use one clean ewe_platform checkout. Swift sources live in `mobile/ios/`. Kotlin sources live in `mobile/android/`.
The package version must match the Rust crate, Swift manifest and Gradle project.
The Swift binary target must reference the archive URL for this version.
The candidate records the final checksum and Swift tag revision.
If the build produces a different archive, the release changes only the checksum in a separate Swift commit.

```sh
cargo install --locked --path backends/foundation_ui_native --features tasks --bin native-task
mise run toolchain:android
native-task task run native:bindings:sync-ios --check
native-task task run native:bindings:sync-android --check
python3 scripts/ui/check_behaviours.py ios
python3 scripts/ui/check_behaviours.py android
native-task task run native:release:prepare --config release/native-ui.toml --output dist/native-ui
native-task task run native:release:verify --candidate dist/native-ui --templates both
```

The prepare command validates the Apple archive, Maven archives and Maven dependency declarations.
It writes the inputs and their SHA-256 digests to `native-ui.json`.
Verification builds the Swift starter with the candidate archive. It builds the Android starter using a temporary Maven repository.
The public Android check uses a fresh Gradle cache.

Run the package-only Android starter on an emulator with the local Maven candidate.

```sh
mise exec -- native-task task run native:release:test-android-starter --maven "$PWD/target/native-ui/maven"
```

Omit `--maven` to resolve the release version from Maven Central.
The task creates a temporary Android 36 emulator and starts the Rust todo host.
It checks the visible count, adds one todo through the public action endpoint, and taps **Refresh**.
The test passes when the count changes from zero to one.
Linux runners need read and write access to `/dev/kvm` for hardware acceleration.

Run the same count check on an iPhone simulator with Xcode and an installed iOS runtime.
Point the local Swift package at the candidate archive before the test.

```sh
mise exec -- native-task task run native:release:test-ios-starter --local-package "$PWD"
```

Omit `--local-package` to test the published Swift package. The script removes its temporary simulator after the test.

## Publish one version

Push the approved monorepo commit before creating the release tag.
Protect the `native-ui-release` environment in GitHub. Configure these environment secrets.

| Setting | Purpose |
|---|---|
| `NATIVE_RELEASE_TOKEN` | Create the GitHub release and the Swift version tag in ewe_platform. |
| `NATIVE_GPG_PRIVATE_KEY` | Sign each Maven publication file using an isolated OpenPGP keyring. |
| `NATIVE_GPG_FINGERPRINT` (environment variable) | Select the signing key. |
| `CENTRAL_USERNAME`, `CENTRAL_PASSWORD` | Authenticate with a Central Portal user token for the verified Maven namespace. |

Run the manual workflow to build a read-only preview. A tag such as `native-ui-v0.1.1` runs the protected publication job.
Keep the signed workflow artifact for a retry. The release command compares each existing tag and asset before writing.
A Central deployment receipt on the draft release records the deployment identifier. The command resumes that deployment after an interruption.

```sh
native-task task run native:release:publish --candidate /path/to/signed-candidate --resume
native-task task run native:release:verify --candidate /path/to/signed-candidate --templates both --public
```

The release job publishes the Apple archive, Android bundle and manifest on GitHub.
It tags the checksum-bearing root Swift manifest in ewe_platform. Submit the monorepo package to Swift Package Index after the first public tag.
SwiftPM can resolve the tag directly. Follow the portal procedure if Central reports a failed deployment. [Central Portal publishing](https://central.sonatype.org/publish/publish-portal-api/) · [Swift Package Index submission](https://swiftpackageindex.com/add-a-package)

## Install the candidate version after publication

Add the Swift package by its exact version. Import `EweUI` in the application.

```swift
.package(url: "https://github.com/ewe-studios/ewe_platform.git", exact: "0.1.1")
```

Add `google()` and `mavenCentral()` to the Android repositories. Add the renderer dependency.

```kotlin
implementation("io.github.ewe-studios:ewe-ui:0.1.1")
```

The renderer pulls the binding and core modules. The package-only starters call a remote Rust action.
The separate todo examples show how an application can also register local Rust actions.

When you change release coordinates, copy `release/native-ui.toml` to `backends/foundation_ui_project/mobile-release.toml`. The command packages these settings with the mobile starters.

## Run a native task

The native package and the platform executable register the same task set.
Run these commands from the repository checkout.

```sh
platform task ls
platform task run native:build-android --with-todo
platform task run native:release:prepare --config release/native-ui.toml --output dist/native-ui
platform task run native:release:verify --candidate dist/native-ui --templates both
cargo run -p foundation_ui_native --features tasks --bin native-task -- task ls
```

Import the trusted public signing key and set `NATIVE_GPG_FINGERPRINT` before you publish.
The runner checks every signature against that key.

Keep the draft release and the upload-attempt asset if a Maven Central upload returns an uncertain result.
Find the deployment using the candidate name printed in the attempt asset.
Supply a receipt with the matching deployment name and identifier before you resume.
The resume check accepts a matching receipt and verifies it with Maven Central before continuing.
