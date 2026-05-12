#!/bin/bash
# Start the display manager (LightDM) for GUI auto-login
# This script starts LightDM which provides the graphical login screen with auto-login

set -e

# Detect if we're in a VM or container
if [ -f /etc/os-release ]; then
    . /etc/os-release
fi

# Get the user to auto-login
VM_USER="${1:-$SUDO_USER}"
if [ -z "$VM_USER" ] || [ "$VM_USER" = "root" ]; then
    VM_USER="vagrant"
fi

echo "Starting display manager for user: $VM_USER"

# Check if LightDM is installed
if ! command -v lightdm >/dev/null 2>&1; then
    echo "ERROR: LightDM is not installed. Run the GUI install first."
    echo "       TESTBED_INSTALL_GUI=1 cargo test ..."
    exit 1
fi

# Check if X server is available
if ! command -v Xorg >/dev/null 2>&1 && ! command -v X >/dev/null 2>&1; then
    echo "ERROR: X server is not installed."
    exit 1
fi

# Make sure the user has a valid session configured
USER_HOME=$(eval echo ~$VM_USER 2>/dev/null || echo "/home/$VM_USER")
if [ ! -d "$USER_HOME" ]; then
    echo "WARNING: Home directory not found for $VM_USER, using /tmp"
    USER_HOME="/tmp"
fi

# Ensure proper permissions for X server
# Some systems need this for non-root X access
if [ -d /tmp/.X11-unix ]; then
    sudo chmod 1777 /tmp/.X11-unix 2>/dev/null || true
fi

# Method 1: Try to start LightDM via systemd (if available)
if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    echo "Attempting to start LightDM via systemd..."
    if sudo systemctl start lightdm 2>/dev/null; then
        echo "LightDM started via systemd"
        echo "GUI should be available. If using VNC, connect now."
        exit 0
    else
        echo "systemctl start lightdm failed, trying manual start..."
    fi
fi

# Method 2: Start LightDM manually
# This is useful for containers or VMs without full systemd
if [ -f /etc/lightdm/lightdm.conf ] || [ -f /etc/lightdm/lightdm.conf.d/*.conf ]; then
    echo "Starting LightDM manually..."

    # Export required environment variables
    export DISPLAY=:0
    export XAUTHORITY="$USER_HOME/.Xauthority"

    # Start LightDM in background
    sudo -E lightdm --test-mode &
    LIGHTDM_PID=$!

    # Wait a moment for it to start
    sleep 2

    if kill -0 $LIGHTDM_PID 2>/dev/null; then
        echo "LightDM started (PID: $LIGHTDM_PID)"
        echo "GUI should be available shortly."
        echo "If using VNC, connect to the display."

        # Keep the script running if called directly
        if [ -t 0 ]; then
            echo ""
            echo "Press Ctrl+C to stop LightDM"
            wait $LIGHTDM_PID
        fi
        exit 0
    else
        echo "LightDM failed to start, trying direct X session..."
    fi
else
    echo "LightDM config not found, trying direct X session..."
fi

# Method 3: Fall back to direct startx
# This is the most basic method but requires the user to be logged in
if command -v startx >/dev/null 2>&1; then
    echo "Starting X session directly with startx..."
    echo "Note: This requires the user to be logged in at a TTY."

    # Check if openbox session exists
    if [ -f /usr/bin/openbox-session ] || [ -f /usr/local/bin/openbox-session ]; then
        SESSION="openbox-session"
    else
        SESSION="xterm"
    fi

    echo "To start the GUI manually, run:"
    echo "  sudo -u $VM_USER startx /usr/bin/$SESSION -- :0"

    # If we have a TTY, try to start it
    if [ -t 0 ] && [ -n "$DISPLAY" ]; then
        echo "Starting $SESSION..."
        sudo -u "$VM_USER" startx /usr/bin/$SESSION -- :0 vt7 &
    fi

    exit 0
fi

echo "ERROR: Could not start display manager."
echo "Make sure GUI packages are installed with: TESTBED_INSTALL_GUI=1"
exit 1
