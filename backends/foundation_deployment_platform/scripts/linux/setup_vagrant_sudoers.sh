#!/bin/bash
# Add vagrant user to sudoers with passwordless sudo
# This script should be run after SSH is set up and working

set -e

echo "=== Setting up passwordless sudo for vagrant ==="

# Check if running as root or with sudo
if [ "$EUID" -ne 0 ]; then
    echo "Error: This script must be run as root or with sudo"
    exit 1
fi

# Create sudoers file for vagrant
cat > /etc/sudoers.d/vagrant << 'EOF'
# Allow vagrant user to run all commands without password
vagrant ALL=(ALL) NOPASSWD:ALL

# Disable requiretty for vagrant (needed for non-interactive SSH)
Defaults:vagrant !requiretty
Defaults:vagrant env_keep += "HOME"
Defaults:vagrant env_keep += "PATH"
EOF

# Set correct permissions
chmod 440 /etc/sudoers.d/vagrant

# Validate sudoers file
if visudo -c -f /etc/sudoers.d/vagrant; then
    echo "Sudoers file is valid"
else
    echo "Error: Invalid sudoers file"
    rm -f /etc/sudoers.d/vagrant
    exit 1
fi

# Also ensure /etc/sudoers doesn't have requiretty that would block vagrant
if grep -q "^Defaults.*requiretty" /etc/sudoers 2>/dev/null; then
    echo "Removing requiretty from /etc/sudoers..."
    sed -i '/^Defaults.*requiretty/d' /etc/sudoers
fi

echo "=== Vagrant sudo setup complete ==="
echo "Vagrant can now run: sudo <command> without password"
