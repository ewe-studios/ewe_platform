#!/bin/bash
# Install GNOME desktop environment for headful VNC/desktop support with auto-login
# Supports: Debian, Ubuntu, Arch Linux

set -e

echo "=== Starting GNOME Installation ==="

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

# Get username (vagrant for test VMs)
VM_USER="${SUDO_USER:-$USER}"
if [ "$VM_USER" = "root" ] || [ -z "$VM_USER" ]; then
    VM_USER="vagrant"
fi

echo "Installing GNOME desktop environment for distro: $DISTRO..."
echo "VM user: $VM_USER"

# Define packages by distro
case "$DISTRO" in
    ubuntu|debian|pop|mint)
        echo "Detected Debian-based distro: $DISTRO"

        # Debian/Ubuntu GNOME packages - minimal GNOME session
        GNOME_DEPS=(
            # Display manager (GDM for GNOME)
            gdm3
            # GNOME Core
            gnome-session
            gnome-shell
            gnome-terminal
            gnome-control-center
            gnome-settings-daemon
            gnome-keyring
            # X server
            xserver-xorg-core
            xserver-xorg-video-fbdev
            xserver-xorg-input-all
            x11-xserver-utils
            # Desktop components
            nautilus
            gnome-calculator
            gnome-system-monitor
            gnome-screenshot
            # Fonts
            fonts-dejavu-core
            fonts-freefont-ttf
            fonts-noto-core
            # Theming
            adwaita-icon-theme
            gnome-themes-standard
            # Utilities
            dbus-x11
            policykit-1
            libnotify-bin
        )

        echo "Updating package list..."
        sudo apt-get update -qq
        echo "Installing GNOME packages (this may take 5-10 minutes)..."
        DEBIAN_FRONTEND=noninteractive sudo apt-get install -y "${GNOME_DEPS[@]}" 2>&1 || {
            echo "Some packages failed to install, continuing..."
        }
        echo "Package installation complete"

        # Configure GDM with auto-login
        echo "Configuring GDM auto-login..."
        sudo tee /etc/gdm3/custom.conf > /dev/null << EOF
[daemon]
AutomaticLoginEnable=true
AutomaticLogin=$VM_USER
WaylandEnable=false
EOF

        # Enable GDM service
        sudo systemctl enable gdm3.service 2>/dev/null || true
        # Set graphical target as default
        sudo systemctl set-default graphical.target 2>/dev/null || true
        ;;

    arch|manjaro|endeavouros)
        echo "Detected Arch-based distro: $DISTRO"

        # Arch Linux GNOME packages
        GNOME_DEPS=(
            # Display manager
            gdm
            # GNOME Core
            gnome-session
            gnome-shell
            gnome-terminal
            gnome-control-center
            gnome-settings-daemon
            gnome-keyring
            # X server
            xorg-server
            xorg-drivers
            xorg-xinit
            # Desktop components
            nautilus
            gnome-calculator
            gnome-system-monitor
            gnome-screenshot
            # Fonts
            ttf-dejavu
            ttf-freefont
            ttf-noto
            # Theming
            adwaita-icon-theme
            gnome-themes-extra
            # Utilities
            dbus
            polkit
            libnotify
        )

        sudo pacman -Sy --noconfirm
        sudo pacman -S --noconfirm "${GNOME_DEPS[@]}" || {
            echo "Some packages failed to install, continuing..."
        }

        # Configure GDM with auto-login
        echo "Configuring GDM auto-login..."
        sudo tee /etc/gdm/custom.conf > /dev/null << EOF
[daemon]
AutomaticLoginEnable=true
AutomaticLogin=$VM_USER
WaylandEnable=false
EOF

        # Enable GDM service
        sudo systemctl enable gdm.service 2>/dev/null || true
        # Set graphical target as default
        sudo systemctl set-default graphical.target 2>/dev/null || true
        ;;

    *)
        if echo "$DISTRO_LIKE" | grep -q "debian"; then
            echo "Detected Debian-based distro (via ID_LIKE): $DISTRO"
            sudo apt-get update -qq
            DEBIAN_FRONTEND=noninteractive sudo apt-get install -y gdm3 gnome-session gnome-shell || true
            sudo systemctl set-default graphical.target 2>/dev/null || true
        elif echo "$DISTRO_LIKE" | grep -q "arch"; then
            echo "Detected Arch-based distro (via ID_LIKE): $DISTRO"
            sudo pacman -Sy --noconfirm
            sudo pacman -S --noconfirm gdm gnome-session gnome-shell || true
            sudo systemctl set-default graphical.target 2>/dev/null || true
        else
            echo "Unknown distro: $DISTRO"
            echo "Please install GNOME packages manually"
            exit 1
        fi
        ;;
esac

echo ""
echo "GNOME desktop environment installed successfully!"
echo ""
echo "GDM display manager is configured with auto-login."
echo "To start the GUI display manager:"
echo "  sudo systemctl start gdm"
echo ""
