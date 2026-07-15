#!/bin/bash
# QEMU SMB Helper - Wraps smbd to create required directories
# Place this script in PATH before /bin/smbd
#
# Usage: Add to PATH: export PATH="/path/to/this/script/dir:$PATH"
# Then run QEMU normally: qemu-system-x86_64 -netdev user,id=net,smb=/path/to/share ...

# If this is being called by QEMU (which passes -s /path/to/smb.conf)
# we need to create the ncalrpc directory before running real smbd

if [ "$1" = "-s" ] && [ -n "$2" ]; then
    SMB_CONF="$2"
    SMB_DIR=$(dirname "$SMB_CONF")

    # Create the ncalrpc subdirectory if it doesn't exist
    if [ ! -d "$SMB_DIR/ncalrpc" ]; then
        mkdir -p "$SMB_DIR/ncalrpc"
        mkdir -p "$SMB_DIR/cores"
        chmod 700 "$SMB_DIR/cores"
    fi
fi

# Run the real smbd
exec /bin/smbd "$@"
