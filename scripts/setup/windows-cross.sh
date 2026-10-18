#!/usr/bin/env bash
# Install the Windows cross-compile toolchain (mingw-w64) + the Rust std target
# so the Windows selector in foundation_nativeapis can be compiled on this
# machine (spec-62 F07 validation).
#
# Opt-in: mingw-w64 is a ~a few hundred MB cross-compiler and is only needed to
# compile/validate the `#[cfg(target_os = "windows")]` selector (and any future
# Windows code). `mise run setup` does NOT run this. Run it explicitly with:
#   mise run setup:windows
# or:
#   bash scripts/setup/windows-cross.sh
#
# `cargo check --target x86_64-pc-windows-gnu` only needs the Rust std target
# (no linker); a full `cargo build` additionally needs mingw-w64, installed here.
# The linker entry in `.cargo/config.toml` points at `x86_64-w64-mingw32-gcc`,
# which both brew (macOS) and pacman (Arch) place on PATH.
set -euo pipefail

os="$(uname -s)"
case "$os" in
  Darwin)
    if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
      echo "✓ mingw-w64 already installed"
    else
      command -v brew >/dev/null 2>&1 || {
        echo "Homebrew is required (mise run setup installs it)." >&2
        exit 1
      }
      echo "Installing mingw-w64 (provides x86_64-w64-mingw32-gcc)..."
      brew install mingw-w64
    fi
    ;;
  Linux)
    if [ -f /etc/arch-release ]; then
      if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
        echo "✓ mingw-w64 already installed"
      else
        echo "Installing mingw-w64-gcc..."
        sudo pacman -S --needed --noconfirm mingw-w64-gcc
      fi
    else
      echo "Unsupported Linux distro. Install mingw-w64 (x86_64-w64-mingw32-gcc) manually." >&2
      exit 1
    fi
    ;;
  *)
    echo "Unsupported OS: $os" >&2
    exit 1
    ;;
esac

# Rust std for the target (idempotent; also listed in rust-toolchain.toml).
rustup target add x86_64-pc-windows-gnu

echo ""
echo "✓ Windows cross-compile toolchain ready"
echo "  Validate the Windows selector:"
echo "    cargo check --target x86_64-pc-windows-gnu -p foundation_nativeapis"
