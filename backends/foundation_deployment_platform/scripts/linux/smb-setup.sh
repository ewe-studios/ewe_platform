#!/bin/bash
# Setup SMB for QEMU on Linux host
# This script configures the host system for QEMU's built-in SMB server

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_SOURCE="${SCRIPT_DIR}/smb.conf"

echo "=== QEMU SMB Setup ==="
echo ""

# Check if running as root for config installation
if [ "$EUID" -ne 0 ]; then
    echo "This script requires sudo to install system-wide Samba configuration."
    echo "Please run: sudo $0"
    exit 1
fi

echo "Creating Samba directories..."
mkdir -p /etc/samba
mkdir -p /var/log/samba
mkdir -p /var/lib/samba/private
mkdir -p /var/cache/samba
mkdir -p /run/samba
mkdir -p /run/samba/ncalrpc

echo "Installing smb.conf..."
if [ -f "$CONFIG_SOURCE" ]; then
    cp "$CONFIG_SOURCE" /etc/samba/smb.conf
else
    # Create default config inline
    cat > /etc/samba/smb.conf << 'INNEREOF'
[global]
workgroup = WORKGROUP
server string = QEMU SMB Server
security = user
map to guest = Bad User

# Logging
log file = /var/log/samba/log.%m
max log size = 1000
logging = file

# QEMU SMB share - path is set dynamically by QEMU
[qemu]
path = %S
read only = no
browseable = yes
guest ok = yes
INNEREOF
fi

chmod 644 /etc/samba/smb.conf

echo "Setting permissions..."
chmod 755 /var/log/samba
chmod 755 /var/lib/samba
chmod 755 /var/cache/samba
chmod 755 /run/samba
chmod 755 /run/samba/ncalrpc

# Set ownership for the user who will run QEMU
# This is needed because smbd runs as the QEMU user, not root
if [ -n "$SUDO_USER" ]; then
    TARGET_USER="$SUDO_USER"
    TARGET_GROUP="$(id -gn $SUDO_USER)"
    chown "$TARGET_USER:$TARGET_GROUP" /var/log/samba
    chown -R "$TARGET_USER:$TARGET_GROUP" /var/lib/samba
    chown "$TARGET_USER:$TARGET_GROUP" /var/cache/samba
    chown -R "$TARGET_USER:$TARGET_GROUP" /run/samba
    echo "Set ownership for user: $TARGET_USER"
fi

echo "Setting smbd capabilities..."
SMBD_PATH=$(which smbd)
if [ -n "$SMBD_PATH" ]; then
    setcap cap_net_bind_service+ep "$SMBD_PATH"
    echo "Added CAP_NET_BIND_SERVICE capability to $SMBD_PATH"
else
    echo "Warning: smbd not found in PATH"
fi

echo "Testing configuration..."
if command -v testparm >/dev/null 2>&1; then
    testparm -s /etc/samba/smb.conf 2>&1 | head -20
else
    echo "testparm not available, skipping validation"
fi

echo ""
echo "=== SMB Setup Complete ==="
echo ""
echo "QEMU's built-in SMB server is now configured."
echo ""
echo "NOTE: Due to a QEMU/Samba compatibility issue, QEMU's built-in SMB"
echo "may not work with modern Samba versions (4.x). The smbd daemon requires"
echo "the 'ncalrpc' subdirectory to exist, which QEMU doesn't create."
echo ""
echo "Workaround: Use a standalone Samba configuration or virtiofs for sharing."
echo ""
echo "Alternative: Manual SMB server startup:"
echo "  1. Create a Samba config file"
echo "  2. Run: sudo smbd -F -s /path/to/smb.conf"
echo "  3. Configure VM to connect to host IP: 10.0.2.2"
