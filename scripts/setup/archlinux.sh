#!/usr/bin/env bash
# scripts/setup/archlinux.sh — Arch Linux development setup for ewe_platform.
# Run via `mise run setup` (or directly).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

log() { printf '[setup:arch] %s\n' "$*"; }

[ "$(uname -s)" = Linux ] || { echo "archlinux.sh is for Linux" >&2; exit 1; }
[ -f /etc/arch-release ] || { echo "archlinux.sh is for Arch Linux" >&2; exit 1; }

log "Installing native packages with pacman..."
# Maps the CI Linux build list (ci/platform-images/images/linux/packages/10-build.txt).
# cmake, ninja and protoc come from mise ([tools]) so every platform pins the
# same versions. libclang is the bindgen dependency; clang+lld are the
# `.cargo/config.toml` linker.
sudo pacman -S --needed --noconfirm \
    base-devel clang lld llvm libclang libomp pkg-config make \
    openssl zlib zstd xz bzip2 sqlite curl \
    python python-pip git git-lfs

log "Native packages installed"

# --- shared, platform-independent steps -------------------------------------
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"
install_rust_components
install_wasm_targets
init_submodules
install_pnpm_deps

log "Arch Linux setup complete"
