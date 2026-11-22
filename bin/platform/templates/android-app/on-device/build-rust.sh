#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
root=$PWD
cd rust
target_dir=${CARGO_TARGET_DIR:-"$PWD/target"}
cargo build --features bindgen --bin uniffi-bindgen --lib
case "$(uname -s)" in
    Darwin) lib=libcounter_app.dylib ;;
    Linux) lib=libcounter_app.so ;;
    *) echo "Use Linux or macOS to build the Rust library." >&2; exit 1 ;;
esac
out="$root/app/generated"
mkdir -p "$out"
stage=$(mktemp -d "$out/libraries.XXXXXX")
trap 'rm -rf "$stage"' EXIT
"$target_dir/debug/uniffi-bindgen" generate --library "$target_dir/debug/$lib" --crate counter_app \
    --language kotlin --no-format --out-dir "$stage/kotlin"
cargo ndk --platform 26 -t arm64-v8a -t x86_64 -o "$stage/libraries" build --lib --release
for abi in arm64-v8a x86_64; do
    mkdir -p "$stage/jniLibs/$abi"
    cp "$stage/libraries/$abi/libcounter_app.so" "$stage/jniLibs/$abi/"
done
for tree in kotlin jniLibs; do
    rm -rf "$out/$tree"
    mv "$stage/$tree" "$out/$tree"
done
