#!/bin/bash
# Install mise tools with verification
set -e

echo "=== Installing mise tools ==="

# Verify mise is installed
if [ ! -f "/home/vagrant/.local/bin/mise" ]; then
    echo "ERROR: mise not found at /home/vagrant/.local/bin/mise"
    echo "Please install mise first"
    exit 1
fi

MISE_EXE="/home/vagrant/.local/bin/mise"
echo "Using mise: $("$MISE_EXE" --version)"

# Create mise config inline (this replaces the {{MISE_TOML}} template)
mkdir -p /home/vagrant/.config/mise
cat > /home/vagrant/.config/mise/config.toml << 'MISECONFIG'
[tools]
rust = "stable"
"aqua:nushell/nushell" = "latest"
cargo-binstall = "latest"
sccache = "latest"
"cargo:tauri-cli" = "2"
MISECONFIG

echo "Created /home/vagrant/.config/mise/config.toml"

# Trust the config
"$MISE_EXE" trust /home/vagrant/.config/mise/config.toml

# Show what will be installed
echo "Installing tools from config..."
"$MISE_EXE" list 2>/dev/null || true

# Install tools
echo "Running mise install..."
"$MISE_EXE" install

# Verify installations
echo ""
echo "=== Verifying installations ==="

FAILED=0

# Check for expected tools (using the actual mise tool names)
for tool in rust "aqua:nushell/nushell" cargo-binstall sccache "cargo:tauri-cli"; do
    if "$MISE_EXE" list "$tool" >/dev/null 2>&1; then
        echo "✓ $tool is installed"
    else
        echo "✗ $tool is NOT installed"
        FAILED=$((FAILED + 1))
    fi
done

# Check shims were created
SHIMS_DIR="/home/vagrant/.local/share/mise/shims"
if [ -d "$SHIMS_DIR" ]; then
    SHIM_COUNT=$(ls -1 "$SHIMS_DIR" 2>/dev/null | wc -l)
    echo "✓ Shims directory exists with $SHIM_COUNT tools"
else
    echo "✗ Shims directory not found at $SHIMS_DIR"
    echo "Creating shims directory..."
    mkdir -p "$SHIMS_DIR"
fi

# Check installs directory
INSTALLS_DIR="/home/vagrant/.local/share/mise/installs"
if [ -d "$INSTALLS_DIR" ]; then
    echo "✓ Installs directory exists"
    ls -la "$INSTALLS_DIR" 2>/dev/null || true
else
    echo "✗ Installs directory not found at $INSTALLS_DIR"
    FAILED=$((FAILED + 1))
fi

# Test that cargo works via mise
echo ""
echo "=== Testing tool execution ==="
if "$MISE_EXE" exec -- cargo --version >/dev/null 2>&1; then
    CARGO_VER=$("$MISE_EXE" exec -- cargo --version 2>/dev/null | head -1)
    echo "✓ cargo works: $CARGO_VER"
else
    echo "✗ cargo command failed"
    FAILED=$((FAILED + 1))
fi

if [ $FAILED -eq 0 ]; then
    echo ""
    echo "=== All mise tools installed successfully ==="
    exit 0
else
    echo ""
    echo "ERROR: $FAILED tool(s) failed to install"
    exit 1
fi
