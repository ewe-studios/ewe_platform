#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
root=$PWD
cd rust
target_dir=${CARGO_TARGET_DIR:-"$PWD/target"}
cargo build --features bindgen --bin uniffi-bindgen --lib
mkdir -p "$root/generated"
stage=$(mktemp -d "$root/generated/build.XXXXXX")
trap 'rm -rf "$stage"' EXIT
"$target_dir/debug/uniffi-bindgen" generate --library "$target_dir/debug/libcounter_app.dylib" \
    --crate counter_app --language swift --no-format --out-dir "$stage"
mkdir "$stage/headers"
cp "$stage/CounterFFI.h" "$stage/headers/"
cp "$stage/CounterFFI.modulemap" "$stage/headers/module.modulemap"
{ printf 'import EweCore\nimport EweCoreFFI\n'; cat "$stage/CounterKit.swift"; } > "$stage/AppBridge.swift"
case "$(uname -m)" in arm64) sim=aarch64-apple-ios-sim ;; x86_64) sim=x86_64-apple-ios ;; *) exit 1 ;; esac
for target in aarch64-apple-ios "$sim"; do
    IPHONEOS_DEPLOYMENT_TARGET=17.0 cargo build --target "$target" --release --lib
done
xcodebuild -create-xcframework -library "$target_dir/aarch64-apple-ios/release/libcounter_app.a" \
    -library "$target_dir/$sim/release/libcounter_app.a" -output "$stage/CounterFFI.xcframework"
rm -rf "$root/generated/CounterFFI.xcframework"
mv "$stage/CounterFFI.xcframework" "$root/generated/"
mkdir -p "$root/generated/CounterFFI"
cp "$stage/headers/CounterFFI.h" "$stage/headers/module.modulemap" "$root/generated/CounterFFI/"
mv "$stage/AppBridge.swift" "$root/generated/CounterKit.swift"
