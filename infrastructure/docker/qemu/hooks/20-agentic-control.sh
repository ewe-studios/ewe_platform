#!/usr/bin/env bash
# F32: Agentic control info — printed at boot time.
# Downstream images can override by COPYing their own 20-*.sh hook.

QEMU_DIR="${QEMU_DIR:-/run/shm}"
echo "[agentic] HMP monitor:  socat - UNIX-CONNECT:${QEMU_DIR}/monitor.sock"
echo "[agentic] QMP monitor:  socat - UNIX-CONNECT:${QEMU_DIR}/qmp.sock   (python3 qmp-shell ${QEMU_DIR}/qmp.sock)"
echo "[agentic] Screenshots:  HMP: screendump /tmp/x.ppm  →  convert /tmp/x.ppm /tmp/x.png"

return 0
