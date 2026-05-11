#!/bin/bash
# Set up 9p/virtiofs project mount on Linux guests

set -e

MOUNT_POINT="/mnt/project"
TAG="project"

# Create mount point
sudo mkdir -p "$MOUNT_POINT"

# Mount using 9p (virtio-9p)
if ! mount | grep -q "$MOUNT_POINT"; then
    sudo mount -t 9p -o trans=virtio,version=9p2000.L "$TAG" "$MOUNT_POINT" || {
        echo "Failed to mount 9p filesystem"
        exit 1
    }
fi

# Add to fstab for persistence across reboots
if ! grep -q "$TAG" /etc/fstab 2>/dev/null; then
    echo "$TAG $MOUNT_POINT 9p trans=virtio,version=9p2000.L 0 0" | sudo tee -a /etc/fstab > /dev/null
fi

echo "Project mount ready at $MOUNT_POINT"
