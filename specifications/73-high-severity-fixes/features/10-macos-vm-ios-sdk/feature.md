---
feature: "Install a pinned iOS SDK in the macOS platform image"
status: "in_progress"
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

## 4. Design (completed 2026-11-07)

### 4.1 Measured state

1. `ci/platform-images/images/macos/guest/provision.sh` installs only the Xcode Command Line
   Tools (lines 60-74). It never installs Xcode and never installs a platform SDK.
2. `.github/workflows/platform-vms.yaml:115-121` (`macos-ssh`) already looks for the SDK:
   it reads `xcode-select -p`, falls back to `/Applications/Xcode.app/Contents/Developer`,
   and exits with `error: install full Xcode on the VM disk; the iOS SDK is missing`. So the
   defect is the image, not the workflow: the job fails on every run.
3. `components.env` holds every pin the image uses. The file is plain `KEY=value` lines, so
   both `provision.sh` and a workflow step can source it.

### 4.2 Install path

Use a full Xcode that is already on the VM disk, and install the iOS platform into it with
Xcode's own downloader:

```bash
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
xcodebuild -downloadPlatform iOS -buildVersion "$IOS_SDK_VERSION"
```

Why this path:
- The Command Line Tools do not ship any platform SDK. A downloaded `.xip` Xcode is the only
  other source, and it needs an Apple developer account to download. That download cannot be
  automated. The provisioning script already assumes a one-time manual install by the admin
  user (its header says so), so "Xcode is already on the disk" fits the image model.
- `-downloadPlatform` takes the version as an argument, so the pin is a real pin.
- It needs no Apple account.

Licence position, to state in the docs: Apple's licence for Xcode and the iOS SDK allows use
only on Apple-branded hardware. The CI host is Apple hardware (`apple-hardware` label). The
same licence applies to the SDK download. State it in the macOS image README.

Disk: full Xcode is about 15 GB; the iOS platform adds about 8 GB. The VM disk must reserve
that space. State the number in the README and note the `macos-storage` input.

### 4.3 The pin

`components.env` gets one line:

```
IOS_SDK_VERSION=<version, measured on the image>
```

Measure it: if the image is reachable, read `xcodebuild -version` and pick the matching
platform version. If the image is not reachable, pin the platform version of the Xcode that
the README already names, and state that the pin is unverified until the CI run.

Two readers, one value:
1. `provision.sh` sources `components.env` already, so it reads `$IOS_SDK_VERSION`.
2. The `macos-ssh` job sources the same file before it reads the value:
   `. ci/platform-images/images/macos/guest/components.env`.

### 4.4 The provisioning step

Add a block after the Command Line Tools block:

1. `[ -d /Applications/Xcode.app ] || fail "full Xcode is missing; install the pinned Xcode
   once by hand (see the README)"`.
2. Select it: `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`.
3. Install the platform: `xcodebuild -downloadPlatform iOS -buildVersion "$IOS_SDK_VERSION"`.
4. Verify: `xcodebuild -showsdks | grep -q "iphoneos$IOS_SDK_VERSION"` or `fail`.
5. Add `xcodebuild -version` and `xcodebuild -showsdks` to `$STATUS/versions.txt`.
6. Write `$STATUS/ios-sdk.txt` with the `DEVELOPER_DIR` path and the version, so the workflow
   reads the provisioned value and not a guess.

### 4.5 The workflow change

In the `macos-ssh` job `run` block (`.github/workflows/platform-vms.yaml:105-125`):
1. Replace the two-line SDK hunt with the values from `$STATUS/ios-sdk.txt`:
   `export DEVELOPER_DIR=$(sed -n 's/^developer_dir=//p' .../ios-sdk.txt)`.
2. Keep the existing explicit failure if the file is absent.
3. Add the feature 09 proof after the existing iOS check:
   `cargo check -p foundation_platform --target aarch64-apple-ios --profile uat`.

### 4.6 Documentation

`ci/platform-images/README.md` is the image documentation. It already has a macOS table
under "What gets installed in the VM" (line 359-366) and a licensing section (line 460).
Add:
1. One row or sentence: full Xcode and the iOS platform are part of the one-time manual
   install, and `provision.sh` installs and verifies the platform from the pinned version.
2. In the licensing section: the iOS SDK follows the same Apple EULA rule as macOS, so it
   stays inside the private VM disk.
3. The disk space: Xcode about 15 GB and the iOS platform about 8 GB. Name the
   `macos-storage` input.

## 5. Tasks

- [ ] Verify the failure on the current image. Record the output. (Waits for a
  VM run: this host cannot boot the macOS VM. The old workflow failed with
  `error: install full Xcode on the VM disk; the iOS SDK is missing`.)
- [x] Fill section 4.
- [x] Update `provision.sh`.
- [x] Update the workflow.
- [x] Update the documentation.
- [ ] Run the provisioning steps on the image, or record the CI run. (Waits for
  the first `macos-ssh` run on the `apple-hardware` host.)
- [ ] Close #355 in the PR body.

**Waiting for the VM run.** The pin `IOS_SDK_VERSION=26.2` matches Xcode 26.3,
the newest Xcode that runs on the macOS 15 guest (the image sets `VERSION="15"`).
It is unverified on the VM until the first CI run. That run also proves
`xcodebuild -downloadPlatform`, the `iphoneos26.2` check and
`cargo check -p foundation_platform --target aarch64-apple-ios --profile uat`.
Local checks pass: `bash -n`, the PyYAML parse and `actionlint`; `shellcheck`
is not installed on this host.

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

## 9. Result (2026-11-07)

Commit `a8e9b73b9`. The pin is `IOS_SDK_VERSION=26.2` in `components.env`, with Xcode 26.3 as
the one-time manual install. Xcode 26.3 is the newest Xcode that runs on the macOS 15 guest
(the Dockerfile sets `VERSION="15"`). `provision.sh` selects Xcode, installs the platform,
verifies `iphoneos26.2`, and writes `status/ios-sdk.txt`. The `macos-ssh` job sources the pin,
compares it with the provisioned file, exports `DEVELOPER_DIR`, and runs the feature 09
`cargo check`. Local checks pass: `bash -n`, PyYAML, `actionlint` (exit 0).

The pin and the CI proof wait for the first `macos-ssh` run on the `apple-hardware` host. The
issue stays open until that run.

Two defects were filed from this work: #373 (medium, the macOS 15 guest caps the iOS SDK at
Xcode 26.3) and #374 (low, the README provisioning estimate omits the 8 GB platform download).
