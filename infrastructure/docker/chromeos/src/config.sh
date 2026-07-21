#!/usr/bin/env bash
# config.sh — clipboard extension for ChromeOS QEMU config.
#
# Sources the base image's config.sh (already patched by Dockerfile) and
# appends qemu-vdagent clipboard support so the noVNC browser client can
# share clipboard with the VM.
#
# Note: ChromeOS does not have a SPICE vdagent, so clipboard sync may be
# limited to the VNC protocol level (paste-only without a guest agent).

set -Eeuo pipefail

# Source the original config from the base image (patched by Dockerfile)
. /run/base_config.sh

# ── Clipboard via qemu-vdagent ────────────────────────────────────────────────
if ! disabled "${CLIPBOARD:-Y}"; then
  DEV_OPTS+=" -chardev qemu-vdagent,id=vdagent,clipboard=on,mouse=on"
  DEV_OPTS+=" -device virtio-serial-pci"
  DEV_OPTS+=" -device virtserialport,chardev=vdagent,name=com.redhat.spice.0"
fi

buildArguments

return 0
