#!/bin/bash
set -euo pipefail

DEV_USER="${DEV_USER:-developer}"
SSH_KEY_FILE="/tmp/authorized_keys"

echo "=== ewe-test-linux starting ==="

# ── SSH setup ────────────────────────────────────────────────────────────────
if [ -f "${SSH_KEY_FILE}" ]; then
    mkdir -p "/home/${DEV_USER}/.ssh"
    cat "${SSH_KEY_FILE}" >> "/home/${DEV_USER}/.ssh/authorized_keys"
    chown -R "${DEV_USER}:${DEV_USER}" "/home/${DEV_USER}/.ssh"
    chmod 700 "/home/${DEV_USER}/.ssh"
    chmod 600 "/home/${DEV_USER}/.ssh/authorized_keys"
    echo "SSH key injected for ${DEV_USER}"
fi

# Start SSH daemon
/usr/sbin/sshd -D &

# ── X11 + VNC ────────────────────────────────────────────────────────────────
# Start a virtual X server with the configured resolution
Xvfb "${DISPLAY}" -screen 0 "${RESOLUTION}x24" -ac +extension GLX +render &
sleep 1

# Start lightweight window manager
openbox --replace &
sleep 1

# Start clipboard sync daemon — bridges X11 PRIMARY/CLIPBOARD selections
# so copy/paste works between the VM and VNC client
autocutsel -fork &
autocutsel -selection PRIMARY -fork &

# Start VNC server attached to the virtual display (no password — dev only)
x11vnc -display "${DISPLAY}" -forever -nopw -shared -quiet &
echo "VNC running on :0 (port 5900)"

# ── noVNC — web-based VNC client ─────────────────────────────────────────────
# Proxies VNC to a browser on port 6080
websockify --web /usr/share/novnc 6080 localhost:5900 &
echo "noVNC available at http://localhost:6080/vnc.html"

# ── Keep container alive ─────────────────────────────────────────────────────
echo "=== ewe-test-linux ready ==="
echo "  SSH:  ssh ${DEV_USER}@localhost -p 22"
echo "  VNC:  vnc://localhost:5900"
echo "  Web:  http://localhost:6080/vnc.html"

# Wait forever — the container stays running until explicitly stopped
tail -f /dev/null
