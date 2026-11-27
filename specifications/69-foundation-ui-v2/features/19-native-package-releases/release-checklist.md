# Draft native package release checklist

The preview targets native user interface (UI) version 0.1.2 on its own branch.
The local Rust native crate uses that version.
The shared branch still selects 0.1.1 for the public mobile packages.
The preview leaves public registries and tags untouched.

## Verify the candidate

1. Select one clean source commit for both native builds.
2. Match the release settings, generator settings, Android package and starter versions on the preview branch.
3. Run the native package workflow manually on a branch. Keep the candidate and its build record.
4. Check the source revision and Secure Hash Algorithm 256 (SHA-256) digests in `native-ui.json`.
5. Build both starters with the candidate. The Apple starter uses the candidate archive.
6. Use the candidate Maven repository for the Android starter. Check its dependency records.

The workflow fixes the compiler path and Android Software Development Kit (SDK) selection before building.
The Apple archive task records a separate Swift manifest commit with the final digest.
Use that recorded commit for the Swift Package Manager (SwiftPM) version tag after release approval.

## Verify the public inputs

| Input | Preview finding |
|---|---|
| Cargo build dependency `foundation_resources` | The crates.io sparse index returns 404 for the crate. The application manifest accepts compatible versions from 0.1.1. |
| Android renderer `io.github.ewe-studios:ewe-ui:0.1.1` | Maven Central and Google Maven return 404 for the package descriptor. |
| Swift package `EweUI` version 0.1.1 | The repository has no matching semantic version tag. The native release endpoint returns 404. |

The public starter checks remain pending until the selected release inputs resolve.
The local Rust package lists its source files from a clean clone.
Its package check stops at the first missing Cargo dependency.
The separate Cargo release workflow must publish compatible dependency versions before the resource crate.

| Cargo dependency | Required range | Latest active stable version in the public index |
|---|---|---|
| `foundation_codegen` | `^0.2.3` | `0.2.2` |
| `foundation_wasm_ui` | `^0.1.13` | `0.1.4` |
| `foundation_http` | `^0.0.17` | `0.0.15` |
| `foundation_netio` | `^0.0.19` | `0.0.17` |

Check the public index entries and dependency resolution in a fresh generated project.
[Code generator index](https://index.crates.io/fo/un/foundation_codegen) · [Browser renderer index](https://index.crates.io/fo/un/foundation_wasm_ui) · [HTTP library index](https://index.crates.io/fo/un/foundation_http) · [Network library index](https://index.crates.io/fo/un/foundation_netio)

The [manual candidate run](https://github.com/ewe-studios/ewe_platform/actions/runs/46053814588) uses the preview branch commit `3bb99148e`.
This run builds the candidate and its starter checks before any publication request.

## Publication approval record

The GitHub environment query returns zero records for `native-ui-release`.
The deployment query also returns zero records for that environment.
Keep the release tag pending until the release owner approves the verified candidate and the protected environment is ready.

## After explicit release approval

1. Confirm the chosen version and source commit with the release owner.
2. Configure the protected `native-ui-release` environment and verify the Maven namespace.
3. Create the approved native release tag. Let the protected job sign and publish the verified inputs.
4. Check the GitHub asset digests, Swift tag revision and Maven deployment receipt.
5. Run the public starter checks with clean package caches.
6. Build a newly generated application from the public Rust and native packages.
7. Record the resolved versions and test results. Update the default generator versions after both starter checks pass.

[Release procedure](../../../../scripts/ui_native/release/README.md)
[Consumer issue 845](https://github.com/ewe-studios/ewe_platform/issues/845)

## Managed Maven versions

The Apple preview builds the libraries and stops when it reads the Android package descriptor.
Gradle imports the Compose bill of materials (BOM).
The descriptor omits literal versions for dependency versions that the BOM sets.
Permit these external dependency entries. Require the exact candidate version for both Ewe dependencies.
Check the Maven descriptor path in error messages. Verify the generated descriptor and a failing Ewe version case.
[Issue 1165](https://github.com/ewe-studios/ewe_platform/issues/1165)

## Todo browser binding command

The Android preview assembles both Todo application variants from the shared build provider.
The device test task then builds the Todo server.
The server build needs the locked WebAssembly binding command to generate browser bindings.
Install the command in both preview jobs before the Todo tests.
[Issue 1166](https://github.com/ewe-studios/ewe_platform/issues/1166)
