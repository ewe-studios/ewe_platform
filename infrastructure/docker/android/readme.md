# ewestudios/android — Android emulator on QEMU

Built on `ewestudios/qemu:7.37` (same base as macOS, Windows, ChromeOS).
Provides a full Android emulator with VNC + noVNC visual access, ADB, and
agentic control.

## Quick Start

```bash
docker compose --profile android up -d
# Open http://localhost:6081/vnc.html
# or vnc://localhost:5901 (port 5900 inside → 5901 on host)
```

## Architecture

```
ewestudios/qemu:7.37        ← base (full QEMU, VNC, QMP, hooks, socat, imagemagick)
        │
ewestudios/android:latest   ← FROM scratch + COPY base
        │                     + Xvfb + openbox + x11vnc + websockify (display stack)
        │                     + Android SDK 34 + NDK 26.1 + emulator
        │                     + Full QEMU symlinked over headless SDK binary
        │
ewe_android container       ← Xvfb :99 → emulator renders to X11
                              → x11vnc captures :99 on port 5900
                              → websockify proxies to browser on port 6080
```

## Device Selection

4 pre-built AVDs. Select via `DEVICE` env var:

| DEVICE      | Type   | SDK Device    | Resolution |
|-------------|--------|---------------|------------|
| `pixel_6`   | Phone  | pixel_6       | 1080×2400  |
| `nexus_5`   | Phone  | Nexus 5       | 1080×1920  |
| `pixel_c`   | Tablet | pixel_c       | 2560×1800  |
| `nexus_7`   | Tablet | Nexus 7       | 1200×1920  |

Default: `pixel_6`.

```yaml
# docker-compose.yml
environment:
  DEVICE: "nexus_5"     # switch to Nexus 5 phone
  # DEVICE: "pixel_c"   # switch to Pixel C tablet
```

Or via `docker run`:
```bash
docker run -e DEVICE=nexus_5 -e RAM_SIZE=4G -e CPU_CORES=2 \
  --device /dev/kvm \
  -p 5901:5900 -p 6081:6080 -p 5554:5554 \
  ewestudios/android:latest
```

## Ports

| Host Port | Container Port | Service |
|-----------|---------------|---------|
| 5901      | 5900          | x11vnc (raw VNC) |
| 6081      | 6080          | noVNC (web viewer) |
| 5554      | 5554          | ADB emulator console |
| 5555      | 5555          | ADB second instance |

## Agentic Control (F32)

```bash
# Keyboard
adb connect localhost:5554
adb shell input keyevent KEYCODE_HOME
adb shell input text "hello"

# Touch
adb shell input tap 500 500
adb shell input swipe 500 500 500 1000

# Screenshot
adb exec-out screencap -p > screen.png

# App management
adb install app.apk
adb shell am start -n com.example.app/.MainActivity
```

## Environment Variables

| Variable          | Default    | Description |
|-------------------|-----------|-------------|
| `DEVICE`          | `pixel_6` | AVD to boot (`pixel_6`, `nexus_5`, `pixel_c`, `nexus_7`) |
| `RAM_SIZE`        | `4G`      | Emulator RAM |
| `CPU_CORES`       | `2`       | Emulator vCPUs |
| `DISK_SIZE`       | `8G`      | VM disk size |
| `ANDROID_VERSION` | `14`      | Android API level label |
| `ANDROID_EMULATOR_EXTRA_ARGS` | (empty) | Extra emulator CLI flags |

## Building

```bash
# Build the image (takes ~5 min, mostly Android SDK download)
docker build -t ewestudios/android:latest infrastructure/docker/android/

# Rebuild without cache
docker build --no-cache -t ewestudios/android:latest infrastructure/docker/android/
```

## Files

```
infrastructure/docker/android/
├── Dockerfile                    ← FROM scratch + COPY base + SDK + QEMU swap
├── readme.md                     ← This file
├── src/
│   ├── entry.sh                  ← Boot: Xvfb → x11vnc → emulator → ADB
│   └── create-avds.sh            ← Build-time AVD creation (4 devices)
└── hooks/
    └── 20-agentic-control.sh     ← QEMU hook override (ADB info)
```
