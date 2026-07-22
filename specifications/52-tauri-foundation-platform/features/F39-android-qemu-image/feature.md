---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F39-android-qemu-image"
this_file: "specifications/52-tauri-foundation-platform/features/F39-android-qemu-image/feature.md"

status: completed
priority: critical
created: 2026-07-22

depends_on:
  - "F30-docker-test-infra"
  - "F32-docker-agentic-control"

tasks:
  completed: 12
  uncompleted: 0
  total: 12
  completion_percentage: 100%
---

# F39 — Android QEMU Docker image (infrastructure/docker/android)

## Problem

The current Android test image (`artefacts/dockerfiles/android/Dockerfile`, 128
lines, built from `ubuntu:24.04`) is architecturally wrong:

1. **Doesn't build on our QEMU base** — macOS, Windows, and ChromeOS all use
   `COPY --from=ewestudios/qemu:7.37 / /`. Android doesn't. It duplicates
   package lists, misses QMP, hooks, socat, and imagemagick.
2. **No visual display** — uses `qemu-system-x86_64-headless` (SDK-bundled
   emulator binary) which has NO VNC support. Multiple attempts to add VNC
   failed (`-gpu guest` crashes, `-engine classic` not installed).
3. **Custom Python HTTP server** — hacked in to serve ADB screencap PNGs
   because VNC didn't work. This is not durable.
4. **Duplicates infrastructure** — `artefacts/dockerfiles/android/` is
   separate from `infrastructure/docker/`. All other platforms live under
   `infrastructure/docker/`.

## Solution: Follow the macOS/Windows pattern

Build `infrastructure/docker/android/` on the QEMU base image, exactly
like macOS and Windows. The Android emulator bundles its own QEMU, but
we can use Xvfb + x11vnc + openbox (the budtmo/docker-android pattern)
to capture the emulator's display to VNC.

```
infrastructure/docker/qemu/          ← base (debian, qemu-system-x86, VNC, QMP, hooks)
infrastructure/docker/macos/         ← FROM scratch + COPY base + macOS install scripts
infrastructure/docker/windows/       ← FROM scratch + COPY base + Windows install scripts
infrastructure/docker/android/       ← FROM scratch + COPY base + Android SDK + emulator ← NEW
```

### Architecture

```
┌──────────────────────────────────────────────┐
│  infrastructure/docker/android/Dockerfile     │
│  FROM scratch                                 │
│  COPY --from=ewestudios/qemu:7.37 / /        │
│  RUN install Android SDK + emulator + AVD    │
│  COPY src/entry.sh /run/                     │
│  ENTRYPOINT entry.sh                          │
└──────────────────┬───────────────────────────┘
                   │
     ┌─────────────▼──────────────┐
     │  entry.sh                  │
     │  1. Start Xvfb :99         │  ← emulator renders into virtual X server
     │  2. Start openbox (WM)     │
     │  3. Start x11vnc on :99    │  ← VNC captures the X display
     │  4. Start noVNC/websockify │  ← browser access at :6081
     │  5. Start ADB server       │
     │  6. Start emulator -avd    │  ← emulator uses DISPLAY=:99
     │  7. Wait for boot          │
     │  8. Serve forever          │
     └────────────────────────────┘
```

### Why Xvfb + x11vnc works

The Android emulator binary (`$ANDROID_HOME/emulator/emulator`) renders to an
X11 display. By setting `DISPLAY=:99` and running Xvfb + x11vnc, we capture
the emulator's framebuffer without needing QEMU VNC support. This is the
exact pattern used by budtmo/docker-android (the most popular Android Docker
image, 5M+ pulls).

Our QEMU base already includes `qemu-system-x86` (full, not headless), so
the emulator's QEMU binary coexists — it handles the guest VM, while
Xvfb/x11vnc handle the display capture.

## Docker image structure

```
infrastructure/docker/android/
├── Dockerfile          ← FROM scratch, COPY qemu base, install Android SDK
├── compose.yml         ← docker compose service definition
├── src/
│   └── entry.sh        ← boot sequence (Xvfb, x11vnc, emulator, ADB)
├── hooks/              ← QEMU hook overrides (20-agentic-control.sh etc.)
└── assets/             ← emulator config, ADB key pre-auth
```

## Docker Compose

```yaml
android:
  image: ewestudios/android:latest
  container_name: ewe_android
  environment:
    ANDROID_VERSION: "14.0"
    RAM_SIZE: "4G"
    CPU_CORES: "2"
  devices:
    - /dev/kvm
  ports:
    - "5901:5900"      # VNC (x11vnc)
    - "6081:6080"      # noVNC web viewer
    - "5554:5554"      # emulator ADB
    - "5555:5555"      # second emulator
  volumes:
    - ./android_shared:/shared
  profiles: [android]
```

## Requirements

### R1. Dockerfile — build from QEMU base
- `FROM scratch` + `COPY --from=ewestudios/qemu:7.37 / /`
- Install Android SDK command-line tools
- Install SDK packages: platform-tools, build-tools, platforms;android-34,
  emulator, system-images;android-34;default;x86_64
- Install x11vnc, xvfb, openbox, xterm (display stack)
- Pre-create AVD: `avdmanager create avd -n test_avd ...`
- JAVA_HOME: OpenJDK 17

### R2. entry.sh — boot sequence
1. SSH setup (key injection)
2. Xvfb :99 + openbox (window manager)
3. x11vnc on :99 → port 5900
4. noVNC/websockify 6080 → 5900
5. ADB server start
6. emulator -avd test_avd -no-window -gpu swiftshader_indirect with DISPLAY=:99
7. Wait for `sys.boot_completed == 1`
8. Print ready message + ports
9. `tail -f /dev/null`

### R3. No custom HTTP servers
- Use x11vnc for VNC (standard, well-tested)
- Use novnc for web viewer (included in QEMU base)
- No Python HTTP servers, no ADB screencap polling
- User connects via VNC client or browser at :6081

### R4. ADB + port exposure
- ADB server on 5037 (internal)
- Emulator on 5554, 5555
- Host: adb connect localhost:5554
- Screenshots: adb exec-out screencap -p (for CI/test assertions, not for display)

### R5. Hooks inheritance
- Inherits QEMU hooks from base: 20-agentic-control.sh
- Can override with own hooks/ directory
- QMP socket available at /run/shm/qmp.sock (from base)

### R6. Agentic control (F32)
- ADB: input tap, input keyevent, input text
- ADB: screencap for CI assertions
- QMP: available but emulator is not QEMU-managed (emulator manages QEMU internally)
- SSH: for test command dispatch

### R7. Deprecate artefacts/dockerfiles/android
- Mark as deprecated with pointer to infrastructure/docker/android
- Remove the 128-line standalone Dockerfile
- Keep entrypoint.sh as reference for the boot sequence

### R8. Image name + tags
- `ewestudios/android:latest` (Docker Hub)
- `ewe-android:latest` (local development)
- Built from `infrastructure/docker/android/`

### R9. Testing
```bash
# Build
docker build -t ewestudios/android:latest infrastructure/docker/android/

# Start
docker compose --profile android up -d

# Visual verification
open http://localhost:6081/vnc.html

# ADB
adb connect localhost:5554
adb shell input tap 500 500
adb exec-out screencap -p > screenshot.png

# Install APK
adb install app-debug.apk
adb shell am start -n com.ewe.platform/.MainActivity
```

### R10. CI integration
- GitHub Actions: build image, boot emulator, wait for boot, verify ADB,
  install APK, take screenshot, compare to baseline

## Files

| File | Action |
|------|--------|
| `infrastructure/docker/android/Dockerfile` | **NEW** — build from QEMU base + Android SDK |
| `infrastructure/docker/android/src/entry.sh` | **NEW** — Xvfb + x11vnc + emulator boot |
| `infrastructure/docker/android/compose.yml` | **NEW** — service definition |
| `infrastructure/docker/android/hooks/` | **NEW** — QEMU hook overrides |
| `artefacts/dockerfiles/android/` | **DEPRECATE** — add README pointing to new location |
| `docker-compose.yaml` | Update android service to use ewestudios/android |
