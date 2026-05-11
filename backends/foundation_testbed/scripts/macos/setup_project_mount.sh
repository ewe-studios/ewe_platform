#!/bin/bash
# Set up 9p/virtiofs project mount on macOS guests

set -e

MOUNT_POINT="/Volumes/project"
TAG="project"

# Create mount point with proper permissions
sudo mkdir -p "$MOUNT_POINT"
sudo chmod 755 "$MOUNT_POINT"

# Check if already mounted
if mount | grep -q "$MOUNT_POINT"; then
    echo "Already mounted at $MOUNT_POINT"
    exit 0
fi

# Mount using 9p (virtio-9p)
# Note: macOS doesn't have native 9p support, so this requires macFUSE or similar
# For QEMU/macOS, we use the mount_9p command if available
if command -v mount_9p >/dev/null 2>&1; then
    sudo mount_9p -o trans=virtio "$TAG" "$MOUNT_POINT" || {
        echo "Failed to mount 9p filesystem with mount_9p"
        exit 1
    }
else
    # Fallback: use the standard mount command with 9p
    sudo mount -t 9p -o trans=virtio,version=9p2000.L "$TAG" "$MOUNT_POINT" || {
        echo "Failed to mount 9p filesystem"
        echo "Note: macOS guests may require additional drivers for 9p/virtiofs"
        exit 1
    }
fi

# Add to fstab for persistence (optional - macOS uses a different fstab format)
# For now we skip fstab on macOS as it requires different handling

echo "Project mount ready at $MOUNT_POINT"
