#!/usr/bin/env bash
set -Eeuo pipefail

echo "=== ewe-android starting ==="

AVD_NAME="${DEVICE:-pixel_6}"
echo "Device: ${AVD_NAME}"

# ── Emulator shared libs (Qt + tcmalloc + WebRTC from SDK) ─────────
export LD_LIBRARY_PATH="${EMULATOR_LIB64}:${LD_LIBRARY_PATH:-}"

# ── X11 display (emulator renders to this) ───────────────────────────
echo "Starting Xvfb on :99..."
Xvfb :99 -screen 0 1080x2400x24 -ac +extension GLX +render &
sleep 1

echo "Starting openbox..."
openbox --replace &
sleep 1

# ── x11vnc — capture X display → port 5900 ──────────────────────────
echo "Starting x11vnc on :5900..."
x11vnc -display :99 -forever -nopw -shared -quiet -rfbport 5900 &
sleep 1

# ── noVNC — proxy VNC → browser on port 6080 ────────────────────────
echo "Starting noVNC on :6080..."
websockify --web /usr/share/novnc 6080 localhost:5900 &
sleep 1

# ── ADB ──────────────────────────────────────────────────────────────
adb start-server

# ── Emulator — renders to DISPLAY=:99 via full QEMU + Qt libs ───────
echo "Starting emulator: ${AVD_NAME} (DISPLAY=:99, LD_LIBRARY_PATH set)"
nohup emulator \
    -avd "${AVD_NAME}" \
    -no-audio \
    -no-boot-anim \
    -no-snapshot-load \
    -gpu swiftshader_indirect \
    -netdelay none \
    -netspeed full \
    ${ANDROID_EMULATOR_EXTRA_ARGS:-} \
    > /tmp/emulator.log 2>&1 &

EMULATOR_PID=$!
echo "Emulator PID: ${EMULATOR_PID}"

# ── Wait for boot ────────────────────────────────────────────────────
echo "Waiting for boot..."
TIMEOUT=180; ELAPSED=0
while [ $ELAPSED -lt $TIMEOUT ]; do
    if adb -e shell getprop sys.boot_completed 2>/dev/null | grep -q "1"; then
        echo "Emulator booted in ${ELAPSED}s"
        break
    fi
    sleep 2; ELAPSED=$((ELAPSED + 2))
done

[ $ELAPSED -ge $TIMEOUT ] && echo "WARNING: timeout" && tail -20 /tmp/emulator.log

adb -e shell settings put secure show_ime_with_hard_keyboard 0 2>/dev/null || true

echo ""
echo "=== ewe-android ready ==="
echo "  Device:  ${AVD_NAME}"
echo "  noVNC:   http://localhost:6080/vnc.html"
echo "  VNC:     vnc://localhost:5900"
echo "  ADB:     adb connect localhost:5554"
echo ""
echo "Available devices (set DEVICE env var):"
avdmanager list avd 2>/dev/null | grep "Name:" | sed 's/.*Name: /  /'

tail -f /dev/null
