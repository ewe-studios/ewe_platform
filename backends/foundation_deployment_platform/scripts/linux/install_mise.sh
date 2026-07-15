#!/bin/bash
set -euo pipefail

# Idempotent check: skip if mise is already installed
if [ -f "/home/vagrant/.local/bin/mise" ]; then
    echo "mise already installed: $(/home/vagrant/.local/bin/mise --version)"
    exit 0
fi

echo "Starting mise install..." >&2

# Check prerequisites
echo "Checking curl..." >&2
which curl >/dev/null 2>&1 || { echo "ERROR: curl not found" >&2; exit 1; }
echo "Checking internet connectivity..." >&2
curl -fsSL https://mise.run >/dev/null 2>&1 || { echo "ERROR: Cannot reach mise.run (no internet?)" >&2; exit 1; }

# Install mise
echo "Downloading and running mise installer..." >&2
if ! curl -fsSL https://mise.run | sh; then
    echo "ERROR: mise install script failed" >&2
    exit 1
fi

# Verify mise was installed
echo "Verifying mise installation..." >&2
if [ ! -f "/home/vagrant/.local/bin/mise" ]; then
    echo "ERROR: mise binary not found at /home/vagrant/.local/bin/mise after install" >&2
    # Check if it was installed somewhere else
    echo "Searching for mise binary..." >&2
    find /home/vagrant -name "mise" -type f 2>/dev/null | head -5 || true
    exit 1
fi

# Make sure it's executable
chmod +x "/home/vagrant/.local/bin/mise"

# Verify it works
echo "Testing mise binary..." >&2
if ! "/home/vagrant/.local/bin/mise" --version >/dev/null 2>&1; then
    echo "ERROR: mise binary installed but cannot execute" >&2
    # Try to see what the file is
    file "/home/vagrant/.local/bin/mise" >&2 || true
    ls -la "/home/vagrant/.local/bin/mise" >&2 || true
    exit 1
fi

echo "mise installed successfully: $("/home/vagrant/.local/bin/mise" --version)" >&2
