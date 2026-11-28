# Native release prerequisites

Cargo identifies a package by its name, version, and source.
A Cargo source can be a registry, a Git revision, or a local path.
A package archive is the file that Cargo publishes for one crate version.

## Selected sources

The public Swift starter requires the semantic version tag and its checksummed archive.
The Android starter requires three Maven Central modules at the same release version.
These are `ewe-ui-core`, `ewe-ui-binding`, and `ewe-ui` under `io.github.ewe-studios`.
The GitHub Release attaches the Apple archive, Android publication archives, and release manifest.

A composed project with an Android or iPhone target now takes Rust resources from the native release Git tag.
The native core and any browser build in that project use the same source.
A web-only project keeps its registry dependency.
The native starters and composed mobile release path require no new crates.io publication.

The current default remains 0.1.1 until the public starter checks pass for its replacement.
The unsigned 0.1.2 preview now passes on the exact proposed release source.

## Public availability

On 28 November 2026, neither 0.1.1 nor 0.1.2 had any of the three descriptors in Maven Central.
The public repository had neither native release tag nor matching Swift semantic tag.
The GitHub Release endpoint was available for the repository, but returned 404 for both versions.
A fresh Cargo directory failed to resolve the old resource declaration from crates.io.
[Public resource index](https://index.crates.io/fo/un/foundation_resources) · [Maven repository](https://repo.maven.apache.org/maven2/io/github/ewe-studios/) · [Release tags](https://github.com/ewe-studios/ewe_platform/tags)

## Former registry path

The former generated mobile resource declaration selected `foundation_resources ^0.1.1`.
The table records the normal and build dependency order for that crate at `7d4422f48`.
It includes optional dependencies so each published manifest resolves.
At the earlier native preview, the resource and database versions were 0.1.2 and 0.0.12.
The other entries match the later values below.

| Crate | Workspace version | Public version status |
| --- | --- | --- |
| `foundation_buildtools` | `0.0.3` | Present |
| `foundation_compact` | `0.2.7` | Missing |
| `foundation_errstacks` | `0.0.4` | Present |
| `foundation_nostd` | `0.0.10` | Present |
| `foundation_theme` | `0.0.4` | Present |
| `foundation_ui_traits` | `0.0.6` | Missing |
| `foundation_arrow` | `0.0.6` | Missing |
| `foundation_context` | `0.0.4` | Present |
| `foundation_macros` | `0.0.25` | Missing |
| `foundation_ui_wire` | `0.0.7` | Missing |
| `foundation_vfs` | `0.0.1` | Present |
| `foundation_cloudflare_types` | `0.1.3` | Present |
| `foundation_codegen` | `0.2.3` | Missing |
| `foundation_ui_runtime` | `0.0.8` | Missing |
| `foundation_wasm` | `0.0.11` | Missing |
| `foundation_core` | `0.0.23` | Present |
| `foundation_reactor` | `0.0.5` | Present |
| `foundation_signals` | `0.0.7` | Missing |
| `foundation_netio` | `0.0.19` | Missing |
| `foundation_db` | `0.0.13` | Missing |
| `foundation_nativeapis` | `0.0.16` | Present |
| `foundation_iogate` | `0.0.7` | Missing |
| `foundation_tasks` | `0.1.1` | Missing |
| `foundation_http` | `0.0.17` | Missing |
| `foundation_wasm_ui` | `0.1.13` | Missing |
| `foundation_resources` | `0.1.3` | Missing |

The retained versioned test dependency `foundation_auth 0.0.16` also exists in the registry.
The fixed macro archive removes the test dependency that previously caused a publication cycle.
[Publication cycle repair](https://github.com/ewe-studios/ewe_platform/issues/1196)

All 26 names have one matching entry in each workspace lock file.
The 16 absent versions alone do not reproduce that lock from public inputs.
The public `foundation_core 0.0.23` and `foundation_cloudflare_types 0.1.3` require `foundation_macros ^0.0.23`.
The current crate graph requires `foundation_macros ^0.0.25`.

Seven available archives differ from the current source or dependency declarations.
They are `foundation_cloudflare_types`, `foundation_context`, `foundation_core`, `foundation_nativeapis`, `foundation_reactor`, `foundation_theme`, and `foundation_vfs`.
The compared source files match for `foundation_buildtools`, `foundation_errstacks`, and `foundation_nostd`.
The first two public manifests declare an older minimum Rust version.
A coherent registry distribution would require dependency and version changes beyond the missing set.
Versionless path dependencies also block the full crate publication job.
[Crate archive follow-up](https://github.com/ewe-studios/ewe_platform/issues/883)

## Reversible validation

The pinned Rust 1.98.0 cycle check passes after the macro fixture repair.
A normal clone packages all 26 crates against the supported local Cargo patch table.
A fresh generated application resource build succeeds from the normalized `foundation_resources 0.1.3` archive with that local table.
The build stages the generated Android icon, launch theme, and application resources.
The installed generator passes all eight tests, including all seven target selections.
The mixed project generates identical Git release sources for the application, native core, and browser resources.

The clean Git resolver fixture keeps the public Git URL and uses a disposable local mirror.
It replaces the absent public tag with a scratch revision and disables unrelated submodule updates in that mirror.
Both locks contain one `foundation_resources 0.1.3` package.
The native graph uses `build`, `mobile-launch`, and `runtime`.
The browser graph uses `browser-package`, `build`, and `mobile-launch`.
The native core and the resource package share one Git source identity.

The unmodified checkout reaches 9.0 GiB while Cargo fetches nested tool submodules.
The audit stops that checkout before lock resolution and removes its scratch cache.
Cargo also prints a nonfatal diagnostic while it scans the raw PlatformApp template manifest.
These findings remain open. The scratch mirror verifies dependency resolution before any public release.
[Git checkout cost](https://github.com/ewe-studios/ewe_platform/issues/1200) · [Template discovery diagnostic](https://github.com/ewe-studios/ewe_platform/issues/1201)

## Verified 0.1.2 proposal

The release source is `4e4a50b9d230cb536f092f11bd689be60c2bf9d1`.
It includes the shared template source selection and the Android build type staging repair.
The preview builds both starters against staged packages and runs both Todo apps on each platform.
The iPhone suites pass 11 tests and skip six checks that require optional fixtures.
The Swift package suite passes 30 tests.
[Current native preview](https://github.com/ewe-studios/ewe_platform/actions/runs/46079128868)

The unsigned archive digests use the 256-bit Secure Hash Algorithm (SHA-256).

| Archive | SHA-256 |
| --- | --- |
| `EweCoreFFI.xcframework.zip` | `3a851c9b11aa0ae333586e7ee8d751e858358ce1d6af0c60a98b81c1907728d4` |
| `android-publication.zip` | `c4b0c2e284f343be3055b393b269c4ae2e01f76a468da90f53d722f62fa5c5f4` |

The release record gives Swift checksum commit `47f8b98e8439d3de4f26a06bdefef9146e15599e`.
An independent checkout reproduces the commit using the verified Apple archive bytes.
The Apple archive contains iPhone device, simulator, and Mac libraries.
The Android archive contains all three Maven modules and both native architectures.
[Unsigned candidate](https://github.com/ewe-studios/ewe_platform/actions/runs/46079128868/artifacts/15239750961)

The generated Android application packages (APKs) use Maven 0.1.2 from this archive.
Both build orders preserve distinct debug and release Rust libraries in their respective x86-64 APKs.
The generated mixed browser project loads WebAssembly in Chrome and increments its counter.
The scratch Git mirror omits unrelated tool submodules and keeps the package source bytes.
[Build type staging](https://github.com/ewe-studios/ewe_platform/issues/800)

## Historical approval for the verified source

The owner approves release 0.1.2 from the exact source above when the required credentials are ready.
The authenticated environment lookup returns 404 for `native-ui-release`.
The repository lists none of the required credential names or signing variable.
Central Portal access is needed to confirm authority over the Maven namespace.
The release policy does not name an authorized reviewer or team.
Supply that identity before configuring the approval gate.
The owner or release administrator protects the environment and configures these inputs.

| Setting | Permission or input |
| --- | --- |
| `NATIVE_RELEASE_TOKEN` | Create the release assets and Swift version tag. Configure repository Contents write access and any tag protection approval. |
| `NATIVE_GPG_PRIVATE_KEY` | Sign the Maven publication files in the protected job. |
| `NATIVE_GPG_FINGERPRINT` | Select the signing key through the environment variable. |
| `CENTRAL_USERNAME` and `CENTRAL_PASSWORD` | Authenticate to the Central Portal for the verified `io.github.ewe-studios` namespace. |

Start the approved sequence after the required environment and credentials are ready.

1. Create `native-ui-v0.1.2` at the approved source revision. This starts the protected publication job.
2. Let the job publish the native assets and Maven modules at 0.1.2.
3. Let the job create Swift tag `0.1.2` at the recorded checksum commit.
4. Compare the public archive digests and Maven deployment receipt with the release manifest.
5. Run both public starter checks and generate a clean mobile application with only the public inputs.
6. Advance the template defaults after the public checks pass.
[Release procedure](../../../../scripts/ui_native/release/README.md) · [Public consumer issue](https://github.com/ewe-studios/ewe_platform/issues/845)

## Release prerequisites for the updated workflow

The registrar GitHub App replaces the personal release token in the current workflow.
The repository lists both registrar App credential names and the signing fingerprint variable.
The organization stores the Central user token pair. Its signing key secret selects only this repository.
The organization lists `eweplatform-runner-registrar` with Contents write and access to all repositories.
The workflow asks for access to this repository only. Credential values and Central namespace membership still need separate proof.
The `native-ui-release` environment contains `CENTRAL_USERNAME` and `CENTRAL_PASSWORD`.
The environment requires review by `influx6`. GitHub allows the person who starts the run to approve it.
Sonatype lists the Ubuntu and OpenPGP keyservers for its public key lookup.
The initial fingerprint lookups at Ubuntu and OpenPGP return HTTP status 404. The MIT lookup does not respond.
The release start record below verifies public key retrieval after upload.
[Central keyserver requirements](https://central.sonatype.org/publish/requirements/gpg/#distributing-your-public-key)

Build and preview an updated release revision that contains the App token workflow.
Obtain owner approval for that exact revision before the tag push.
The previous approval remains specific to `4e4a50b9d230cb536f092f11bd689be60c2bf9d1`.
A tag at that commit runs its original workflow and requires the original token input.
[Current workflow](../../../../.github/workflows/native-ui-release.yml) · [Publication setup](../../../../bin/ewe_ui/docs/operations/publishing.md)

### Updated 0.1.2 preview

Commit `26315e7ff2063639de10b94051f909d7239c90e4` contains the registrar token workflow.
Its only file difference from the earlier 0.1.2 proposal is the native release workflow.
The release configuration, embedded generator configuration, and native crate all select 0.1.2.
The manual preview passes both the candidate and Android example jobs. GitHub skips publication and both public install jobs.
The Swift package passes all 30 tests. Both iPhone Todo applications pass 11 tests and skip six in total.
The refreshed Maven and GitHub lookups find no public 0.1.2 release.
[Updated native preview](https://github.com/ewe-studios/ewe_platform/actions/runs/46099351076)

The unsigned manifest records Swift checksum commit `ef0272e89c3c6e992017b3cb6e63f50c55fbf36b`.
The downloaded archives match the manifest's 256-bit Secure Hash Algorithm (SHA-256) digests.

| Archive | SHA-256 |
| --- | --- |
| `EweCoreFFI.xcframework.zip` | `f522520c4fa8f2c210695775baa4fa99df53c7997069f114e6f17659f5b9010e` |
| `android-publication.zip` | `bf78ceee6a50e7920a867410af73a2c8377e8dc0dd2a4454ea0835e7b2c84f98` |

This evidence applies to the exact preview source. Owner approval for that source remains separate from GitHub environment review.
[Unsigned candidate](https://github.com/ewe-studios/ewe_platform/actions/runs/46099351076/artifacts/15247740386)

### Integration with master

Commit `b00b5e70c99f433822af9e872e454e5da394e6e7` merges master `f5e69eef72ddee9dee9049efa24964bddde23f24` into the candidate on a separate branch.
The 0.1.2 package coordinates and native manifest match the preview source.
The resolved source passes workflow lint and locked Cargo metadata resolution without network access.
The integrated preview passes both jobs. The Swift package passes 30 tests, and the iPhone Todo suites pass 11 tests with six skips.
The Android job builds and exercises the two example applications.
GitHub skips publication and the two public install jobs.
[Integration branch](https://github.com/ewe-studios/ewe_platform/tree/integrate/native-ui-0.1.2-app-token-master) · [Integrated preview](https://github.com/ewe-studios/ewe_platform/actions/runs/46106592555)

The unsigned manifest identifies the integrated commit and version 0.1.2.
It records Swift checksum commit `4c3d9fb38f812f8cd330c39e84d86ca4bb27d269`.
Both downloaded archives match the manifest digests.

| Archive | SHA-256 |
| --- | --- |
| `EweCoreFFI.xcframework.zip` | `3cc972369ddce78b1f31684be0f0e902facfc1b9e8d6d5595ad4b074dd75cc66` |
| `android-publication.zip` | `28099ac9a8ef72bf9baa9c29302a30da196ef08ee24124104c2b196d5f50b9a9` |

The owner approves the integrated revision after public key verification. See the release start record below.
[Integrated unsigned candidate](https://github.com/ewe-studios/ewe_platform/actions/runs/46106592555/artifacts/15250267442)

### Approved 0.1.2 release start

The owner approves native release 0.1.2 at integrated commit `b00b5e70c99f433822af9e872e454e5da394e6e7` after public key verification.
GNU Privacy Guard (GnuPG) verifies that the export contains only public key packets.
It uploads the OpenPGP certificate to the Ubuntu keyserver over the secure HTTP Keyserver Protocol (HKPS).
A separate empty keyring retrieves the certificate and confirms fingerprint `9650A201269A0768F5D34DCC03A01ECE6BC5C71F`.
The remote `native-ui-v0.1.2` tag resolves to the approved commit.
The tag run repeats the candidate checks before the publication job requests environment approval.
[Tag run](https://github.com/ewe-studios/ewe_platform/actions/runs/46121293056)
