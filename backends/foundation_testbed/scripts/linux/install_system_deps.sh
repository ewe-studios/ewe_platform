#!/bin/bash
# System dependencies for building Tauri apps - distro-aware
# Supports: Debian, Ubuntu, Arch Linux

set -e

# Detect distro
if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO=$ID
    DISTRO_LIKE=$ID_LIKE
elif [ -f /etc/debian_version ]; then
    DISTRO="debian"
else
    DISTRO="unknown"
fi

# Common package names by distro
# Debian/Ubuntu
DEBIAN_DEPS="{{DEPS}}"

# Arch Linux equivalents (pacman)
ARCH_DEPS=(
    base-devel curl git pkg-config clang lld
    webkit2gtk gtk3 libayatana-appindicator librsvg openssl libxdo
    libsoup3 javascriptcoregtk libsoup gtk4
    xvfb scrot openbox
)

case "$DISTRO" in
    ubuntu|debian|pop|mint)
        echo "Detected Debian-based distro: $DISTRO"
        echo "Updating package list..."
        sudo apt-get update -qq
        echo "Installing dependencies..."
        DEBIAN_FRONTEND=noninteractive sudo apt-get install -y $DEBIAN_DEPS
        ;;

    arch|manjaro|endeavouros)
        echo "Detected Arch-based distro: $DISTRO"
        echo "Updating package database..."
        sudo pacman -Sy --noconfirm
        echo "Installing dependencies..."
        sudo pacman -S --noconfirm "${ARCH_DEPS[@]}"
        ;;

    *)
        if echo "$DISTRO_LIKE" | grep -q "debian"; then
            echo "Detected Debian-based distro (via ID_LIKE): $DISTRO"
            sudo apt-get update -qq
            DEBIAN_FRONTEND=noninteractive sudo apt-get install -y $DEBIAN_DEPS
        elif echo "$DISTRO_LIKE" | grep -q "arch"; then
            echo "Detected Arch-based distro (via ID_LIKE): $DISTRO"
            sudo pacman -Sy --noconfirm
            sudo pacman -S --noconfirm "${ARCH_DEPS[@]}"
        else
            echo "Unknown distro: $DISTRO"
            echo "Attempting Debian/Ubuntu style..."
            sudo apt-get update -qq 2>/dev/null || true
            DEBIAN_FRONTEND=noninteractive sudo apt-get install -y $DEBIAN_DEPS 2>/dev/null || {
                echo "Failed to install dependencies. Please install manually:"
                echo "$DEBIAN_DEPS"
                exit 1
            }
        fi
        ;;
esac

echo "System dependencies installed successfully."
