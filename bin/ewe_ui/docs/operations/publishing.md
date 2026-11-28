# Publish Ewe user interface packages

Use the native workflow for the Swift and Android framework packages.
Use the browser workflow for the npm runtime package.
Follow the application build manual to create an Android App Bundle. [Building](building.md)

## Preview the native release

A candidate contains the package archives and a record of their source revisions and digests.
Use `release/native-ui.toml` from the proposed release revision to identify the version, packages, and target architectures.
Check that the Apple archive address and the Rust and Gradle release versions match that version.
Use the exact revision approved for release. The branch that supplies the command manuals can have a different package version.
[Release inputs](../../../../release/native-ui.toml) · [Proposed version and source](../../../../specifications/69-foundation-ui-v2/features/19-native-package-releases/release-prerequisites.md)

1. Push the proposed revision to a branch that can run the **Native UI packages** workflow.
2. Run the workflow manually against that branch and confirm the run uses the proposed revision.
3. Check the `preview` and `android-examples` results. Save the `native-candidate` artifact.
4. Review `native-ui.json` and confirm the version, source revision, Swift checksum commit, and archive digests.

The preview verifies native bindings and builds the Swift and Android starters with staged packages.
It also tests both Todo applications on each mobile platform.
To prepare and verify a candidate locally on macOS, use the checkout tools in the native release procedure.
[Native workflow](../../../../.github/workflows/native-ui-release.yml) · [Local candidate procedure](../../../../scripts/ui_native/release/README.md)

## Set up native publication

Configure the repository's `native-ui-release` environment and its required reviewers before starting publication.
Use an owner-approved reviewer. Confirm access to the `io.github.ewe-studios` namespace in the Central Portal.
Check the organization namespace in the Central Portal. GitHub sign-in automatically verifies only the personal namespace.
If the organization namespace is unverified, ask its administrator to start verification.
For code-hosting verification, create the temporary public repository named by the Portal's verification key under the organization.
If the namespace is already verified, ask its administrator for publishing access.
Generate the user token from an account with that access. Use its generated username and password as the two Central secrets.
[Central namespaces](https://central.sonatype.com/publishing/namespaces) · [Namespace verification](https://central.sonatype.org/register/namespace/#github-namespaces) · [Central user tokens](https://central.sonatype.com/usertoken) · [Token instructions](https://central.sonatype.org/publish/generate-portal-token/)

Use the registrar GitHub App that the Rust crate release workflow uses.
Confirm that its installation includes `ewe-studios/ewe_platform` and grants Contents write permission.
Confirm that its identity can create the release assets and the Swift tag under the repository rules.
Configure the following inputs in GitHub Actions.
[GitHub App installation tokens](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app) · [Repository environments](https://github.com/ewe-studios/ewe_platform/settings/environments) · [Environment setup](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments)

| Name | Location | Purpose |
| --- | --- | --- |
| `NATIVE_GPG_PRIVATE_KEY` | Organization secret with access for `ewe-studios/ewe_platform` | Supply the OpenPGP private key to sign the Maven files. |
| `NATIVE_GPG_FINGERPRINT` | Repository variable | Select the signing key by its full fingerprint. |
| `EWE_RUNNER_REGISTRA_CLIENT_ID`, `EWE_RUNNER_REGISTRA_PRIVATE_KEY` | Repository secrets | Create short-lived registrar App tokens with Contents write for this repository. |
| `CENTRAL_USERNAME`, `CENTRAL_PASSWORD` | Organization secrets available to the repository | Supply a Central Portal user token for the verified Maven namespace. |

Set the signing secret from the armored private-key export. Put the matching public key's fingerprint in the variable.
Keep the private key backup in approved private storage. Only the public key goes to a keyserver.
Upload the public key to a keyserver that Maven Central supports. Confirm it can be retrieved by its full fingerprint.
Run these commands after you replace `FINGERPRINT` with that fingerprint.

```sh
gpg --keyserver keyserver.ubuntu.com --send-keys FINGERPRINT
gpg --keyserver keyserver.ubuntu.com --recv-keys FINGERPRINT
```

The job imports the key into a temporary keyring and exports the public key for signature verification.
The job creates a token before signing and a fresh token before publication. Each token expires after one hour.
The signing task needs push access to see draft releases and recover assets on retries.
[Draft release visibility](https://docs.github.com/en/rest/releases/releases#list-releases)
The publication task gives Maven Central 30 minutes to report publication by default.
If token creation fails, the job stops. After an interruption, retry with the verified candidate and the recorded deployment.
[Central public key requirements](https://central.sonatype.org/publish/requirements/gpg/#distributing-your-public-key) · [Workflow settings](../../../../.github/workflows/native-ui-release.yml) · [Signing task](../../../../backends/foundation_ui_native/src/tasks/sign.rs)

## Tag and publish the native release

1. Obtain the owner's approval for the exact version and commit that passed preview with this workflow.
2. Push `native-ui-vX.Y.Z` at that commit. Replace `X.Y.Z` with the version in the approved release configuration.
3. Confirm that the tag run passes both the `preview` and `android-examples` jobs.
4. Have the designated reviewer approve the `native-ui-release` environment deployment.
5. Check the signing and publication results. Keep the `signed-native-packages-<run-attempt>` artifact.

The tag starts the publication workflow. It runs the version check before preparing the candidate.
The protected job publishes the GitHub release assets and the three Maven modules.
It creates the plain Swift version tag at the checksum commit recorded in the candidate.
Keep the release tag and candidate files for a retry. The publish task checks existing assets and resumes a recorded Central deployment.
If an upload result is uncertain, follow the receipt procedure before you retry.
[Native workflow](../../../../.github/workflows/native-ui-release.yml) · [Resume and receipt procedure](../../../../scripts/ui_native/release/README.md)

## Verify the public native packages

1. Check that the GitHub release has the Apple archive, Android publication archive, and `native-ui.json`.
2. Compare both public archive digests with the public release record. Check the Central deployment receipt.
3. Confirm that the Swift version tag resolves to the recorded checksum commit.
4. Check that `public-install` resolves both package sets and runs the iPhone starter.
5. Check that `android-device` runs the Android starter against the published Maven modules.
6. Before changing generator defaults, build a fresh generated mobile project with only the public package inputs.

The public starter tests add a Todo through the server and verify the resulting count on the test device.
The native release procedure shows how to run the install checks again.
[Public workflow jobs](../../../../.github/workflows/native-ui-release.yml) · [Public starter checks](../../../../scripts/ui_native/release/README.md)

## Publish the browser runtime

The npm package is `@ewe-studios/foundation-wasm-ui`.
Its version matches the Rust `foundation_wasm_ui` crate version.
To make a local archive without publishing, run the pack task from the checkout.

```sh
ewe_ui wasm-bundle npm-pack --output target/browser-runtime-preview
```

1. Review the archive and approve the release commit and crate version.
2. Push a `foundation_wasm_ui-vX.Y.Z` tag at that commit. Use the crate version for `X.Y.Z`.
3. Configure the workflow's `NPM_TOKEN` secret with permission to publish the package.
4. Dispatch **Release the browser runtime** against the tag.
5. Confirm the external consumer and local registry checks pass before the workflow publishes.
6. Confirm the publish job succeeds and compare the package's registry integrity with the verified archive.

The workflow packs the selected tag, records its commit and integrity, and passes the tested archive to publication.
[Browser workflow](../../../../.github/workflows/browser-runtime-release.yaml) · [Browser runtime consumer guide](../../../../backends/foundation_wasm_ui/npm/README.md) · [Pack and publish checks](../../../../backends/foundation_wasm_ui/src/tasks/npm_release.rs)
