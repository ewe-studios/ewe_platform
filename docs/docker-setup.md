# Docker Development Environment

Docker-based reproducible dev environment for cross-platform ewe_platform
development (spec-52 F29 Stage 1).

## Prerequisites

- Docker Engine 24+ with Compose v2
- KVM enabled (`ls -1 /dev/kvm` must exist)
- 32 GB+ free disk space (macOS base image is ~25 GB)
- 16 GB+ RAM recommended (8 GB minimum for macOS alone)

## Quick Start

```bash
# Full-stack: macOS + Android
make dev

# iOS development only (macOS with Xcode)
make ios

# Android development only
make android

# Stop everything
make docker-stop

# Tear down completely (removes volumes)
make docker-clean
```

## Services

### macOS (`dockurr/macos`)

| Property | Value |
|----------|-------|
| Version | macOS Sequoia 15 |
| RAM | 8 GB |
| CPU cores | 4 |
| Disk | 64 GB |
| VNC | `localhost:5900` |
| Web UI | `localhost:8006` |

First boot: 10-15 minutes. The base image downloads and installs macOS.
Subsequent boots: 2-5 minutes.

**What's pre-installed:**
- Xcode 16+ (for iOS simulator builds)
- Homebrew (for `cargo-tauri` and toolchain deps)
- Rust toolchain (`rustup`, `cargo`)

**File sharing:** The repo is mounted at `/shared` inside the container.
Files written to `/shared` on macOS appear in your host repo directory.

### Android Emulator (`dockurr/android`)

| Property | Value |
|----------|-------|
| Version | Android 14.0 |
| RAM | 4 GB |
| CPU cores | 2 |
| Web UI | `localhost:8007` |

First boot: 5-8 minutes. Includes Android SDK + emulator image download.
Subsequent boots: 1-2 minutes.

**File sharing:** `./android_shared/` is mounted at `/shared` inside the container.
Push APKs via ADB: `adb connect localhost:5555 && adb install app.apk`.

## Workflows

### iOS Simulator Testing

1. `make ios` — boot macOS VM
2. Connect via VNC (`localhost:5900`)
3. Inside macOS: `cd /shared && cargo tauri ios build`
4. Open Xcode, select simulator, run

### Android Emulator Testing

1. `make android` — boot Android emulator
2. Open `localhost:8007` in browser (web-based VNC viewer)
3. Build: `cargo tauri android build --debug`
4. Install: `adb connect localhost:5555 && adb install <apk>`

### CI/Headless

For headless environments, omit VNC/web UI access and use ADB/Xcode
command-line tools:

```bash
# Android (headless)
docker compose --profile android up android -d
adb connect localhost:5555
adb install app-debug.apk
adb shell am start -n com.ewe.platform/.MainActivity
adb shell input tap 500 500  # simulate touch

# iOS (headless — requires VNC for full Xcode automation)
# Use xcodebuild CLI inside macOS container:
docker compose exec macos xcodebuild test -project ...
```

## Troubleshooting

### KVM not available
```bash
# Check:
ls -1 /dev/kvm
# If missing, enable virtualization in BIOS/UEFI.
# On Linux: modprobe kvm && modprobe kvm_intel (or kvm_amd)
```

### macOS won't boot
- Check disk space: `df -h` — need 25 GB+ free
- Increase RAM: set `RAM_SIZE: "12G"` in `docker-compose.yaml`
- First boot is slow — wait 10-15 minutes, check VNC for progress bar

### Android emulator is slow
- Ensure KVM is available (`/dev/kvm`)
- Increase CPU cores: set `CPU_CORES: "4"` in `docker-compose.yaml`
- Use x86_64 system image (default in dockurr/android)
