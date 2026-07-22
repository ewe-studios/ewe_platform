#!/usr/bin/env bash
set -Eeuo pipefail

echo "=== ewe-android starting ==="

AVD_NAME="${DEVICE:-pixel_6}"
echo "Device: ${AVD_NAME}"

# ── Emulator libs (Qt + tcmalloc from SDK) ───────────────────────────
export LD_LIBRARY_PATH="/opt/android-sdk/emulator/lib64:/opt/android-sdk/emulator/lib64/qt/lib"

# ── ADB ──────────────────────────────────────────────────────────────
adb start-server

# ── Emulator ─────────────────────────────────────────────────────────
# docker-android verified pattern: -no-window, -gpu swiftshader_indirect.
# gfxstream renders to GPU framebuffer (not X11). ADB screencap is the
# official graphics capture path.
echo "Starting emulator: ${AVD_NAME}"
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
[ $ELAPSED -ge $TIMEOUT ] && echo "WARNING: timeout" && tail -5 /tmp/emulator.log

adb -e shell settings put secure show_ime_with_hard_keyboard 0 2>/dev/null || true

# ── Screen server (ADB screencap → HTTP, port 6080) ──────────────────
python3 /run/screencap-http.py 6080 &
echo "Screen: http://localhost:6080/"

echo ""
echo "=== ewe-android ready ==="
echo "  Device:  ${AVD_NAME}"
echo "  Screen:  http://localhost:6080/"
echo "  ADB:     adb connect localhost:5554"
echo ""
avdmanager list avd 2>/dev/null | grep "Name:" | sed 's/.*Name: /  /'

tail -f /dev/null
