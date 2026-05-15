#!/bin/bash
# Install lightweight GUI packages for headful VNC/desktop support with auto-login display manager
# Supports: Debian, Ubuntu, Arch Linux
# This is optional and only needed for interactive GUI testing

# Don't exit on error, we want to see what happens
# set -e

echo "=== Starting GUI Installation ==="

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

echo "Installing lightweight desktop environment for distro: $DISTRO..."
echo "VM user: $VM_USER"

# Define packages by distro
case "$DISTRO" in
    ubuntu|debian|pop|mint)
        echo "Detected Debian-based distro: $DISTRO"

        # Debian/Ubuntu packages - include LightDM display manager with auto-login
        GUI_DEPS=(
            # Display manager
            lightdm
            # X server
            xserver-xorg-core
            xserver-xorg-video-fbdev
            xserver-xorg-input-all
            x11-xserver-utils
            x11-utils
            x11-xkb-utils
            xterm
            xinit
            # Window manager
            openbox
            obconf
            # Panel and accessories
            tint2
            lxappearance
            pcmanfm
            lxterminal
            mousepad
            xfce4-terminal
            thunar
            # Window management tools for Tauri apps
            xdotool
            wmctrl
            feh
            nitrogen
            compton
            dunst
            # System services
            dbus-x11
            policykit-1
            policykit-1-gnome
            gnome-keyring
            libnotify-bin
            # Fonts
            fonts-dejavu-core
            fonts-freefont-ttf
            fonts-noto-core
            # Utilities
            hsetroot
            menu-xdg
        )

        echo "Updating package list..."
        sudo apt-get update -qq
        echo "Installing GUI packages (this may take 5-10 minutes)..."
        DEBIAN_FRONTEND=noninteractive sudo apt-get install -y "${GUI_DEPS[@]}" 2>&1 | tee /tmp/gui_install.log || {
            echo "Some packages failed to install, continuing..."
        }
        echo "Package installation complete"

        # Configure LightDM with auto-login
        echo "Configuring LightDM auto-login..."
        sudo mkdir -p /etc/lightdm/lightdm.conf.d
        sudo tee /etc/lightdm/lightdm.conf.d/99-autologin.conf > /dev/null << EOF
[Seat:*]
autologin-user=$VM_USER
autologin-user-timeout=0
autologin-session=openbox
user-session=openbox
EOF

        # Enable LightDM service
        sudo systemctl enable lightdm.service 2>/dev/null || true
        # Set graphical target as default (boot to GUI instead of TTY)
        sudo systemctl set-default graphical.target 2>/dev/null || true
        ;;

    arch|manjaro|endeavouros)
        echo "Detected Arch-based distro: $DISTRO"

        # Arch Linux packages - include LightDM display manager
        GUI_DEPS=(
            # Display manager
            lightdm
            lightdm-gtk-greeter
            # X server
            xorg-server
            xorg-drivers
            xorg-xinit
            xterm
            # Window manager
            openbox
            obconf
            # Panel and accessories
            tint2
            lxappearance
            pcmanfm-gtk3
            lxterminal
            mousepad
            xfce4-terminal
            thunar
            feh
            nitrogen
            picom
            dunst
            # Window management tools for Tauri apps
            xdotool
            wmctrl
            # System services
            dbus
            polkit
            polkit-gnome
            gnome-keyring
            libnotify
            # Fonts
            ttf-dejavu
            ttf-freefont
            ttf-noto
            # Utilities
            hsetroot
            archlinux-xdg-menu
        )

        sudo pacman -Sy --noconfirm
        sudo pacman -S --noconfirm "${GUI_DEPS[@]}" || {
            echo "Some packages failed to install, continuing..."
        }

        # Configure LightDM with auto-login
        echo "Configuring LightDM auto-login..."
        sudo mkdir -p /etc/lightdm
        sudo tee /etc/lightdm/lightdm.conf > /dev/null << EOF
[Seat:*]
autologin-user=$VM_USER
autologin-user-timeout=0
autologin-session=openbox
user-session=openbox
greeter-session=lightdm-gtk-greeter
EOF

        # Enable LightDM service
        sudo systemctl enable lightdm.service 2>/dev/null || true
        # Set graphical target as default (boot to GUI instead of TTY)
        sudo systemctl set-default graphical.target 2>/dev/null || true
        ;;

    *)
        if echo "$DISTRO_LIKE" | grep -q "debian"; then
            echo "Detected Debian-based distro (via ID_LIKE): $DISTRO"
            sudo apt-get update -qq
            DEBIAN_FRONTEND=noninteractive sudo apt-get install -y lightdm xserver-xorg openbox || true
            # Set graphical target as default
            sudo systemctl set-default graphical.target 2>/dev/null || true
        elif echo "$DISTRO_LIKE" | grep -q "arch"; then
            echo "Detected Arch-based distro (via ID_LIKE): $DISTRO"
            sudo pacman -Sy --noconfirm
            sudo pacman -S --noconfirm lightdm lightdm-gtk-greeter xorg-server openbox || true
            sudo systemctl enable lightdm.service 2>/dev/null || true
            # Set graphical target as default
            sudo systemctl set-default graphical.target 2>/dev/null || true
        else
            echo "Unknown distro: $DISTRO"
            echo "Please install GUI packages manually"
            exit 1
        fi
        ;;
esac

# Configure Openbox for the user
echo "Configuring Openbox desktop..."
USER_HOME=$(eval echo ~$VM_USER)
sudo mkdir -p "$USER_HOME/.config/openbox"

# Create autostart file
cat | sudo tee "$USER_HOME/.config/openbox/autostart" > /dev/null << 'AUTOSTART'
# Openbox autostart - minimal desktop

# Set background color (solid color, nitrogen needs images)
hsetroot -solid "#2E3440" &

# Start panel
tint2 &

# Start notification daemon
dunst &

# Start compositor for transparency
picom -b 2>/dev/null || compton -b 2>/dev/null &

# Set cursor theme
xsetroot -cursor_name left_ptr
AUTOSTART

# Set ownership
sudo chown -R "$VM_USER:$VM_USER" "$USER_HOME/.config"

# Create basic Openbox menu
sudo tee "$USER_HOME/.config/openbox/menu.xml" > /dev/null << 'MENU'
<?xml version="1.0" encoding="UTF-8"?>
<openbox_menu xmlns="http://openbox.org/"
        xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
        xsi:schemaLocation="http://openbox.org/
                file:///usr/share/openbox/menu.xsd">
    <menu id="root-menu" label="Openbox 3">
        <item label="Terminal (Xfce)">
            <action name="Execute">
                <execute>xfce4-terminal</execute>
            </action>
        </item>
        <item label="Terminal (Lite)">
            <action name="Execute">
                <execute>lxterminal</execute>
            </action>
        </item>
        <item label="File Manager (Thunar)">
            <action name="Execute">
                <execute>thunar</execute>
            </action>
        </item>
        <item label="File Manager (PCManFM)">
            <action name="Execute">
                <execute>pcmanfm</execute>
            </action>
        </item>
        <item label="Text Editor">
            <action name="Execute">
                <execute>mousepad</execute>
            </action>
        </item>
        <separator />
        <item label="Tauri E2E Test">
            <action name="Execute">
                <execute>/home/vagrant/.local/bin/launch-tauri-app.sh</execute>
            </action>
        </item>
        <separator />
        <item label="Reconfigure">
            <action name="Reconfigure" />
        </item>
        <item label="Restart">
            <action name="Restart" />
        </item>
        <item label="Exit">
            <action name="Exit" />
        </item>
    </menu>
</openbox_menu>
MENU

# Set ownership
sudo chown -R "$VM_USER:$VM_USER" "$USER_HOME/.config/openbox"

# Create .xsession for the user (fallback for some DMs)
sudo tee "$USER_HOME/.xsession" > /dev/null << 'XSESSION'
#!/bin/sh
exec openbox-session
XSESSION
sudo chmod +x "$USER_HOME/.xsession"
sudo chown "$VM_USER:$VM_USER" "$USER_HOME/.xsession"

# Ensure user is in video group (needed for some display features)
sudo usermod -a -G video "$VM_USER" 2>/dev/null || true

# Install Tauri app launcher script
sudo mkdir -p "$USER_HOME/.local/bin"
cat | sudo tee "$USER_HOME/.local/bin/launch-tauri-app.sh" > /dev/null << 'LAUNCHER'
#!/bin/bash
# Launch Tauri app with proper display settings for VNC/GUI environments

set -e

APP_PATH="${1:-/mnt/project/examples/testbed/tauri-app/target/release/tauri-e2e-test}"
DISPLAY_NUM="${DISPLAY:-:0}"

# Check if app exists
if [ ! -f "$APP_PATH" ]; then
    echo "ERROR: App not found at $APP_PATH"
    echo "Usage: $0 [path/to/app]"
    exit 1
fi

# Ensure DISPLAY is set
export DISPLAY="$DISPLAY_NUM"

# Check if display is available
if ! xset q >/dev/null 2>&1; then
    echo "ERROR: No X11 display available at $DISPLAY"
    echo "Make sure Xvfb or a display server is running"
    exit 1
fi

# Kill any existing instance of the app
APP_NAME=$(basename "$APP_PATH")
pkill -9 "$APP_NAME" 2>/dev/null || true
sleep 0.5

# Launch the app
echo "Launching $APP_NAME on DISPLAY=$DISPLAY..."
"$APP_PATH" &
APP_PID=$!
sleep 2

# Check if window appeared
if command -v xdotool >/dev/null 2>&1; then
    WINDOW_ID=$(xdotool search --name "Tauri" 2>/dev/null | head -1)
    if [ -n "$WINDOW_ID" ]; then
        echo "Window found: $WINDOW_ID"
        # Ensure window is mapped and raised
        xdotool windowmap "$WINDOW_ID" 2>/dev/null || true
        xdotool windowraise "$WINDOW_ID" 2>/dev/null || true
        # Center window on screen
        xdotool windowmove "$WINDOW_ID" 200 100 2>/dev/null || true
    fi
    # Bring to front using wmctrl as well
    wmctrl -r "Tauri" -b add,above 2>/dev/null || true
fi

echo "Tauri app is running. PID: $APP_PID"
echo "To stop: kill $APP_PID"
LAUNCHER

sudo chmod +x "$USER_HOME/.local/bin/launch-tauri-app.sh"
sudo chown "$VM_USER:$VM_USER" "$USER_HOME/.local/bin/launch-tauri-app.sh"

echo "GUI environment installed successfully!"
echo ""
echo "LightDM display manager is configured with auto-login."
echo "To start the GUI display manager:"
echo "  sudo systemctl start lightdm"
echo ""
echo "Or if running manually (e.g., in VNC):"
echo "  startx /usr/bin/openbox-session"
echo ""
echo "To launch the Tauri E2E test app:"
echo "  ~/.local/bin/launch-tauri-app.sh"
