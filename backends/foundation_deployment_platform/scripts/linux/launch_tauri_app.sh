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
WINDOW_ID=$(xdotool search --name "Tauri" 2>/dev/null | head -1)
if [ -n "$WINDOW_ID" ]; then
    echo "Window found: $WINDOW_ID"
    # Ensure window is mapped and raised
    xdotool windowmap "$WINDOW_ID" 2>/dev/null || true
    xdotool windowraise "$WINDOW_ID" 2>/dev/null || true
    # Center window on screen
    xdotool windowmove "$WINDOW_ID" 200 100 2>/dev/null || true
    echo "App launched successfully (PID: $APP_PID)"
else
    echo "Warning: Window not found, but app may still be running (PID: $APP_PID)"
fi

# Bring to front using wmctrl as well
wmctrl -r "Tauri" -b add,above 2>/dev/null || true

echo "Tauri app is running. PID: $APP_PID"
echo "To stop: kill $APP_PID"
