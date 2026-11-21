#!/usr/bin/env bash
# Install both Android todo apps on one device and check their task stores.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
exec "$root/scripts/ui_native/release/test-android-starter.sh" --todo "$@"
