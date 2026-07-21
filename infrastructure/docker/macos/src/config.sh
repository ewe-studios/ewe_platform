#!/usr/bin/env bash
# config.sh — clipboard extension for dockurs/qemu base config.
#
# Sources the original config.sh from the base image and appends
# qemu-vdagent clipboard support (qemu-vdagent chardev + virtio-serial
# channel) so the noVNC browser client can share clipboard with the VM.
#
# The guest OS needs SPICE vdagent installed for clipboard to sync
# inside the guest. For macOS this provides the VNC-side clipboard
# buffer (paste-only without a guest agent).

set -Eeuo pipefail

# Source the original config from the base QEMU image
. /run/base_config.sh

# ── Clipboard via qemu-vdagent ────────────────────────────────────────────────
# Bridges clipboard data between the noVNC client and the VM guest.
# QEMU's VNC server automatically relays clipboard through the vdagent chardev.
#
# Guest requirements:
#   Windows: SPICE Guest Tools (spice-guest-tools)
#   Linux:   spice-vdagent package
#   macOS:   no native vdagent — VNC clipboard buffer works for paste only

if ! disabled "${CLIPBOARD:-Y}"; then
  DEV_OPTS+=" -chardev qemu-vdagent,id=vdagent,clipboard=on,mouse=on"
  DEV_OPTS+=" -device virtio-serial-pci"
  DEV_OPTS+=" -device virtserialport,chardev=vdagent,name=com.redhat.spice.0"
fi

buildArguments

return 0
