#!/usr/bin/env bash
# Install the pinned Android NDK and the Rust std targets for Android
# cross-compilation (spec-55 mobile / spec-62 F08).
#
# Opt-in: the NDK is ~660 MB (Linux) / ~1 GB (macOS), so `mise run setup` does
# NOT run this. Run it explicitly with:
#   mise run setup:android
# or:
#   bash scripts/setup/android-ndk.sh
#
# Installs the NDK to the Android Studio default location. The
# `scripts/android-ndk/*` linkers referenced by `.cargo/config.toml` resolve it
# from there (or from ANDROID_NDK_HOME / ANDROID_NDK_ROOT if you install it
# elsewhere), so no per-machine path edits are needed.
set -euo pipefail

NDK_VERSION="27.0.12077973" # NDK r27 — keep in sync with scripts/android-ndk/cc
NDK_RELEASE="r27"

os="$(uname -s)"
case "$os" in
  Darwin)
    sdk_root="${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}"
    archive="android-ndk-${NDK_RELEASE}-darwin.dmg"
    url="https://dl.google.com/android/repository/$archive"
    ;;
  Linux)
    sdk_root="${ANDROID_SDK_ROOT:-$HOME/Android/sdk}"
    archive="android-ndk-${NDK_RELEASE}-linux.zip"
    url="https://dl.google.com/android/repository/$archive"
    ;;
  *)
    echo "Unsupported OS: $os" >&2
    exit 1
    ;;
esac

ndk_home="$sdk_root/ndk/$NDK_VERSION"

if [ -f "$ndk_home/source.properties" ]; then
  echo "✓ Android NDK r27 already installed at $ndk_home"
else
  tmp="$(mktemp -d)"
  mount_point=""
  cleanup() {
    if [ -n "$mount_point" ]; then hdiutil detach "$mount_point" >/dev/null 2>&1 || true; fi
    rm -rf "$tmp"
  }
  trap cleanup EXIT

  echo "Downloading $url ..."
  curl -fL --retry 3 -o "$tmp/$archive" "$url"

  echo "Extracting NDK to $ndk_home ..."
  case "$os" in
    Darwin)
      # The macOS NDK ships as a .dmg containing AndroidNDK*.app/Contents/NDK.
      mount_point="$(hdiutil attach -nobrowse -readonly "$tmp/$archive" | grep -oE '/Volumes/.*' | head -n 1)"
      app="$(find "$mount_point" -maxdepth 1 -name 'AndroidNDK*.app' -type d | head -n 1)"
      if [ -z "$app" ]; then
        echo "Could not find AndroidNDK*.app in $mount_point" >&2
        exit 1
      fi
      mkdir -p "$(dirname "$ndk_home")"
      cp -R "$app/Contents/NDK" "$ndk_home"
      hdiutil detach "$mount_point" >/dev/null
      mount_point=""
      ;;
    Linux)
      command -v unzip >/dev/null 2>&1 || { echo "unzip is required (sudo pacman -S unzip)" >&2; exit 1; }
      mkdir -p "$(dirname "$ndk_home")"
      unzip -q "$tmp/$archive" -d "$tmp/extracted"
      mv "$tmp/extracted"/android-ndk-* "$ndk_home"
      ;;
  esac
fi

# Rust std for the Android targets (idempotent; also listed in rust-toolchain.toml).
rustup target add aarch64-linux-android x86_64-linux-android armv7-linux-androideabi

# On Apple Silicon the NDK's darwin-x86_64 tools run under Rosetta 2.
if [ "$os" = "Darwin" ] && [ "$(uname -m)" = "arm64" ]; then
  if [ -d "$ndk_home/toolchains/llvm/prebuilt/darwin-x86_64" ] \
     && ! sysctl -n sysctl.proc_translated 2>/dev/null | grep -q 1 \
     && ! arch -x86_64 /usr/bin/true 2>/dev/null; then
    echo "⚠ Rosetta 2 appears to be missing; the NDK's x86_64 macOS tools need it."
    echo "  Install it with: softwareupdate --install-rosetta --agree-to-license"
  fi
fi

echo ""
echo "✓ Android NDK r27 ready at $ndk_home"
echo "  Try: cargo build --target aarch64-linux-android --package foundation_wireguard"
