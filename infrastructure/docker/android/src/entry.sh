#!/usr/bin/env bash
set -Eeuo pipefail

echo "=== ewe-android starting (QEMU base + emulator) ==="

# ── Device selection ─────────────────────────────────────────────────────
AVD_NAME="${DEVICE:-pixel_6}"
echo "Device: ${AVD_NAME}"

# ── Xvfb ─────────────────────────────────────────────────────────────────
echo "Starting Xvfb on :99 (1080x2400)..."
Xvfb :99 -screen 0 1080x2400x24 -ac +extension GLX +render &
sleep 1

# ── openbox ──────────────────────────────────────────────────────────────
echo "Starting openbox..."
openbox --replace &
sleep 1

# ── x11vnc — capture X display on port 5900 ─────────────────────────────
echo "Starting x11vnc on :5900..."
x11vnc -display :99 -forever -nopw -shared -quiet -rfbport 5900 &
sleep 1

# ── noVNC — proxy VNC to browser on port 6080 ────────────────────────────
echo "Starting noVNC on :6080..."
websockify --web /usr/share/novnc 6080 localhost:5900 &
sleep 1

# ── ADB server ───────────────────────────────────────────────────────────
adb start-server
echo "ADB server started"

# ── Android emulator ─────────────────────────────────────────────────────
# Emulator renders its window to DISPLAY=:99 (Xvfb).
# x11vnc captures :99 → port 5900 → websockify proxies to :6080.
# No -no-window — we want the emulator to actually draw its UI.
echo "Starting Android emulator: ${AVD_NAME}"
nohup emulator \
    -avd "${AVD_NAME}" \
    -no-audio \
    -no-boot-anim \
    -gpu swiftshader_indirect \
    -netdelay none \
    -netspeed full \
    ${ANDROID_EMULATOR_EXTRA_ARGS:-} \
    > /tmp/emulator.log 2>&1 &

EMULATOR_PID=$!
echo "Emulator PID: ${EMULATOR_PID}"

# ── Wait for boot ────────────────────────────────────────────────────────
echo "Waiting for emulator to boot..."
TIMEOUT=180; ELAPSED=0
while [ $ELAPSED -lt $TIMEOUT ]; do
    if adb -e shell getprop sys.boot_completed 2>/dev/null | grep -q "1"; then
        echo "Emulator booted in ${ELAPSED}s"
        break
    fi
    sleep 2; ELAPSED=$((ELAPSED + 2))
done

if [ $ELAPSED -ge $TIMEOUT ]; then
    echo "WARNING: Emulator did not boot within ${TIMEOUT}s."
    tail -20 /tmp/emulator.log
fi

# ── HW keyboard ─────────────────────────────────────────────────────────
adb -e shell settings put secure show_ime_with_hard_keyboard 0 2>/dev/null || true

echo ""
echo "=== ewe-android ready ==="
echo "  Device:  ${AVD_NAME}"
echo "  noVNC:   http://localhost:6080/vnc.html"
echo "  VNC:     vnc://localhost:5900"
echo "  ADB:     adb connect localhost:5554"
echo "  Display: :99 (1080x2400)"
echo "  Emulator: PID ${EMULATOR_PID}"

# ── List available devices ───────────────────────────────────────────────
echo ""
echo "Available devices (set DEVICE env var to switch):"
avdmanager list avd 2>/dev/null | grep "Name:" | sed 's/.*Name: /  /'

tail -f /dev/null
