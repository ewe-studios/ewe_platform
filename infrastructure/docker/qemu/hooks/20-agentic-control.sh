#!/usr/bin/env bash
# F32: Agentic control — applied to ALL QEMU-backed images.
# Downstream images can override by COPYing their own 20-*.sh hook.

echo "[agentic] HMP monitor:  socat - UNIX-CONNECT:${QEMU_DIR}/monitor.sock"
echo "[agentic] QMP monitor:  socat - UNIX-CONNECT:${QEMU_DIR}/qmp.sock   (python3 qmp-shell ${QEMU_DIR}/qmp.sock)"
echo "[agentic] Screenshots:  HMP: screendump /tmp/x.ppm → convert /tmp/x.ppm /tmp/x.png"
echo "[agentic] Python QMP:   python3 -c 'from qemu.qmp import QMPClient; ...'"

return 0
