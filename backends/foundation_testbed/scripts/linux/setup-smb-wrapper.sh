#!/bin/bash
# QEMU SMB Wrapper - Alternative approach using exec replacement
# This creates a fake 'smbd' that QEMU will spawn, which then creates
# the required directories and execs the real smbd

set -e

REAL_SMBD="/bin/smbd"
WRAPPER_DIR="${HOME}/.local/bin"
WRAPPER_SMBD="${WRAPPER_DIR}/smbd"

echo "=== QEMU SMB Wrapper Setup ==="

# Create wrapper directory
mkdir -p "$WRAPPER_DIR"

# Create the wrapper script
cat > "$WRAPPER_SMBD" << 'WRAPPER_EOF'
#!/bin/bash
# Auto-generated smbd wrapper for QEMU SMB compatibility
# Creates required ncalrpc directory before running real smbd

# Find the real smbd
REAL_SMBD="/bin/smbd"
for path in /usr/bin/smbd /usr/local/bin/smbd; do
    if [ -x "$path" ]; then
        REAL_SMBD="$path"
        break
    fi
done

# If called with -s (config file), ensure ncalrpc directory exists
if [ "$1" = "-s" ] && [ -n "$2" ]; then
    SMB_CONF="$2"
    SMB_DIR=$(dirname "$SMB_CONF")

    # Create required directories
    if [ -d "$SMB_DIR" ]; then
        mkdir -p "$SMB_DIR/ncalrpc"
        mkdir -p "$SMB_DIR/cores"
        chmod 700 "$SMB_DIR/cores" 2>/dev/null || true
        chmod 755 "$SMB_DIR"
    fi
fi

# Exec the real smbd with all original arguments
exec "$REAL_SMBD" "$@"
WRAPPER_EOF

chmod +x "$WRAPPER_SMBD"

echo "Wrapper created at: $WRAPPER_SMBD"
echo ""
echo "=== Usage ==="
echo "Prepend the wrapper directory to your PATH when running QEMU:"
echo ""
echo '  export PATH="'"$WRAPPER_DIR"':$PATH"'
echo '  qemu-system-x86_64 -netdev "user,id=net,smb=/path/to/share" ...'
echo ""
echo "Or run QEMU with modified PATH directly:"
echo ""
echo '  PATH="'"$WRAPPER_DIR"':$PATH" qemu-system-x86_64 -netdev "user,id=net,smb=/path/to/share" ...'
echo ""
echo "=== Permanent Setup ==="
echo "Add this to your /home/vagrant/.bashrc or /home/vagrant/.zshrc:"
echo '  export PATH="'"$WRAPPER_DIR"':$PATH"'

