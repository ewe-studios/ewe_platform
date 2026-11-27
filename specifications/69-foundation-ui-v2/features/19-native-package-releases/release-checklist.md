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
The Rust crate uses the separate Cargo release workflow.
Check its public index entry and dependency resolution in a fresh generated project.

## After explicit release approval

1. Confirm the chosen version and source commit with the release owner.
2. Confirm access to the protected release environment and the verified Maven namespace.
3. Create the approved native release tag. Let the protected job sign and publish the verified inputs.
4. Check the GitHub asset digests, Swift tag revision and Maven deployment receipt.
5. Run the public starter checks with clean package caches.
6. Build a newly generated application from the public Rust and native packages.
7. Record the resolved versions and test results. Update the default generator versions after both starter checks pass.

[Release procedure](../../../../scripts/ui_native/release/README.md)
[Consumer issue 845](https://github.com/ewe-studios/ewe_platform/issues/845)
