#!/usr/bin/env bash
# scripts/setup/macos.sh — macOS development setup for ewe_platform.
# Run via `mise run setup` (or directly). bash 3.2 safe.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

log() { printf '[setup:macos] %s\n' "$*"; }
fail() { log "ERROR: $*"; exit 1; }

[ "$(uname -s)" = Darwin ] || fail "macos.sh is for macOS"

# --- Xcode Command Line Tools (clang/cc, git, make, the macOS SDK) ----------
if xcode-select -p >/dev/null 2>&1; then
    log "Xcode Command Line Tools present: $(xcode-select -p)"
else
    log "Installing Xcode Command Line Tools (a dialog will appear)..."
    xcode-select --install
    echo "Follow the macOS prompt to install the Command Line Tools, then re-run:"
    echo "  mise run setup"
    exit 0
fi

# --- Homebrew ---------------------------------------------------------------
if ! command -v brew >/dev/null 2>&1; then
    log "Installing Homebrew..."
    env NONINTERACTIVE=1 /bin/bash -c \
        "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    # brew lands in /opt/homebrew (Apple Silicon) or /usr/local (Intel).
    for prefix in /opt/homebrew /usr/local; do
        if [ -x "$prefix/bin/brew" ]; then
            eval "$("$prefix/bin/brew" shellenv)"
            break
        fi
    done
    command -v brew >/dev/null 2>&1 || fail "Homebrew installed but brew is not on PATH"
fi
log "Homebrew: $(brew --version | head -1)"

# --- native libraries -------------------------------------------------------
log "Installing native libraries (llvm for libclang, openssl, pkg-config)..."
brew install llvm openssl pkg-config

# --- libclang discovery (bindgen) -------------------------------------------
# bindgen links libclang through clang-sys. Xcode CLT alone has no libclang;
# brew's llvm is keg-only, so point clang-sys at it via LIBCLANG_PATH.
LLVM_PREFIX="$(brew --prefix llvm)"
export LIBCLANG_PATH="$LLVM_PREFIX/lib"
[ -f "$LIBCLANG_PATH/libclang.dylib" ] || fail "libclang.dylib not found under $LIBCLANG_PATH"
log "LIBCLANG_PATH=$LIBCLANG_PATH"

# Persist it for ordinary `cargo build` runs (the default shell is zsh on macOS).
if [ -f "$HOME/.zshrc" ] && ! grep -q 'LIBCLANG_PATH' "$HOME/.zshrc"; then
    {
        echo ""
        echo "# ewe_platform: libclang for bindgen (brew llvm, keg-only)"
        echo "export LIBCLANG_PATH=\"$LLVM_PREFIX/lib\""
    } >> "$HOME/.zshrc"
    log "Appended LIBCLANG_PATH export to ~/.zshrc"
fi

# --- shared, platform-independent steps -------------------------------------
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"
install_rust_components
install_wasm_targets
init_submodules
install_pnpm_deps

log "macOS setup complete"
