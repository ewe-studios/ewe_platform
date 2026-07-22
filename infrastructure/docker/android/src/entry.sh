#!/usr/bin/env bash
set -Eeuo pipefail

echo "=== ewe-android starting (QEMU base + emulator) ==="

# ── Xvfb — virtual X display for the emulator ────────────────────────────
echo "Starting Xvfb on ${DISPLAY:-:99} (1080x2400)..."
Xvfb "${DISPLAY:-:99}" -screen 0 1080x2400x24 -ac +extension GLX +render &
sleep 1

# ── openbox — lightweight window manager ─────────────────────────────────
echo "Starting openbox..."
openbox --replace &
sleep 1

# ── x11vnc — capture X display, serve on port 5900 ───────────────────────
echo "Starting x11vnc on :5900..."
x11vnc -display "${DISPLAY:-:99}" -forever -nopw -shared -quiet -rfbport 5900 &
sleep 1

# ── noVNC — proxy VNC to browser on port 6080 ────────────────────────────
echo "Starting noVNC on :6080..."
websockify --web /usr/share/novnc 6080 localhost:5900 &
sleep 1

# ── ADB server ───────────────────────────────────────────────────────────
adb start-server
echo "ADB server started"

# ── Set wallpaper (plain color, no feh needed) ───────────────────────────
# feh is not installed — we don't need wallpaper. The emulator is the only
# thing rendering on the display.

# ── Android emulator ─────────────────────────────────────────────────────
AVD_NAME="${ANDROID_AVD_NAME:-test_avd}"

echo "Starting Android emulator: ${AVD_NAME}"
nohup emulator \
    -avd "${AVD_NAME}" \
    -no-window \
    -no-audio \
    -no-boot-anim \
    -gpu swiftshader_indirect \
    -netdelay none \
    -netspeed full \
    ${ANDROID_EMULATOR_EXTRA_ARGS:-} \
    > /tmp/emulator.log 2>&1 &

EMULATOR_PID=$!
echo "Emulator PID: ${EMULATOR_PID}"

# ── Wait for emulator to boot ────────────────────────────────────────────
echo "Waiting for emulator to boot..."
TIMEOUT=180
ELAPSED=0
while [ $ELAPSED -lt $TIMEOUT ]; do
    if adb -e shell getprop sys.boot_completed 2>/dev/null | grep -q "1"; then
        echo "Emulator booted in ${ELAPSED}s"
        break
    fi
    sleep 2
    ELAPSED=$((ELAPSED + 2))
done

if [ $ELAPSED -ge $TIMEOUT ]; then
    echo "WARNING: Emulator did not boot within ${TIMEOUT}s."
    echo "Last 20 lines of emulator log:"
    tail -20 /tmp/emulator.log
fi

# ── HW keyboard enabled ─────────────────────────────────────────────────
adb -e shell settings put secure show_ime_with_hard_keyboard 0 2>/dev/null || true
echo "HW keyboard IME disabled"

echo ""
echo "=== ewe-android ready ==="
echo "  noVNC:   http://localhost:6080/vnc.html"
echo "  VNC:     vnc://localhost:5900"
echo "  ADB:     adb connect localhost:5554"
echo "  SSH:     ssh root@localhost"
echo "  Emulator: PID ${EMULATOR_PID}, AVD ${AVD_NAME}"
echo "  Display:  ${DISPLAY:-:99} (1080x2400)"

tail -f /dev/null
