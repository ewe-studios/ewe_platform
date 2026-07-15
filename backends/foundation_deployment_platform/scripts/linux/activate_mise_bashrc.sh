#!/bin/bash
# Activate mise and add tools to PATH with proper error handling
set -e

echo "=== Activating mise configuration ==="

# Verify mise is installed
if [ ! -f "/home/vagrant/.local/bin/mise" ]; then
    echo "ERROR: mise not found at /home/vagrant/.local/bin/mise"
    exit 1
fi

MISE_EXE="/home/vagrant/.local/bin/mise"
echo "Found mise at: $MISE_EXE"

# User-specific activation in .bashrc and .bash_profile
if ! grep -q 'mise activate' /home/vagrant/.bashrc 2>/dev/null; then
    echo 'eval "$(/home/vagrant/.local/bin/mise activate bash)"' >> /home/vagrant/.bashrc
    echo "Added mise activation to /home/vagrant/.bashrc"
else
    echo "mise activation already in /home/vagrant/.bashrc"
fi

if [ -f /home/vagrant/.bash_profile ]; then
    if ! grep -q 'mise activate' /home/vagrant/.bash_profile 2>/dev/null; then
        echo 'eval "$(/home/vagrant/.local/bin/mise activate bash)"' >> /home/vagrant/.bash_profile
        echo "Added mise activation to /home/vagrant/.bash_profile"
    else
        echo "mise activation already in /home/vagrant/.bash_profile"
    fi
else
    echo 'eval "$(/home/vagrant/.local/bin/mise activate bash)"' > /home/vagrant/.bash_profile
    echo "Created /home/vagrant/.bash_profile with mise activation"
fi

# Create global config in /etc/profile.d for all users (with sudo)
if command -v sudo >/dev/null 2>&1; then
    echo "Creating global profile.d configuration..."

    # Create mise-shims.sh
    echo 'export PATH="/home/vagrant/.local/share/mise/shims:$PATH"' | sudo tee /etc/profile.d/mise-shims.sh > /dev/null
    sudo chmod 644 /etc/profile.d/mise-shims.sh
    echo "Created /etc/profile.d/mise-shims.sh"

    # Create mise.sh
    echo 'eval "$(/home/vagrant/.local/bin/mise activate bash)"' | sudo tee /etc/profile.d/mise.sh > /dev/null
    sudo chmod 644 /etc/profile.d/mise.sh
    echo "Created /etc/profile.d/mise.sh"

    # Ensure /etc/bash.bashrc sources profile.d
    if [ -f /etc/bash.bashrc ]; then
        if ! grep -q "/etc/profile.d" /etc/bash.bashrc 2>/dev/null; then
            echo 'for i in /etc/profile.d/*.sh; do [ -r "$i" ] && . "$i"; done' | sudo tee -a /etc/bash.bashrc > /dev/null
            echo "Added profile.d sourcing to /etc/bash.bashrc"
        fi
    fi

    # Add to /etc/profile for login shells
    if ! grep -q "/etc/profile.d" /etc/profile 2>/dev/null; then
        echo 'for i in /etc/profile.d/*.sh; do [ -r "$i" ] && . "$i"; done' | sudo tee -a /etc/profile > /dev/null
        echo "Added profile.d sourcing to /etc/profile"
    fi
else
    echo "WARNING: sudo not available, skipping global profile.d configuration"
fi

# Verify mise is working
echo "Testing mise..."
if "$MISE_EXE" --version > /dev/null 2>&1; then
    echo "mise is working: $("$MISE_EXE" --version)"
else
    echo "ERROR: mise command failed"
    exit 1
fi

echo "=== Mise activation complete ==="
echo ""
echo "IMPORTANT: Run 'source /home/vagrant/.bashrc' or start a new shell to use mise tools"
