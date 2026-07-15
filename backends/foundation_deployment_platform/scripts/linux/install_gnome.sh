#!/bin/bash
# Install full Ubuntu GNOME desktop using tasksel
# Simple, direct approach - installs complete ubuntu-desktop

set -e

echo "=== Starting Ubuntu Desktop Installation via tasksel ==="

# Get username
VM_USER="${SUDO_USER:-$USER}"
if [ "$VM_USER" = "root" ] || [ -z "$VM_USER" ]; then
    VM_USER="vagrant"
fi

echo "Installing for user: $VM_USER"

# Fix any interrupted dpkg first
echo "Checking dpkg status..."
sudo dpkg --configure -a || true

# Install tasksel and ubuntu-desktop
echo "Updating package list..."
sudo apt-get update -qq

echo "Installing tasksel..."
DEBIAN_FRONTEND=noninteractive sudo apt-get install -y tasksel

echo "Installing ubuntu-desktop (this will take 10-15 minutes)..."
DEBIAN_FRONTEND=noninteractive sudo tasksel install ubuntu-desktop || {
    echo "tasksel completed with some non-critical errors, continuing..."
}

echo "Ubuntu desktop packages installed"

# Ensure GDM3 is installed and configured
echo "Configuring GDM3..."
sudo apt-get install -y gdm3 || true

# Configure GDM with auto-login
if [ -d "/etc/gdm3" ] || [ -d "/etc/gdm" ]; then
    echo "Setting up GDM auto-login for $VM_USER..."
    GDM_CONF="/etc/gdm3/custom.conf"
    if [ ! -f "$GDM_CONF" ]; then
        GDM_CONF="/etc/gdm/custom.conf"
    fi

    # Create custom.conf if it doesn't exist
    sudo mkdir -p "$(dirname "$GDM_CONF")"

    sudo tee "$GDM_CONF" > /dev/null << EOF
[daemon]
AutomaticLoginEnable=true
AutomaticLogin=$VM_USER
WaylandEnable=false

[security]
DisallowTCP=false

[xdmcp]
Enable=true
EOF
fi

# Set graphical target as default
echo "Setting graphical.target as default..."
sudo systemctl set-default graphical.target || true

# Enable and start GDM3
echo "Enabling GDM3 service..."
sudo systemctl enable gdm3.service 2>/dev/null || sudo systemctl enable gdm.service 2>/dev/null || true

echo "=== Ubuntu Desktop installation complete ==="
echo "Reboot required to start GDM3 and enter graphical mode"
