#!/usr/bin/env bash
# config.sh — clipboard extension for dockurr/windows-arm base config.
#
# Sources the original config.sh from the base image and appends
# qemu-vdagent clipboard support (qemu-vdagent chardev + virtio-serial
# channel) so the noVNC browser client can share clipboard with the VM.

set -Eeuo pipefail

# Source the original config from the base image
. /run/base_config.sh

# ── Clipboard via qemu-vdagent ────────────────────────────────────────────────
if ! disabled "${CLIPBOARD:-Y}"; then
  DEV_OPTS+=" -chardev qemu-vdagent,id=vdagent,clipboard=on,mouse=on"
  DEV_OPTS+=" -device virtio-serial-pci"
  DEV_OPTS+=" -device virtserialport,chardev=vdagent,name=com.redhat.spice.0"
fi

buildArguments

return 0
