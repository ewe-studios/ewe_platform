#!/bin/bash
# Install development dependencies for Rust, Tauri, and cross-compilation
# Includes: rustup, llvm, gcc, mise, ARM cross-compilation tools, Tauri system deps

set -e

echo "=== Installing Development Dependencies ==="

# Fix any interrupted dpkg first
echo "Checking dpkg status..."
sudo dpkg --configure -a 2>/dev/null || true

# Update package list
echo "Updating package list..."
sudo apt-get update -qq

# Core build tools and libraries
echo "Installing core build tools..."
DEBIAN_FRONTEND=noninteractive sudo apt-get install -y \
    build-essential \
    cmake \
    meson \
    ninja-build \
    git \
    curl \
    wget \
    pkg-config \
    libssl-dev \
    || true

# LLVM and Clang toolchain
echo "Installing LLVM/Clang..."
DEBIAN_FRONTEND=noninteractive sudo apt-get install -y \
    clang \
    lld \
    llvm \
    libclang-dev \
    || true

# GCC and cross-compilation tools for ARM
echo "Installing GCC and ARM cross-compilation tools..."
DEBIAN_FRONTEND=noninteractive sudo apt-get install -y \
    gcc \
    g++ \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    gcc-arm-linux-gnueabihf \
    g++-arm-linux-gnueabihf \
    libc6-dev-arm64-cross \
    libc6-dev-armhf-cross \
    || true

# Tauri system dependencies (WebKit, GTK, etc.)
echo "Installing Tauri system dependencies..."
DEBIAN_FRONTEND=noninteractive sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libxdo-dev \
    libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev \
    || true

# X11 and display support for headless/headful testing
echo "Installing X11/display dependencies..."
DEBIAN_FRONTEND=noninteractive sudo apt-get install -y \
    xvfb \
    x11-utils \
    x11-xserver-utils \
    scrot \
    openbox \
    dbus-x11 \
    || true

# Additional development tools
echo "Installing additional dev tools..."
DEBIAN_FRONTEND=noninteractive sudo apt-get install -y \
    file \
    lsb-release \
    software-properties-common \
    apt-transport-https \
    ca-certificates \
    gnupg \
    || true

echo "=== Development Dependencies Installation Complete ==="
