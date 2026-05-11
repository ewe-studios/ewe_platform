#!/bin/bash
TAURI_SYSTEM_DEPS="build-essential curl git pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev xvfb scrot openbox"
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y $TAURI_SYSTEM_DEPS
