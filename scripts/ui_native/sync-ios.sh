#!/usr/bin/env bash
# Keep generated Swift wire readers, native bindings and reference fixtures in sync.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
swift_package="$root/mobile/ios"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
"$root/scripts/ui_native/generate-bindings.sh" --out "$stage/bindings"
mkdir -p "$stage/EweCore" "$stage/Fixtures"
cp "$stage/bindings/swift/EweCore.swift" "$stage/EweCore/"
wire="$root/backends/foundation_ui_wire"
cp "$wire/fixtures/native/"* "$stage/Fixtures/"
for file in frame_ops_all_forms.bin frame_state_custom.bin state_counter_custom.json action_increment_custom.bin; do
    cp "$wire/fixtures/$file" "$stage/Fixtures/"
done
sync_dir() {
    case "${3:-}" in
        --check) diff -ru "$1" "$2" ;;
        '') mkdir -p "$2"; rsync -a --delete "$1/" "$2/" ;;
        *) echo 'Expected --check or no argument.' >&2; exit 2 ;;
    esac
}
sync_dir "$wire/generated/swift" "$swift_package/Sources/EweWire" "${1:-}"
sync_dir "$stage/EweCore" "$swift_package/Sources/EweCore" "${1:-}"
sync_dir "$stage/Fixtures" "$swift_package/Tests/EweUITests/Fixtures" "${1:-}"
