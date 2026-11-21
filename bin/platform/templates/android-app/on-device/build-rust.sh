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
mkdir -p "$out/kotlin" "$out/jniLibs"
"$target_dir/debug/uniffi-bindgen" generate --library "$target_dir/debug/$lib" --crate counter_app \
    --language kotlin --no-format --out-dir "$out/kotlin"
cargo ndk -p 26 -t arm64-v8a -t x86_64 -o "$out/jniLibs" build --lib --release
