---
feature: "Install a pinned iOS SDK in the macOS platform image"
status: "pending"
priority: "high"
depends_on: ["09-ios-wasmtime-gating"]
crates: []
issues: [355]
model: "haiku"
branch: "spec73/f10-macos-vm-ios-sdk"
---

# Feature 10: macOS VM iOS SDK

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Software Development Kit (SDK); Xcode Command Line Tools
(CLT); Continuous Integration (CI); virtual machine (VM); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Close [#355](https://github.com/ewe-studios/ewe_platform/issues/355): the macOS platform VM
cannot build for an iOS target because the image has no iOS SDK.

## 2. Problem

`ci/platform-images/images/macos/guest/provision.sh:57-68` installs the host toolchain and
Rust targets, but not the iOS SDK. Xcode ships the iOS SDK, not the CLT. Without it,
`cargo check --target aarch64-apple-ios` fails at link or at SDK lookup.

The workflow that drives the VM is `.github/workflows/platform-vms.yaml` (the `macos-ssh`
job). Measure on master: read `provision.sh` and the workflow. Record the exact Xcode
version the image has, if any.

## 3. Requirements

1. The image has an iOS SDK of a stated, pinned Xcode version.
2. The pin is in one place. The provisioning script and the documentation read the same
   value.
3. Provisioning checks for the SDK and fails with a clear message when it is missing. It does
   not continue silently.
4. The iOS `cargo check` from feature 09 runs in CI on this image.
5. State the image size and the licence position for the Xcode download in the docs. Apple's
   licence terms govern the download; state that a CI runner with a licensed Xcode is
   required.

## 4. Design

Complete before handoff:

1. Read `provision.sh` and `platform-vms.yaml`. Name the install step and where the iOS SDK
   step goes.
2. Choose the install path: a downloaded and pinned Xcode (`xip`), an already-installed Xcode
   selected with `xcode-select`, or `xcodebuild -downloadPlatform iOS`. State why, and state
   the licence consequence.
3. Name the version variable and both files that read it.
4. Name the CI job and its command.

## 5. Tasks

- [ ] Verify the failure on the current image. Record the output.
- [ ] Fill section 4.
- [ ] Update `provision.sh`.
- [ ] Update the workflow.
- [ ] Update the documentation.
- [ ] Run the provisioning steps on the image, or record the CI run.
- [ ] Close #355 in the PR body.

## 6. Tests

The test is a CI run of the workflow, recorded in the PR: the `macos-ssh` job builds for
`aarch64-apple-ios` and passes. A dry-run check in `provision.sh`
(`xcodebuild -showsdks | grep iphoneos`) fails with a clear message when the SDK is absent.

## 7. Verify

```bash
# in the macOS VM after provisioning
xcodebuild -showsdks
rustup target add aarch64-apple-ios
cargo check -p foundation_platform --target aarch64-apple-ios
```

## 8. Done when

- The image has a pinned iOS SDK.
- Provisioning fails loudly without it.
- The CI job builds for iOS.
- #355 is closed by the PR.
