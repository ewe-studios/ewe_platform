#!/bin/bash
# Start GNOME Display Manager (GDM)
# Supports systemd and manual fallback

set -e

echo "=== Starting Display Manager ==="

# Get username
VM_USER="${SUDO_USER:-$USER}"
if [ "$VM_USER" = "root" ] || [ -z "$VM_USER" ]; then
    VM_USER="vagrant"
fi

echo "Starting display manager for user: $VM_USER"

# Try systemd first
if command -v systemctl >/dev/null 2>&1; then
    echo "Attempting to start GDM via systemd..."
    if sudo systemctl start gdm3 2>/dev/null || sudo systemctl start gdm 2>/dev/null; then
        echo "GDM started via systemd"
        exit 0
    fi
fi

# Check if we can start via service command
if command -v service >/dev/null 2>&1; then
    echo "Trying to start GDM via service command..."
    if sudo service gdm3 start 2>/dev/null || sudo service gdm start 2>/dev/null; then
        echo "GDM started via service command"
        exit 0
    fi
fi

# Manual fallback - start GDM directly
echo "Starting GDM manually..."
if command -v gdm3 >/dev/null 2>&1; then
    sudo gdm3 &
    sleep 2
    echo "GDM3 started manually"
    exit 0
elif command -v gdm >/dev/null 2>&1; then
    sudo gdm &
    sleep 2
    echo "GDM started manually"
    exit 0
fi

echo "WARNING: Could not start GDM display manager"
echo "To start manually: sudo systemctl start gdm3"
exit 1
