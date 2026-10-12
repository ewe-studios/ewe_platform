#!/bin/bash
# ewe_platform's provisioning layer for the macOS CI VM. Run inside the VM by
# platform-images' provision.sh after layer 1 (Xcode CLT, mise, nushell,
# rustup, the Vulkan SDK), with its environment and install_mise_tools.
#
#   - the toolchain rust-toolchain.toml pins (+ clippy), set as the default
#   - the tools in the repository's mise.toml, from its lock
#   - the submodules at their gitlink SHAs under /usr/local/ewe/submodules,
#     and PLATFORM_IMAGES_SUBMODULES_LOCK for link-submodules.nu
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
SCRIPTS="$PLATFORM_IMAGES_HOME/scripts"

log "ewe_platform: Rust toolchain"
nu "$SCRIPTS/install-rust-toolchain.nu" "$HERE/files/rust-toolchain.toml" --components clippy

log "ewe_platform: repository tools"
install_mise_tools "$HERE/mise"

log "ewe_platform: submodules"
sudo mkdir -p /usr/local/ewe
sudo chown "$(id -un)" /usr/local/ewe
nu "$SCRIPTS/bake-submodules.nu" "$HERE/submodules.lock.json" /usr/local/ewe/submodules
echo 'export PLATFORM_IMAGES_SUBMODULES_LOCK=/usr/local/ewe/submodules/submodules.lock.json' >> /usr/local/platform-images/env.d.sh
