#!/usr/bin/env bash
set -Eeuo pipefail

# ── Customization hooks (F32) ─────────────────────────────────────────────
#
# This runs BEFORE anything else in the boot sequence.
# Place scripts in /run/hooks/ and they will be sourced in sorted order.
#
# Downstream images (macOS, Windows, ChromeOS) can COPY hooks here:
#   COPY --chmod=755 ./hooks /run/hooks/
#
# docker-compose users can mount hooks:
#   volumes:
#     - ./my-hooks:/run/hooks
#
# Hook conventions:
#   00-*.sh — early boot (before config, before disks)
#   10-*.sh — pre-config (before config.sh runs)
#   20-*.sh — pre-boot (after config, before QEMU starts)
#   30-*.sh — post-boot (after QEMU PID exists, for background tasks)

HOOK_DIR="/run/hooks"
if [ -d "$HOOK_DIR" ] && ls "$HOOK_DIR"/*.sh >/dev/null 2>&1; then
  for hook in $(ls "$HOOK_DIR"/*.sh 2>/dev/null | sort); do
    info "Hook: ${hook##*/}"
    . "$hook"
  done
fi

# ── Environment overrides ─────────────────────────────────────────────────
#
# These env vars can be set in docker-compose.yml or Dockerfile ENV:
#
#   ARGUMENTS         Extra QEMU arguments appended to the command line.
#                     Example: ARGUMENTS="-device usb-host,hostbus=1"
#
#   QEMU_MONITOR_SOCK  Path for the HMP monitor socket.
#                     Default: $QEMU_DIR/monitor.sock (set in config.sh)
#
#   QEMU_QMP_SOCK      Path for the QMP monitor socket.
#                     Default: $QEMU_DIR/qmp.sock (set in config.sh)
#
#   RAM_SIZE           VM RAM. Default: 2G
#   CPU_CORES          VM CPU cores. Default: 2
#   DISK_SIZE          VM disk size. Default: 64G
#   VERSION            OS version to install.
#   CLIPBOARD          Enable shared clipboard. Default: N

info "QEMU customization hooks loaded (0 hooks run)"

return 0
