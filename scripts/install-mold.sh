#!/bin/bash
set -e

# Script to install mold linker - tries prebuilt binary first, falls back to building from source

MOLD_VERSION="2.4.2"
TARGET="x86_64-unknown-linux-gnu"

echo "=== Installing mold linker ==="

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    TARGET="aarch64-unknown-linux-gnu"
elif [ "$ARCH" = "x86_64" ]; then
    TARGET="x86_64-unknown-linux-gnu"
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

echo "Detected architecture: $ARCH -> target: $TARGET"

# Try to download prebuilt binary from GitHub releases
MOLD_TARBALL="mold-${MOLD_VERSION}-${TARGET}.tar.gz"
MOLD_URL="https://github.com/rui314/mold/releases/download/v${MOLD_VERSION}/${MOLD_TARBALL}"

echo "Attempting to download prebuilt mold from: $MOLD_URL"

if curl -fsSL -o "/tmp/${MOLD_TARBALL}" "$MOLD_URL" 2>/dev/null; then
    echo "Download successful, extracting..."
    cd /tmp
    tar -xzf "${MOLD_TARBALL}"
    cd /tmp/mold-${MOLD_VERSION}-${TARGET}
    cmake --install build --prefix /usr/local
    rm -rf /tmp/mold-* "${MOLD_TARBALL}"
    echo "Mold installed successfully from prebuilt binary"
    mold --version
    exit 0
else
    echo "Failed to download prebuilt binary, building from source..."
fi

# Fall back to building from source
git clone --branch stable --depth=1 https://github.com/rui314/mold.git /tmp/mold
cd /tmp/mold
cmake -DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_COMPILER=c++ -B build
cmake --build build -j$(nproc)
cmake --build build --target install
rm -rf /tmp/mold
echo "Mold built and installed from source"
mold --version
