#!/bin/bash
# Install lightweight GUI packages for headful VNC/desktop support
# Supports: Debian, Ubuntu, Arch Linux
# This is optional and only needed for interactive GUI testing

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

echo "Installing lightweight desktop environment for distro: $DISTRO..."

# Define packages by distro
case "$DISTRO" in
    ubuntu|debian|pop|mint)
        echo "Detected Debian-based distro: $DISTRO"

        # Debian/Ubuntu packages
        GUI_DEPS=(
            xserver-xorg-core
            xserver-xorg-video-fbdev
            x11-xserver-utils
            x11-utils
            xterm
            openbox
            obconf
            tint2
            lxappearance
            pcmanfm
            lxterminal
            leafpad
            feh
            nitrogen
            compton
            dunst
            slim
            dbus-x11
            policykit-1-gnome
            gnome-keyring
            libnotify-bin
            fonts-dejavu-core
            fonts-freefont-ttf
            fonts-noto-core
        )

        sudo apt-get update -qq
        DEBIAN_FRONTEND=noninteractive sudo apt-get install -y "${GUI_DEPS[@]}" || {
            echo "Some packages failed to install, continuing..."
        }
        ;;

    arch|manjaro|endeavouros)
        echo "Detected Arch-based distro: $DISTRO"

        # Arch Linux packages
        GUI_DEPS=(
            xorg-server
            xorg-drivers
            xorg-xinit
            xterm
            openbox
            obconf
            tint2
            lxappearance
            pcmanfm-gtk3
            lxterminal
            leafpad
            feh
            nitrogen
            picom
            dunst
            slim
            dbus
            polkit-gnome
            gnome-keyring
            libnotify
            ttf-dejavu
            ttf-freefont
            ttf-noto
        )

        sudo pacman -Sy --noconfirm
        sudo pacman -S --noconfirm "${GUI_DEPS[@]}" || {
            echo "Some packages failed to install, continuing..."
        }
        ;;

    *)
        if echo "$DISTRO_LIKE" | grep -q "debian"; then
            echo "Detected Debian-based distro (via ID_LIKE): $DISTRO"
            sudo apt-get update -qq
            DEBIAN_FRONTEND=noninteractive sudo apt-get install -y xserver-xorg openbox || true
        elif echo "$DISTRO_LIKE" | grep -q "arch"; then
            echo "Detected Arch-based distro (via ID_LIKE): $DISTRO"
            sudo pacman -Sy --noconfirm
            sudo pacman -S --noconfirm xorg-server openbox || true
        else
            echo "Unknown distro: $DISTRO"
            echo "Please install GUI packages manually"
            exit 1
        fi
        ;;
esac

# Configure Openbox as default window manager for the user
mkdir -p ~/.config/openbox
cat > ~/.config/openbox/autostart << 'AUTOSTART'
# Openbox autostart - minimal desktop
# Set background
nitrogen --restore &
# Start panel
tint2 &
# Start notification daemon
dunst &
# Start compositor for transparency
picom -b &
AUTOSTART

# Create basic Openbox menu
cat > ~/.config/openbox/menu.xml << 'MENU'
<?xml version="1.0" encoding="UTF-8"?>
<openbox_menu xmlns="http://openbox.org/"
        xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
        xsi:schemaLocation="http://openbox.org/
                file:///usr/share/openbox/menu.xsd">
    <menu id="root-menu" label="Openbox 3">
        <item label="Terminal">
            <action name="Execute">
                <execute>lxterminal</execute>
            </action>
        </item>
        <item label="File Manager">
            <action name="Execute">
                <execute>pcmanfm</execute>
            </action>
        </item>
        <item label="Text Editor">
            <action name="Execute">
                <execute>leafpad</execute>
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

echo "GUI environment installed."
echo "Start with: startx /usr/bin/openbox-session"
