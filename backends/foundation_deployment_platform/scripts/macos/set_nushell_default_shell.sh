#!/bin/bash
NU_PATH=$(find ~/.local/share/mise/installs/nu -name nu -type f 2>/dev/null | head -1)
if [ -n "$NU_PATH" ]; then
    chsh -s "$NU_PATH" 2>/dev/null || true
fi
