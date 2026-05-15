#!/bin/bash
# Set nushell as default shell
NU_PATH=$(find "/home/vagrant/.local/share/mise/installs" -name nu -type f 2>/dev/null | head -1)
if [ -n "$NU_PATH" ]; then
    echo "Setting nushell as default shell: $NU_PATH"
    sudo chsh -s "$NU_PATH" "$USER" 2>/dev/null || echo "Note: Could not set default shell (may require password)"
else
    echo "nushell not found in mise installs"
fi
