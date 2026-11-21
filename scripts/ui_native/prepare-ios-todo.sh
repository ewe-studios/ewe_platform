#!/usr/bin/env bash
# Stage one iPhone package that contains the renderer and the app's Rust actions.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
sync_bridge=false
if [[ ${1:-} == --sync-bridge ]]; then
    sync_bridge=true
    shift
fi
out=${1:-$root/target/native-ui/ios-todo}
target_dir=${CARGO_TARGET_DIR:-$root/target}
mkdir -p "$out"
out=$(cd "$out" && pwd)
cd "$root"
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
arch=$(uname -m)
case "$arch" in arm64) target=aarch64-apple-ios-sim ;; x86_64) target=x86_64-apple-ios ;; *) echo "Unsupported simulator CPU $arch" >&2; exit 1 ;; esac
cargo build --locked --profile uat -p ui_todo_native -p foundation_ui_native --features foundation_ui_native/bindgen
stage=$(mktemp -d "$out/.build-XXXXXX")
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/headers" "$out/examples/ui_todo/ios/Bridge" "$out/mobile/ios"
for crate in foundation_ui_native ui_todo_native; do
    "$target_dir/uat/uniffi-bindgen" generate --library "$target_dir/uat/libui_todo_native.dylib" --crate "$crate" \
        --language swift --out-dir "$stage/generated" --no-format
done
# The generated application module references the shared native module.
bridge="$root/examples/ui_todo/ios/Bridge/EweTodoCore.swift"
{
    printf 'import EweCore\nimport EweCoreFFI\n'
    cat "$stage/generated/EweTodoCore.swift"
} > "$stage/EweTodoCore.swift"
if [[ "$sync_bridge" == true ]]; then
    cp "$stage/EweTodoCore.swift" "$bridge"
    exit 0
fi
diff -u "$bridge" "$stage/EweTodoCore.swift"
cp "$stage/generated/"*.h "$stage/headers/"
cat > "$stage/headers/module.modulemap" <<'MAP'
module EweCoreFFI {
    header "EweCoreFFI.h"
    header "EweTodoCoreFFI.h"
    export *
}
MAP
build_slice() {
    local sdk=$1 rust_target=$2
    SDKROOT=$(xcrun --sdk "$sdk" --show-sdk-path) IPHONEOS_DEPLOYMENT_TARGET=17.0 \
        cargo build --locked --release -p ui_todo_native --lib --target "$rust_target"
    mkdir -p "$stage/$sdk"
    ln "$target_dir/$rust_target/release/libui_todo_native.a" "$stage/$sdk/libEweCoreFFI.a" ||
        cp "$target_dir/$rust_target/release/libui_todo_native.a" "$stage/$sdk/libEweCoreFFI.a"
}
build_slice iphonesimulator "$target"
build_slice iphoneos aarch64-apple-ios
xcodebuild -create-xcframework \
    -library "$stage/iphoneos/libEweCoreFFI.a" -headers "$stage/headers" \
    -library "$stage/iphonesimulator/libEweCoreFFI.a" -headers "$stage/headers" \
    -output "$stage/EweCoreFFI.xcframework"
mkdir -p "$out/.build-inputs"
if [[ -d "$out/.build-inputs/EweCoreFFI.xcframework" ]]; then
    mv "$out/.build-inputs/EweCoreFFI.xcframework" "$stage/previous.xcframework"
fi
mv "$stage/EweCoreFFI.xcframework" "$out/.build-inputs/"
rsync -a --delete "$root/mobile/ios/Sources/" "$out/mobile/ios/Sources/"
rsync -a --delete "$root/mobile/ios/Tests/" "$out/mobile/ios/Tests/"
cp "$stage/generated/EweCore.swift" "$out/mobile/ios/Sources/EweCore/"
cp "$root/Package.swift" "$root/Package.resolved" "$out/"
cp "$bridge" "$out/examples/ui_todo/ios/Bridge/"
python3 - "$out" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1])/'Package.swift'
s=p.read_text().replace('products: [','products: [.library(name: "EweTodoCore", targets: ["EweTodoCore"]), ',1)
s=s.replace('        core,','        core,\n        .target(name: "EweTodoCore", dependencies: ["EweCore", "EweCoreFFI"], path: "examples/ui_todo/ios/Bridge"),',1)
p.write_text(s)
PY
echo "Staged iPhone package in $out"
