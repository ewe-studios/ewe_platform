# Platform Test Environments

Docker images for cross-platform testing of `foundation_platform`. Each image
provides a complete test environment with the target platform's toolchain, an
interactive desktop via VNC, and SSH for test command dispatch.

## Images

| Directory | Image | Base | Platform |
|---|---|---|---|
| `linux/` | `ewe-test-linux` | `ubuntu:24.04` | Linux desktop (X11 + GTK + WebKitGTK) |
| `android/` | `ewe-test-android` | `ubuntu:24.04` + Android SDK | Android emulator (x86_64, KVM) |
| `windows/` | `ewe-test-windows` | `ewestudios/windows` (dockurr) | Windows 11 in QEMU |

## Usage

```bash
# Start a test environment
docker compose -f linux/compose.yaml up -d

# Connect via VNC (native client)
vnc://localhost:5900   # linux
vnc://localhost:5901   # android
vnc://localhost:8006   # windows (QEMU VNC)

# Or via web browser (noVNC)
http://localhost:6080/vnc.html  # linux
http://localhost:6081/vnc.html  # android

# Run tests
cargo platform-test --target linux
cargo platform-test --target android
cargo platform-test --target windows
```

## Design

- **Linux + Android**: Native Docker containers. No VM — X11 desktop + VNC.
  Android emulator uses the host's KVM for hardware acceleration.
- **Windows**: `ewestudios/windows` (copied from `dockurr/windows`). Windows 11
  runs inside QEMU. VNC is provided by QEMU. First-boot provisioning is done
  via `/oem/install.bat`.

## Docker Hub

dockurr base images are mirrored to `ewestudios` Docker Hub:
- `ewestudios/windows:latest` ← `dockurr/windows:latest`
- `ewestudios/macos:latest` ← `dockurr/macos:latest`

These are pulled in CI. Rebuild only when Dockerfiles change.

## Learnings

Based on:
- `artefacts/uncloud/recipes/dev-linux/` — dev container patterns (mise, SSH, slim base)
- `artefacts/uncloud/recipes/dev-windows/` — dockurr Windows provisioning (/oem)
- `@formulas/src.rust/src.Dockurr/` — upstream dockurr base images
