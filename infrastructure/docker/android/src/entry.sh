#!/usr/bin/env bash
set -Eeuo pipefail

echo "=== ewe-android starting ==="

AVD_NAME="${DEVICE:-pixel_6}"
echo "Device: ${AVD_NAME}"

# ── Emulator shared libs (Qt + tcmalloc + WebRTC from SDK) ───────────
export LD_LIBRARY_PATH="/opt/android-sdk/emulator/lib64:/opt/android-sdk/emulator/lib64/qt/lib"

# ── Suppress nested VM warning ───────────────────────────────────────
mkdir -p /root/.config/Android\ Open\ Source\ Project
cat > "/root/.config/Android Open Source Project/Emulator.conf" << 'EOF'
[General]
showNestedWarning=false
EOF

# ── docker-android boot sequence ─────────────────────────────────────
echo "Starting Xvfb on :0..."
Xvfb :0 -screen 0 ${SCREEN_WIDTH:-1080}x${SCREEN_HEIGHT:-2400}x${SCREEN_DEPTH:-24} -ac +extension GLX &
sleep 1

echo "Starting openbox..."
openbox --replace &
sleep 1

echo "Starting x11vnc on :5900..."
x11vnc -display :0 -forever -nopw -shared -quiet -rfbport 5900 &
sleep 1

echo "Starting noVNC on :6080..."
websockify --web /usr/share/novnc 6080 localhost:5900 &
sleep 1

# ── ADB ──────────────────────────────────────────────────────────────
adb start-server

# ── Emulator — renders its window to DISPLAY=:0 ──────────────────────
echo "Starting emulator: ${AVD_NAME}"
nohup emulator \
    -avd "${AVD_NAME}" \
    -no-audio \
    -no-boot-anim \
    -gpu swiftshader_indirect \
    -accel on \
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
[ $ELAPSED -ge $TIMEOUT ] && echo "WARNING: timeout" && tail -10 /tmp/emulator.log

adb -e shell settings put secure show_ime_with_hard_keyboard 0 2>/dev/null || true

echo ""
echo "=== ewe-android ready ==="
echo "  Device:  ${AVD_NAME} (${SCREEN_WIDTH:-1080}x${SCREEN_HEIGHT:-2400})"
echo "  noVNC:   http://localhost:6080/vnc.html"
echo "  VNC:     vnc://localhost:5900"
echo "  ADB:     adb connect localhost:5554"
echo ""
avdmanager list avd 2>/dev/null | grep "Name:" | sed 's/.*Name: /  /'

tail -f /dev/null
