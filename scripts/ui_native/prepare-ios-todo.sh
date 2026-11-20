#!/usr/bin/env bash
# Stage one simulator package that contains the renderer and the app's Rust actions.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
source_package=${1:?Pass the ewe-ui-swift checkout}
out=${2:-$root/target/native-ui/ios-todo}
mkdir -p "$out"
out=$(cd "$out" && pwd)
cd "$root"
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
arch=$(uname -m)
case "$arch" in arm64) target=aarch64-apple-ios-sim ;; x86_64) target=x86_64-apple-ios ;; *) echo "Unsupported simulator CPU $arch" >&2; exit 1 ;; esac
SDKROOT=$(xcrun --sdk iphonesimulator --show-sdk-path) IPHONEOS_DEPLOYMENT_TARGET=17.0 \
    cargo build --locked --release -p ui_todo_native --target "$target"
cargo build --locked --profile uat -p ui_todo_native -p foundation_ui_native --features foundation_ui_native/bindgen
stage=$(mktemp -d "$out/.build-XXXXXX")
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/headers" "$out/Sources/EweTodoCore"
for crate in foundation_ui_native ui_todo_native; do
    target/uat/uniffi-bindgen generate --library target/uat/libui_todo_native.dylib --crate "$crate" \
        --language swift --out-dir "$stage/generated" --no-format
done
cp "$stage/generated/"*.h "$stage/headers/"
cat > "$stage/headers/module.modulemap" <<'MAP'
module EweCoreFFI {
    header "EweCoreFFI.h"
    header "EweTodoCoreFFI.h"
    export *
}
MAP
cp "target/$target/release/libui_todo_native.a" "$stage/libEweCoreFFI.a"
xcodebuild -create-xcframework -library "$stage/libEweCoreFFI.a" -headers "$stage/headers" \
    -output "$stage/EweCoreFFI.xcframework"
mkdir -p "$out/.build-inputs"
if [[ -d "$out/.build-inputs/EweCoreFFI.xcframework" ]]; then
    mv "$out/.build-inputs/EweCoreFFI.xcframework" "$stage/previous.xcframework"
fi
mv "$stage/EweCoreFFI.xcframework" "$out/.build-inputs/"
cp -R "$source_package/Sources/" "$out/Sources/"
cp -R "$source_package/Tests" "$out/"
cp "$source_package/Package.swift" "$source_package/Package.resolved" "$out/"
python3 - "$out" "$stage/generated" <<'PY'
from pathlib import Path
import sys
out,gen=map(Path,sys.argv[1:])
# UniFFI emits same-module references for dependent Swift namespaces.
# This app package exposes each namespace in its own Swift module.
src=(gen/'EweTodoCore.swift').read_text()
(out/'Sources/EweTodoCore/EweTodoCore.swift').write_text('import EweCore\nimport EweCoreFFI\n'+src)
p=out/'Package.swift'
s=p.read_text().replace('products: [','products: [.library(name: "EweTodoCore", targets: ["EweTodoCore"]), ',1)
s=s.replace('        core,','        core,\n        .target(name: "EweTodoCore", dependencies: ["EweCore", "EweCoreFFI"]),',1)
p.write_text(s)
PY
echo "Staged simulator package in $out"
