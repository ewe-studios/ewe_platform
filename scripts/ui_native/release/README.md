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
Check out the native repositories at the commits in `release/native-ui.toml`.
The package version must match the Rust crate, Swift manifest and Gradle project.
The Swift binary target must reference the archive URL for this version.
The candidate records the final checksum and Swift tag revision.
If the build produces a different archive, the release changes only the checksum in a separate Swift commit.

```sh
export EWE_UI_SWIFT=/path/to/ewe-ui-swift
export EWE_UI_KOTLIN=/path/to/ewe-ui-kotlin
mise run toolchain:android
scripts/ui_native/sync-ios.sh --check
scripts/ui_native/sync-android.sh --check
python3 scripts/ui/check_behaviours.py ios --package "$EWE_UI_SWIFT"
python3 scripts/ui/check_behaviours.py android --package "$EWE_UI_KOTLIN"
scripts/ui_native/release/prepare.sh --config release/native-ui.toml --output dist/native-ui
scripts/ui_native/release/verify.sh --candidate dist/native-ui --templates both
```

The prepare command validates the Apple archive, Maven archives and Maven dependency declarations.
It writes the inputs and their SHA-256 digests to `native-ui.json`.
Verification builds the Swift starter with the candidate archive. It builds the Android starter using a temporary Maven repository.
The public Android check uses a fresh Gradle cache.

Run the package-only Android starter on an emulator with the local Maven candidate.

```sh
mise exec -- scripts/ui_native/release/test-android-starter.sh --maven "$PWD/target/native-ui/maven"
```

Omit `--maven` to resolve the release version from Maven Central.
The script creates a temporary Android 36 emulator and starts the Rust todo host.
It checks the visible count, adds one todo through the public action endpoint, and taps **Refresh**.
The test passes when the count changes from zero to one.
Linux runners need read and write access to `/dev/kvm` for hardware acceleration.

Run the same count check on an iPhone simulator with Xcode and an installed iOS runtime.
Point the local Swift package at the candidate archive before the test.

```sh
mise exec -- scripts/ui_native/release/test-ios-starter.sh --local-package "$EWE_UI_SWIFT"
```

Omit `--local-package` to test the published Swift package. The script removes its temporary simulator after the test.

## Publish one version

Push the three approved source commits to their repositories before creating the release tag.
Protect the `native-ui-release` environment in GitHub. Configure these environment secrets.

| Setting | Purpose |
|---|---|
| `NATIVE_RELEASE_TOKEN` | Create releases in the core repository and tags in the two source repositories. |
| `NATIVE_GPG_PRIVATE_KEY` | Sign each Maven publication file using an isolated OpenPGP keyring. |
| `NATIVE_GPG_FINGERPRINT` (environment variable) | Select the signing key. |
| `CENTRAL_USERNAME`, `CENTRAL_PASSWORD` | Authenticate with a Central Portal user token for the verified Maven namespace. |

Run the manual workflow to build a read-only preview. A tag such as `native-ui-v0.1.0` runs the protected publication job.
Keep the signed workflow artifact for a retry. The release command compares each existing tag and asset before writing.
A Central deployment receipt on the draft release records the deployment identifier. The command resumes that deployment after an interruption.

```sh
scripts/ui_native/release/publish.sh --candidate /path/to/signed-candidate --resume
scripts/ui_native/release/verify.sh --candidate /path/to/signed-candidate --templates both --public
```

The release job publishes the Apple archive, Android bundle and manifest on GitHub.
It tags both native source repositories with the same version. Submit the Swift repository to Swift Package Index after the first public tag.
SwiftPM can resolve the tag directly. Follow the portal procedure if Central reports a failed deployment. [Central Portal publishing](https://central.sonatype.org/publish/publish-portal-api/) · [Swift Package Index submission](https://swiftpackageindex.com/add-a-package)

## Install the candidate version after publication

Add the Swift package by its exact version. Import `EweUI` in the application.

```swift
.package(url: "https://github.com/ewe-studios/ewe-ui-swift.git", exact: "0.1.0")
```

Add `google()` and `mavenCentral()` to the Android repositories. Add the renderer dependency.

```kotlin
implementation("io.github.ewe-studios:ewe-ui:0.1.0")
```

The renderer pulls the binding and core modules. The package-only starters call a remote Rust action.
The separate todo examples show how an application can also register local Rust actions.
