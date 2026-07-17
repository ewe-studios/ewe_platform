#!/bin/bash
set -euo pipefail

DEV_USER="${DEV_USER:-developer}"
SSH_KEY_FILE="/tmp/authorized_keys"

echo "=== ewe-test-android starting ==="

# ── SSH setup ────────────────────────────────────────────────────────────────
if [ -f "${SSH_KEY_FILE}" ]; then
    mkdir -p "/home/${DEV_USER}/.ssh"
    cat "${SSH_KEY_FILE}" >> "/home/${DEV_USER}/.ssh/authorized_keys"
    chown -R "${DEV_USER}:${DEV_USER}" "/home/${DEV_USER}/.ssh"
    chmod 700 "/home/${DEV_USER}/.ssh"
    chmod 600 "/home/${DEV_USER}/.ssh/authorized_keys"
    echo "SSH key injected for ${DEV_USER}"
fi
/usr/sbin/sshd -D &

# ── X11 + VNC ────────────────────────────────────────────────────────────────
Xvfb "${DISPLAY}" -screen 0 "${RESOLUTION}x24" -ac +extension GLX +render &
sleep 1
openbox --replace &
sleep 1

# VNC on port 5901 (5900 used by Linux test image when co-located)
x11vnc -display "${DISPLAY}" -forever -nopw -shared -rfbport 5901 -quiet &
echo "VNC running on port 5901"

websockify --web /usr/share/novnc 6081 localhost:5901 &
echo "noVNC available at http://localhost:6081/vnc.html"

# ── KVM check ────────────────────────────────────────────────────────────────
if [ -e /dev/kvm ]; then
    echo "KVM available — emulator will use hardware acceleration"
else
    echo "WARNING: /dev/kvm not found — emulator will run in software mode (slow)"
fi

# ── ADB server ───────────────────────────────────────────────────────────────
adb start-server
echo "ADB server running on port 5037"

# ── Android emulator ─────────────────────────────────────────────────────────
# Start the pre-created AVD in headless mode (no GUI window — uses QEMU directly)
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

# ── Wait for emulator to boot ────────────────────────────────────────────────
echo "Waiting for emulator to boot..."
TIMEOUT=120
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
    echo "WARNING: Emulator did not boot within ${TIMEOUT}s. Tests may fail."
    echo "Last 20 lines of emulator log:"
    tail -20 /tmp/emulator.log
fi

echo "=== ewe-test-android ready ==="
echo "  SSH:   ssh ${DEV_USER}@localhost -p 22"
echo "  VNC:   vnc://localhost:5901"
echo "  Web:   http://localhost:6081/vnc.html"
echo "  ADB:   adb connect localhost:5555"
echo "  Emulator: PID ${EMULATOR_PID}, AVD ${AVD_NAME}"

tail -f /dev/null
