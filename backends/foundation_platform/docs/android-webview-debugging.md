# Android WebView Debugging Guide

Practical techniques for debugging the Foundation Platform Android WASM app (`examples/platform_android/`). Package: `com.ewe.platform`.

## 1. Logcat monitoring

The primary tool for viewing Android system and app logs.

```bash
# Clear the log buffer before starting a fresh session
adb logcat -c

# Dump all current logs (one-shot)
adb logcat -d

# Dump and exit immediately, useful for scripting
adb logcat -d -s

# Follow live output (Ctrl+C to stop)
adb logcat
```

### Key filter tags

The most useful log tags for this project:

| Tag | What it shows |
|---|---|
| `Tauri/Console` | `console.log()` / `console.error()` from the WebView JS context |
| `RustStdoutStderr` | `println!()` and `eprintln!()` from the Rust side (routed through Tauri's logger) |
| `chromium` | Chromium WebView engine internals (network errors, rendering) |
| `Uncaught` | Uncaught JS exceptions (search with `grep`, not a formal tag) |

```bash
# Filter to specific tags
adb logcat -s Tauri/Console

# Combine multiple tags with grep
adb logcat | grep -E "Tauri/Console|RustStdoutStderr|chromium"

# Watch for JS errors
adb logcat | grep -i "Uncaught\|console\.error\|E/Tauri"

# Grep for a specific string (e.g. a log line from your code)
adb logcat -d | grep "platform_android"
```

### Rust-side logging

Any `println!()` in the Tauri app (e.g. in `examples/platform_android/src-tauri/src/lib.rs`) will appear under the `RustStdoutStderr` tag. The startup log at line 92 is a good canary:

```rust
println!("[platform_android] Session: {:?}", session.session_id());
```

## 2. Chrome DevTools Protocol via adb forward

WebView exposes the Chrome DevTools Protocol (CDP) over a Unix domain socket. You can forward it to your machine's TCP port and use any CDP client (Chrome DevTools, curl, custom scripts).

### 2.1 Find the WebView DevTools socket

```bash
adb shell "cat /proc/net/unix | grep webview_devtools"
```

Output looks like:

```
0000000000000000: 00000002 00000000 00010000 0001 01 1234567 @webview_devtools_remote_12345
```

The number after `@webview_devtools_remote_` is the process PID. There will be one socket per WebView instance. A Tauri app with a single WebView will have one entry.

### 2.2 Forward the socket to a local TCP port

```bash
# Replace <PID> with the actual PID from the step above
adb forward tcp:9229 localabstract:webview_devtools_remote_<PID>
```

### 2.3 Verify with curl

```bash
# List inspectable pages
curl http://localhost:9229/json

# Response will be a JSON array with page metadata:
# [{"id": "1", "title": "...", "url": "ewe://localhost/app/", "webSocketDebuggerUrl": "ws://..."}]
```

### 2.4 Connect Chrome DevTools

Open `chrome://inspect` in Chrome, or navigate directly to `http://localhost:9229` to see the inspectable page. Click "inspect" to open a full DevTools window with Console, Network, Sources, etc.

### 2.5 Evaluate JavaScript via CDP WebSocket (Python)

For quick evaluation without opening Chrome, use a Python script that speaks the CDP WebSocket protocol. The key detail is that CDP messages are WebSocket **text frames** with the standard WebSocket framing (opcode `0x1`).

```python
#!/usr/bin/env python3
"""Quick JS evaluation on an Android WebView via CDP."""
import sys, json, socket, struct, time

def ws_send(sock, payload: str):
    """Send a WebSocket text frame (opcode 0x1) with masking."""
    data = payload.encode("utf-8")
    mask = b'\x12\x34\x56\x78'  # Arbitrary mask; CDP doesn't validate
    frame = bytearray()
    frame.append(0x81)  # FIN + text opcode
    length = len(data)
    if length < 126:
        frame.append(0x80 | length)
    elif length < 65536:
        frame.append(0x80 | 126)
        frame.extend(struct.pack("!H", length))
    else:
        frame.append(0x80 | 127)
        frame.extend(struct.pack("!Q", length))
    frame.extend(mask)
    frame.extend(bytes(b ^ mask[i % 4] for i, b in enumerate(data)))
    sock.sendall(frame)

def ws_recv(sock) -> str:
    """Receive and unmask a WebSocket frame."""
    header = sock.recv(2)
    opcode = header[0] & 0x0F
    masked = bool(header[1] & 0x80)
    length = header[1] & 0x7F
    if length == 126:
        length = struct.unpack("!H", sock.recv(2))[0]
    elif length == 127:
        length = struct.unpack("!Q", sock.recv(8))[0]
    mask = sock.recv(4) if masked else b''
    payload = b''
    while len(payload) < length:
        chunk = sock.recv(length - len(payload))
        if not chunk:
            break
        payload += chunk
    if masked:
        payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
    return payload.decode("utf-8")

if len(sys.argv) < 2:
    print("Usage: python3 cdp_eval.py <javascript expression>")
    sys.exit(1)

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.connect(("127.0.0.1", 9229))

# Get the WebSocket URL from /json
import urllib.request
pages = json.loads(urllib.request.urlopen("http://127.0.0.1:9229/json").read())
ws_url = pages[0]["webSocketDebuggerUrl"]

# Connect to the WebSocket (parse ws://host:port/path)
from urllib.parse import urlparse
parsed = urlparse(ws_url)
ws_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
ws_sock.connect((parsed.hostname, parsed.port))

# Perform HTTP upgrade to WebSocket
upgrade = (
    f"GET {parsed.path} HTTP/1.1\r\n"
    f"Host: {parsed.hostname}:{parsed.port}\r\n"
    "Upgrade: websocket\r\n"
    "Connection: Upgrade\r\n"
    "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
    "Sec-WebSocket-Version: 13\r\n\r\n"
)
ws_sock.sendall(upgrade.encode())
resp = b""
while b"\r\n\r\n" not in resp:
    resp += ws_sock.recv(4096)

msg_id = 1
ws_send(ws_sock, json.dumps({
    "id": msg_id,
    "method": "Runtime.evaluate",
    "params": {"expression": sys.argv[1], "returnByValue": True}
}))
time.sleep(0.5)
result = ws_recv(ws_sock)
print(json.dumps(json.loads(result), indent=2))
```

Usage:

```bash
python3 cdp_eval.py "document.title"
python3 cdp_eval.py "JSON.stringify(window.location)"
python3 cdp_eval.py "console.log('hello from CDP')"
```

### 2.6 Useful CDP commands

| Command | Purpose |
|---|---|
| `Runtime.evaluate` | Evaluate arbitrary JavaScript in the page context |
| `Runtime.consoleAPICalled` | Subscribe to `console.log`/`console.error` events (enable `Runtime` domain first) |
| `Runtime.exceptionThrown` | Catch uncaught JS exceptions |
| `Page.navigate` | Navigate the WebView to a new URL |
| `Page.reload` | Reload the current page |
| `Network.enable` | Start capturing network requests |
| `Log.enable` | Capture browser console messages |

Example: navigate to a different page:

```json
{"id": 1, "method": "Page.navigate", "params": {"url": "ewe://localhost/app-hello/"}}
```

## 3. UI inspection

### Screenshot capture

```bash
# Capture screen to PNG
adb exec-out screencap -p > screenshot.png

# For scripting: direct PNG output (no newline conversion issues)
adb exec-out screencap -p > screen.png
```

### Layout dump

```bash
# Dump current UI hierarchy as XML
adb shell uiautomator dump /sdcard/ui.xml

# Pull to local machine
adb pull /sdcard/ui.xml .

# View the XML structure
cat ui.xml | xmllint --format -
```

The XML dump shows every UI element, its bounds, class name, text content, and accessibility properties. Useful for verifying that the WebView is actually rendering content and occupies the expected screen region.

## 4. Package management

### Install and uninstall

```bash
# Install APK (replace existing installation)
adb install -r path/to/app.apk

# Uninstall by package name
adb uninstall com.ewe.platform
```

### App lifecycle control

```bash
# Force-stop the app
adb shell am force-stop com.ewe.platform

# Start the app
adb shell am start -n com.ewe.platform/.MainActivity

# Clear app data (factory reset for the app)
adb shell pm clear com.ewe.platform
```

### Check if the app is running

```bash
adb shell ps -A | grep com.ewe.platform
```

## 5. Build modes and the `debug_assertions` gotcha

### The problem

The `EmbedDirectoryAs` derive macro (used at `examples/platform_android/src-tauri/src/generated/app.rs` and `app_hello.rs`) has different behavior in debug vs release builds:

- **Debug builds** (`cfg(debug_assertions)`): The `FILES_DATA` constant is set to `&[]` (empty). File contents are read from disk at runtime using `std::fs::File::open()`. This works on the dev machine where the `public/` directory exists, but **fails on the Android device** because the file paths refer to the build machine's filesystem.

- **Release builds** (`else` branch): File contents are embedded as `&'static [u8]` arrays in the binary. This is what actually works on device.

This logic lives in `backends/foundation_macros/src/embedders.rs`, in the `impl_embeddable_directory` function (line 713: `if cfg!(debug_assertions)`). For directories, the release path embeds files via `const FILES_DATA: &'static [StaticDirectoryData] = &[...]`, while the debug path reads at runtime.

### The fix

Build with release profile for actual device testing:

```bash
# Build for x86_64 emulator (release)
cargo tauri android build --target x86_64

# Build for ARM64 device (release)
cargo tauri android build --target aarch64
```

The Tauri build pipeline for Android (`cargo tauri android build`) produces a release APK by default. If you need a debug APK for any reason, you must ensure the `public/` directory is available on the device filesystem, which is impractical.

### How to verify

Check the generated code at `examples/platform_android/src-tauri/src/generated/app.rs`. In a release build, you should see large `&[1u8, 2u8, ...]` byte arrays for `FILES_DATA`. In debug, you will see `&[]` and the `read_utf8_for` method will use `std::fs::File::open`.

## 6. Release APK signing for emulator

Release APKs must be signed, even for local emulator testing.

### Generate a keystore and key (one-time)

```bash
keytool -genkey -v \
  -keystore ~/debug.keystore \
  -alias debug \
  -keyalg RSA \
  -keysize 2048 \
  -validity 10000 \
  -storepass android \
  -keypass android \
  -dname "CN=Android Debug, OU=Development, O=EWE, L=Unknown, ST=Unknown, C=US"
```

### Sign the APK

The APK is produced by `cargo tauri android build` at:

```
examples/platform_android/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
```

Sign with `apksigner` (preferred, comes with Android SDK build-tools):

```bash
$ANDROID_HOME/build-tools/36.0.0/apksigner sign \
  --ks ~/debug.keystore \
  --ks-pass pass:android \
  --key-pass pass:android \
  --ks-key-alias debug \
  path/to/app-universal-release-unsigned.apk
```

Alternatively, sign with `jarsigner` then zipalign:

```bash
jarsigner -verbose -sigalg SHA1withRSA -digestalg SHA1 \
  -keystore ~/debug.keystore \
  -storepass android -keypass android \
  path/to/app-universal-release-unsigned.apk debug

zipalign -v 4 path/to/app-universal-release-unsigned.apk app-signed.apk
```

### Install the signed APK

```bash
adb install -r app-signed.apk
```

## 7. Quick CDP evaluation one-liner

A compact Python script for one-shot JS evaluation on the connected WebView. Save as `cdp_eval.py` or paste inline.

Requirements: Python 3, no external packages (stdlib only).

```python
#!/usr/bin/env python3
"""cdp_eval.py <js_expr> — evaluate JavaScript on an Android WebView via CDP.

Requires: adb forward already set up (port 9229 forwarded to webview_devtools_remote_<PID>).
Prerequisites:
    adb forward tcp:9229 localabstract:webview_devtools_remote_<PID>

Usage:
    python3 cdp_eval.py "document.title"
    python3 cdp_eval.py "location.href"
    python3 cdp_eval.py "JSON.stringify(performance.getEntries())"
"""
import sys, json, socket, struct, urllib.request, urllib.parse, time

CDP_PORT = 9229

def ws_frame(payload: str) -> bytes:
    data = payload.encode()
    mask = b'\x12\x34\x56\x78'
    length = len(data)
    frame = bytearray([0x81])  # FIN + text opcode
    if length < 126:
        frame.append(0x80 | length)
    elif length < 65536:
        frame.append(0x80 | 126)
        frame.extend(struct.pack("!H", length))
    else:
        frame.append(0x80 | 127)
        frame.extend(struct.pack("!Q", length))
    frame.extend(mask)
    frame.extend(bytes(b ^ mask[i % 4] for i, b in enumerate(data)))
    return bytes(frame)

def ws_recv(sock) -> str:
    hdr = sock.recv(2)
    length = hdr[1] & 0x7F
    if length == 126:
        length = struct.unpack("!H", sock.recv(2))[0]
    elif length == 127:
        length = struct.unpack("!Q", sock.recv(8))[0]
    mask = sock.recv(4)
    payload = bytearray()
    while len(payload) < length:
        chunk = sock.recv(length - len(payload))
        if not chunk: break
        payload.extend(chunk)
    return bytes(b ^ mask[i % 4] for i, b in enumerate(payload)).decode()

def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    # 1. Get WebSocket URL from /json endpoint
    with urllib.request.urlopen(f"http://127.0.0.1:{CDP_PORT}/json") as f:
        pages = json.loads(f.read())
    if not pages:
        print("ERROR: No inspectable pages found. Is the WebView running?")
        sys.exit(1)
    ws_url = pages[0]["webSocketDebuggerUrl"]
    parsed = urllib.parse.urlparse(ws_url)

    # 2. TCP + HTTP upgrade to WebSocket
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect((parsed.hostname, parsed.port))
    upgrade = (
        f"GET {parsed.path} HTTP/1.1\r\n"
        f"Host: {parsed.hostname}:{parsed.port}\r\n"
        "Upgrade: websocket\r\nConnection: Upgrade\r\n"
        "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
        "Sec-WebSocket-Version: 13\r\n\r\n"
    )
    s.sendall(upgrade.encode())
    resp = b""
    while b"\r\n\r\n" not in resp:
        resp += s.recv(4096)

    # 3. Send Runtime.evaluate
    s.sendall(ws_frame(json.dumps({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": {"expression": sys.argv[1], "returnByValue": True}
    })))
    time.sleep(0.3)

    # 4. Read result (may span multiple frames; here we take the first)
    result = ws_recv(s)
    try:
        parsed = json.loads(result)
        if "result" in parsed:
            val = parsed["result"].get("result", {}).get("value", parsed["result"])
            print(json.dumps(val, indent=2, ensure_ascii=False))
        elif "error" in parsed:
            print(f"CDP Error: {parsed['error']}", file=sys.stderr)
        else:
            print(json.dumps(parsed, indent=2, ensure_ascii=False))
    except json.JSONDecodeError:
        print(result)

if __name__ == "__main__":
    main()
```

## Quick-reference cheat sheet

```bash
# ── Logcat ──
adb logcat -c                              # Clear
adb logcat | grep -E "Tauri/Console|RustStdoutStderr"  # App logs
adb logcat -d | grep -i "uncaught\|error\|fatal"       # Errors

# ── CDP ──
adb shell "cat /proc/net/unix | grep webview_devtools"  # Find socket
adb forward tcp:9229 localabstract:webview_devtools_remote_<PID>  # Forward
curl http://localhost:9229/json                          # List pages

# ── UI ──
adb exec-out screencap -p > screen.png                   # Screenshot
adb shell uiautomator dump /sdcard/ui.xml && adb pull /sdcard/ui.xml .  # Layout

# ── Package ──
adb install -r app.apk                                   # Install
adb shell am force-stop com.ewe.platform                 # Kill app
adb shell am start -n com.ewe.platform/.MainActivity     # Launch app
adb uninstall com.ewe.platform                           # Remove app

# ── Build ──
cargo tauri android build --target x86_64                # Emulator (release)
cargo tauri android build --target aarch64               # ARM64 device (release)
```

## Relevant project files

| File | Purpose |
|---|---|
| `examples/platform_android/src-tauri/src/lib.rs` | App entry point, route configuration, WASM app responders |
| `examples/platform_android/src-tauri/src/generated/app.rs` | `EmbedDirectoryAs`-derived `AppAssets` (auto-generated) |
| `examples/platform_android/src-tauri/src/generated/app_hello.rs` | `EmbedDirectoryAs`-derived second app (auto-generated) |
| `examples/platform_android/src-tauri/tauri.conf.json` | Tauri configuration (window URL, identifier) |
| `examples/platform_android/src-tauri/gen/android/app/src/main/java/com/ewe/platform/MainActivity.kt` | Android Activity subclass |
| `examples/platform_android/src-tauri/gen/android/app/src/main/java/com/ewe/platform/generated/RustWebViewClient.kt` | WebView intercept handler (custom scheme routing) |
| `examples/platform_android/src-tauri/gen/android/app/build.gradle.kts` | Android build config (namespace, SDK versions) |
| `examples/platform_android/src-tauri/build.rs` | Build script (calls codegen, builds WASM apps) |
| `backends/foundation_platform/src/codegen.rs` | Code generation (generates AppAssets structs, patches tauri.conf.json) |
| `backends/foundation_macros/src/embedders.rs` | `EmbedDirectoryAs` macro (debug vs release file embedding) |
| `backends/foundation_platform/src/builder.rs` | `PlatformBuilder` with `route_with()` API |
