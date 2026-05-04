# testbed examples

Self-contained example projects that can be mounted into testbed VMs for E2E testing.

## tauri-app

Minimal Tauri application used by the `foundation_testbed` E2E test suite.
This crate is excluded from the workspace (see root `Cargo.toml`) so it can
be built independently inside a VM with its own dependency resolution.

### Usage

The E2E test mounts this directory into the VM via 9p/virtio-fs and runs
`cargo tauri build` inside the guest. No manual build is needed on the host.

To use a pre-bootstrapped VM image so the test skips the 10+ minute bootstrap:

```bash
TESTBED_IMAGE_WINDOWS_BUILD=/home/darkvoid/EweStore/Testbed/windows-11-bootstrapped.qcow2 \
  cargo test -p foundation_testbed --test e2e_tauri test_tauri_build_windows -- --ignored
```
