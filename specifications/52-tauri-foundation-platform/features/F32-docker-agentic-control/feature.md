---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F32-docker-agentic-control"
this_file: "specifications/52-tauri-foundation-platform/features/F32-docker-agentic-control/feature.md"

status: pending
priority: critical
created: 2026-07-21

depends_on:
  - "F30-docker-test-infra"
  - "F14-android-example"
  - "F15-ios-example"

tasks:
  completed: 0
  uncompleted: 24
  total: 24
  completion_percentage: 0%
---

# F32 — Docker Agentic Control: mouse, keyboard, display

## Problem

The Docker test infrastructure (F30) can shell-exec commands inside containers
(`docker exec`), but cannot control the OS visually: no mouse movement, no
keyboard typing, no screenshot capture of the actual desktop. Agentic
engineering (letting an AI drive the OS like a human) requires these
primitives on every platform.

Every platform image uses **QEMU** to run the guest OS. QEMU exposes a
**QEMU Monitor Protocol (QMP)** socket that accepts JSON commands for
keyboard, mouse, and screenshot operations — bypassing the guest entirely.
This is the universal control plane for all QEMU-backed images.

## Architecture

```
┌─────────────────────────────────────────────┐
│  Rust TestEnvironment (foundation_testbed)   │
│  ┌─────────────┐  ┌───────────────────────┐ │
│  │  QmpClient   │  │  GuestAgent           │ │
│  │  (QMP unix)  │  │  (SSH / ADB / WinRM)  │ │
│  │  • key_press │  │  • exec(command)      │ │
│  │  • mouse_move│  │  • install_tool()     │ │
│  │  • screendump│  │  • launch_app()       │ │
│  └──────┬───────┘  └──────────┬────────────┘ │
└─────────┼─────────────────────┼──────────────┘
          │ Docker unix socket  │ SSH/ADB/WinRM
    ┌─────▼─────────────────────▼──────────────┐
    │            QEMU VM (guest OS)             │
    │  ┌─────────┐  ┌────────────────────────┐ │
    │  │  QMP     │  │  Guest automation      │ │
    │  │  socket  │  │  • macOS: cliclick,    │ │
    │  │          │  │    osascript           │ │
    │  │  /run/   │  │  • Windows: AutoHotkey,│ │
    │  │  shm/    │  │    nircmd, PowerShell  │ │
    │  │  monitor │  │  • Linux: xdotool,     │ │
    │  │  .sock   │  │    ydotool, wlctrl    │ │
    │  └─────────┘  │  • Android: ADB input   │ │
    │               │  • iOS: WebDriverAgent  │ │
    │               └────────────────────────┘ │
    └──────────────────────────────────────────┘
```

**Two control tiers:**
- **Tier 1 (QMP)** — keyboard/mouse/screenshot directly through QEMU.
  Universal across ALL QEMU-backed images. Zero guest dependencies.
- **Tier 2 (Guest Agent)** — OS-native automation tools inside the guest.
  Higher-level operations (launch app, install package, run test).
  Accessed via SSH (macOS/Linux), WinRM/SSH (Windows), ADB (Android).

## Platform Audit

### macOS (dockurr/macos → ewestudios/qemu:7.37)

| Capability | Status | Action |
|------------|--------|--------|
| QMP socket | ✅ Exists at `/run/shm/monitor.sock` | No change |
| VNC display | ✅ Port 5900, websocket 5700 | No change |
| SSH | ✅ Port 22 | No change |
| QMP client (`socat`) | ❌ Not installed | **Add to Dockerfile** |
| Screenshot→PNG | ❌ QMP gives PPM, no converter | **Add imagemagick/netpbm** |
| Guest: cliclick | ❌ Not installed | **Add to install.sh** |
| Guest: osascript | ❌ Not installed | **Add to install.sh** (part of macOS) |
| Guest: Rust toolchain | ❌ Not installed | **Add to install.sh** |
| Guest: Tauri CLI | ❌ Not installed | **Add to install.sh** |

### Windows (dockurr/windows → ewestudios/qemu:7.36)

| Capability | Status | Action |
|------------|--------|--------|
| QMP socket | ✅ At `/run/shm/monitor.sock` | No change |
| RDP | ✅ Port 3389 | No change |
| VNC | ✅ Port 5900 | No change |
| QMP client (`socat`) | ❌ Not installed | **Add to Dockerfile** |
| Screenshot→PNG | ❌ | **Add imagemagick/netpbm** |
| Guest: AutoHotkey v2 | ❌ Not installed | **Add to install.bat** |
| Guest: nircmd | ❌ Not installed | **Add to install.bat** |
| Guest: Rust toolchain | ✅ In install.bat | No change |
| Guest: WebView2 | ✅ In install.bat | No change |
| Guest: OpenSSH | ✅ In install.bat | No change |

### Linux (artefacts/dockerfiles/linux)

| Capability | Status | Action |
|------------|--------|--------|
| QEMU | ❌ Native container, no QEMU | N/A — use xdotool |
| X11 + VNC | ✅ x11vnc on :99 | No change |
| SSH | ✅ Port 22 | No change |
| Guest: xdotool | ❌ Not installed | **Add to Dockerfile** |
| Guest: ydotool | ❌ Not installed | **Add to Dockerfile** (Wayland) |
| Guest: imagemagick | ❌ Not installed | **Add to Dockerfile** |
| Guest: scrot | ❌ Not installed | **Add to Dockerfile** |
| DBus + notification | ❌ | **Add dbus-x11** |
| Guest: Tauri deps | ✅ libgtk, libwebkit2gtk, etc. | No change |

### Android (artefacts/dockerfiles/android)

| Capability | Status | Action |
|------------|--------|--------|
| ADB | ✅ SDK installed, emulator pre-created | No change |
| Guest: `adb shell input` | ✅ Built into Android | No change |
| Guest: `screencap` | ✅ Built into Android | No change |
| Guest: `uiautomator` | ✅ Built into Android | No change |
| VNC (emulator display) | ✅ Port 5901 | No change |
| SSH | ✅ Port 22 | No change |
| Guest: Tauri Android deps | ✅ SDK + NDK installed | No change |
| KVM passthrough | ✅ Configured | No change |

### iOS (needs macOS + Xcode)

| Capability | Status | Action |
|------------|--------|--------|
| macOS host | ✅ Docker macOS exists | Needs Xcode installed |
| Xcode 16+ | ❌ Not in base image | **35GB download, one-time** |
| WebDriverAgent | ❌ | **Install via Carthage** |
| XCTest automation | ❌ | **ios-deploy + xcodebuild** |

## Solution

### Part 1: QMP client (Tier 1 — universal)

`QmpClient` is a Rust struct that talks to QEMU's monitor socket. Every
QEMU-backed image (macOS, Windows, ChromeOS) gets this for free.

```rust
// foundation_testbed/src/qmp.rs (NEW)

pub struct QmpClient {
    socket: PathBuf,  // e.g. /run/shm/monitor.sock
}

impl QmpClient {
    pub fn new(socket: impl Into<PathBuf>) -> Self;

    /// Connect to the QMP socket, negotiate capabilities.
    pub fn connect(&self) -> Result<QmpConnection, QmpError>;

    /// Send a key press/release. Uses QEMU key codes.
    pub fn send_key(&self, conn: &QmpConnection, key: QemuKey);
    pub fn send_keys(&self, conn: &QmpConnection, keys: &[QemuKey]);
    pub fn type_text(&self, conn: &QmpConnection, text: &str);

    /// Mouse control (absolute, 0-32767 range — matches QEMU).
    pub fn mouse_move(&self, conn: &QmpConnection, x: u16, y: u16);
    pub fn mouse_click(&self, conn: &QmpConnection, button: MouseButton);
    pub fn mouse_scroll(&self, conn: &QmpConnection, dy: i8);

    /// Screenshot returns PPM bytes (QEMU native format).
    pub fn screendump(&self, conn: &QmpConnection) -> Result<Vec<u8>, QmpError>;

    /// Convert PPM screenshot to PNG.
    pub fn screendump_png(&self, conn: &QmpConnection) -> Result<Vec<u8>, QmpError>;
}
```

QMP transport: Unix socket, JSON-RPC-like protocol.
```
→ {"execute":"qmp_capabilities"}
← {"return":{}}
→ {"execute":"input-send-event","arguments":{"events":[
     {"type":"btn","data":{"down":true,"button":"Left"}},
     {"type":"btn","data":{"down":false,"button":"Left"}}
   ]}}
← {"return":{}}
→ {"execute":"screendump","arguments":{"filename":"/dev/stdout"}}
← PPM binary data...
```

**Why not VNC/RDP for Tier 1?** VNC is a framebuffer protocol — you send pixel updates, not semantic input. QMP is semantic: "press Left Alt + Tab" is one command, not a pixel-level simulation. QMP is also 100x faster for screenshots (writes PPM to a pipe, ~10ms) vs VNC which requires a full frame grab.

### Part 2: Dockerfile extensions

#### macOS Dockerfile additions
```dockerfile
RUN apt-get install -y --no-install-recommends \
    socat \           # echo '{"execute":"qmp_capabilities"}' | socat - UNIX-CONNECT:/run/shm/monitor.sock
    imagemagick \     # convert PPM → PNG
    netpbm \          # pamtopng alternative
    python3 \         # scriptable QMP interaction
    jq                # JSON command construction
```

#### macOS guest (install.sh additions)
```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# cliclick — mouse/keyboard automation for macOS
# Pre-compiled binary, no Xcode needed
curl -L -o /usr/local/bin/cliclick https://github.com/BlueM/cliclick/releases/download/5.0/cliclick
chmod +x /usr/local/bin/cliclick
```

#### Windows Dockerfile additions
```dockerfile
RUN apt-get install -y --no-install-recommends socat imagemagick jq python3
```

#### Windows guest (install.bat additions)
```batch
REM AutoHotkey v2 — keyboard/mouse scripting
powershell -Command "Invoke-WebRequest 'https://www.autohotkey.com/download/ahk-v2.exe' -OutFile '%TEMP%\ahk-setup.exe'; & %TEMP%\ahk-setup.exe /S"

REM nircmd — small CLI for mouse/keyboard/screenshot
powershell -Command "Invoke-WebRequest 'https://www.nirsoft.net/utils/nircmd.zip' -OutFile '%TEMP%\nircmd.zip'; Expand-Archive '%TEMP%\nircmd.zip' 'C:\Windows'"

REM imagemagick — convert PPM screenshots
winget install ImageMagick.ImageMagick --silent
```

#### Linux Dockerfile additions
```dockerfile
RUN apt-get install -y --no-install-recommends \
    xdotool \         # X11 keyboard/mouse simulation
    ydotool \         # Wayland/uinput keyboard/mouse
    scrot \           # X11 screenshot
    imagemagick \     # convert/crop/annotate
    dbus-x11 \        # D-Bus for desktop notifications
    xclip \           # clipboard from command line
    wmctrl            # window manager control
```

#### Android Dockerfile additions
```dockerfile
# Already has ADB + emulator. Add convenience wrappers.
RUN apt-get install -y --no-install-recommends socat imagemagick
```

### Part 3: TestEnvironment API unification

Extend `TestEnvironment` (F30) with agentic methods:

```rust
impl TestEnvironment {
    // ── QMP Tier 1 (all QEMU-backed platforms) ──

    /// Send raw key presses via QMP.
    pub fn key_press(&self, key: &str) -> Result<(), TestEnvError>;

    /// Type a text string via QMP.
    pub fn type_text(&self, text: &str) -> Result<(), TestEnvError>;

    /// Move mouse to absolute coordinates (0.0-1.0).
    pub fn mouse_move(&self, x: f64, y: f64) -> Result<(), TestEnvError>;

    /// Click mouse button at current position.
    pub fn mouse_click(&self) -> Result<(), TestEnvError>;

    /// Capture a PNG screenshot.
    pub fn screenshot_png(&self) -> Result<Vec<u8>, TestEnvError>;

    // ── Guest Tier 2 (SSH/ADB/WinRM) ──

    /// Launch an application on the guest.
    pub fn launch_app(&self, app: &str) -> Result<(), TestEnvError>;

    /// Run a test binary and get its stdout.
    pub fn run_test(&self, binary: &str, args: &[&str]) -> Result<String, TestEnvError>;

    /// Install a package on the guest.
    pub fn install_package(&self, pkg: &str) -> Result<(), TestEnvError>;
}
```

### Part 4: Tauri app testing workflow

```rust
#[platform_test(docker = "macos")]
fn test_tauri_app_launches_on_macos() {
    // 1. The macro creates a session + TestEnvironment for "macos"
    let env = test_env();

    // 2. Copy the built .app bundle into the macOS guest
    env.copy_in(Path::new("target/tauri/MyApp.app"), "/Applications/MyApp.app");

    // 3. Launch the app via QMP keyboard shortcut (Cmd+Space, type name, Enter)
    env.key_combo(&["meta", "space"]);  // Spotlight
    env.type_text("MyApp");
    env.key_press("enter");
    env.sleep(Duration::from_secs(3));  // wait for launch

    // 4. Take a screenshot to verify
    let screenshot = env.screenshot_png().unwrap();
    assert!(!screenshot.is_empty());  // basic check

    // 5. Close the app
    env.key_combo(&["meta", "q"]);
}
```

## Requirements

### R1. QMP client — `foundation_testbed/src/qmp.rs` (NEW)
- Unix socket transport to `/run/shm/monitor.sock`
- `QmpClient` with `connect()`, `send_key()`, `mouse_move()`, `mouse_click()`, `screendump()`
- PPM→PNG conversion via `image` crate or ffmpeg/imagemagick
- `QmpError` enum (connection failed, protocol error, timeout)
- Feature-gated behind `docker-tests`

### R2. macOS Dockerfile — agentic tools
- `socat`, `imagemagick`, `python3`, `jq` added to Dockerfile
- `install.sh` adds Rust + cliclick + Tauri CLI
- QMP socket path documented in image metadata

### R3. Windows Dockerfile — agentic tools
- `socat`, `imagemagick`, `python3`, `jq` added to Dockerfile
- `install.bat` adds AutoHotkey v2 + nircmd + imagemagick

### R4. Linux Dockerfile — agentic tools
- `xdotool`, `ydotool`, `scrot`, `imagemagick`, `xclip`, `wmctrl`, `dbus-x11`
- VNC-based screenshot as fallback when scrot fails

### R5. Android Dockerfile — agentic tools
- `socat`, `imagemagick` for QMP screenshots
- Pre-authorized ADB key for `adb shell` without prompt
- Emulator snapshot for instant boot (avoids 2-minute cold start)

### R6. iOS automation path
- Document process: install Xcode → WebDriverAgent → XCTest
- Not Docker-automatable without Xcode (30GB download)
- Use macOS Docker + manual Xcode setup as documented path

### R7. TestEnvironment API extension — `docker.rs`
- `key_press(key)` — delegates to QMP or guest tool
- `type_text(text)` — delegates to QMP (single chars) or guest tool
- `mouse_move(x, y)` — delegates to QMP or guest tool
- `mouse_click()` — delegates to QMP or guest tool
- `screenshot_png()` — delegates to QMP screendump + PPM→PNG
- `launch_app(name)` — OS-specific (open on macOS, start on Windows, etc.)
- `key_combo(keys)` — QMP multi-key press+release

### R8. Screenshot pipeline
- macOS/Win/ChromeOS: QMP `screendump` → PPM bytes → `image` crate decode → PNG encode → Vec<u8>
- Linux: `scrot -o /tmp/screenshot.png` → `docker cp` → Vec<u8>
- Android: `adb exec-out screencap -p` → PNG bytes
- iOS: `xcrun simctl io booted screenshot` → PNG bytes

### R9. No new Rust dependencies for QMP
- QMP is JSON-over-Unix-socket — use `std::os::unix::net::UnixStream` + `serde_json`
- PPM→PNG: use `image` crate (already in workspace for `screenshot` feature)
- No external QMP library needed

### R10. Mouse coordinate normalization
- QMP uses 0-32767 absolute range for the virtio tablet
- Guest tools use pixel coordinates
- `mouse_move(x, y)` takes normalized 0.0-1.0, converts per-backend

## Verification

```bash
# QMP smoke test (any QEMU-backed container)
echo '{"execute":"qmp_capabilities"}{"execute":"query-version"}' | \
  socat - UNIX-CONNECT:/run/shm/monitor.sock

# Rust: QMP keyboard injection
cargo test -p foundation_testbed --features docker-tests -- qmp_send_key

# Rust: Screenshot via QMP
cargo test -p foundation_testbed --features docker-tests -- qmp_screendump

# Platform-specific: launch app via QMP
cargo test -p foundation_testbed --features docker-tests -- launch_tauri_app_macos

# Build updated images
docker build -t ewe-linux infrastructure/docker/linux -f artefacts/dockerfiles/linux/Dockerfile
docker build -t ewe-android infrastructure/docker/android -f artefacts/dockerfiles/android/Dockerfile
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_testbed/src/qmp.rs` | **NEW** — `QmpClient` with keyboard/mouse/screenshot |
| `backends/foundation_testbed/src/docker.rs` | Extend `TestEnvironment` with agentic methods |
| `artefacts/dockerfiles/linux/Dockerfile` | Add xdotool, scrot, imagemagick, dbus-x11, ydotool |
| `artefacts/dockerfiles/android/Dockerfile` | Add socat, imagemagick, ADB key pre-auth |
| `artefacts/dockerfiles/windows/oem/install.bat` | Add AutoHotkey, nircmd, imagemagick |
| `infrastructure/docker/macos/Dockerfile` | Add socat, imagemagick, python3, jq |
| `infrastructure/docker/macos/src/install.sh` | Add Rust, cliclick, Tauri CLI |
| `infrastructure/docker/windows/Dockerfile` | Add socat, imagemagick, python3, jq |
