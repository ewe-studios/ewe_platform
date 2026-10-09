#!/bin/bash
set -eu
# Create pre-built AVDs. Called at Docker build time.
# AVD name → SDK device name (from `avdmanager list device`)
PACKAGE="system-images;android-${ANDROID_API_LEVEL:-34};default;x86_64"

create_avd() {
    local name="$1"
    local device="$2"
    echo "  Creating AVD: $name (device: $device)"
    echo "no" | avdmanager create avd \
        --name "$name" \
        --package "$PACKAGE" \
        --device "$device" \
        --force > /dev/null 2>&1
}

create_avd pixel_6  pixel_6
create_avd nexus_5  "Nexus 5"
create_avd pixel_c  pixel_c
create_avd nexus_7  "Nexus 7"

echo "AVDs created:"
avdmanager list avd 2>/dev/null | grep "Name:" | sed 's/.*Name: /  /'
