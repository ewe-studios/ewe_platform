#!/bin/bash
set -euo pipefail

DEV_USER="${DEV_USER:-developer}"
SSH_KEY_FILE="/tmp/authorized_keys"

echo "=== ewe-test-android starting ==="

# ── SSH setup ────────────────────────────────────────────────────────────
if [ -f "${SSH_KEY_FILE}" ]; then
    mkdir -p "/home/${DEV_USER}/.ssh"
    cat "${SSH_KEY_FILE}" >> "/home/${DEV_USER}/.ssh/authorized_keys"
    chown -R "${DEV_USER}:${DEV_USER}" "/home/${DEV_USER}/.ssh"
    chmod 700 "/home/${DEV_USER}/.ssh"
    chmod 600 "/home/${DEV_USER}/.ssh/authorized_keys"
    echo "SSH key injected for ${DEV_USER}"
fi
/usr/sbin/sshd -D &

# ── KVM check ────────────────────────────────────────────────────────────
if [ -e /dev/kvm ]; then
    echo "KVM available"
else
    echo "WARNING: /dev/kvm not found"
fi

# ── ADB server ───────────────────────────────────────────────────────────
adb -a -P 5037 start-server 2>/dev/null || adb start-server

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
TIMEOUT=120; ELAPSED=0
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
fi

# ── ADB screencap → PNG → HTTP (visual emulator access) ──────────────────
# Serves the latest Android screenshot on port 6081.
# Refresh the page to get a fresh screenshot. No VNC/QEMU display needed.
cat > /tmp/screencap_server.py << 'PYEOF'
import http.server, subprocess, sys, os

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/":
            self.send_response(200)
            self.send_header("Content-type", "text/html; charset=utf-8")
            self.end_headers()
            html = """<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Android Emulator</title>
<meta http-equiv="refresh" content="2">
<style>body{margin:0;background:#000;display:flex;justify-content:center;align-items:center;min-height:100vh}
img{max-width:100%;max-height:100vh}</style></head>
<body><img src="/screen.png" onerror="this.src='/screen.png?retry='+Date.now()"></body></html>"""
            self.wfile.write(html.encode())
        elif self.path.startswith("/screen.png"):
            try:
                result = subprocess.run(
                    ["adb", "-e", "exec-out", "screencap", "-p"],
                    capture_output=True, timeout=5
                )
                self.send_response(200)
                self.send_header("Content-type", "image/png")
                self.send_header("Cache-Control", "no-cache")
                self.end_headers()
                self.wfile.write(result.stdout)
            except Exception as e:
                self.send_error(500, str(e))
        else:
            self.send_error(404)

http.server.HTTPServer(("0.0.0.0", 6081), Handler).serve_forever()
PYEOF

python3 /tmp/screencap_server.py &
echo "ADB screencap server running on port 6081"
echo "  Browser: http://localhost:6081/ (auto-refreshes every 2s)"

echo "=== ewe-test-android ready ==="
echo "  Screen:  http://localhost:6081/"
echo "  ADB:     adb connect localhost:5554"
echo "  SSH:     ssh ${DEV_USER}@localhost -p 5022"
echo "  Emulator: PID ${EMULATOR_PID}"

tail -f /dev/null
