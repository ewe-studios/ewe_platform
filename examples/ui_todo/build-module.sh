#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
target_dir=${CARGO_TARGET_DIR:-"$here/../../target"}
cargo build --locked --manifest-path "$here/Cargo.toml" --lib --no-default-features --features web \
    --target wasm32-unknown-unknown --release --target-dir "$target_dir"
mkdir -p "$here/target/web"
stage=$(mktemp "$here/target/web/.ui_todo-XXXXXX")
trap 'rm -f "$stage"' EXIT
cp "$target_dir/wasm32-unknown-unknown/release/ui_todo.wasm" "$stage"
chmod 644 "$stage"
mv "$stage" "$here/target/web/ui_todo.wasm"

cargo build --locked --manifest-path "$here/../../backends/foundation_ui_connectrpc/Cargo.toml" --lib --features browser \
    --release --target wasm32-unknown-unknown --target-dir "$target_dir"
mise exec -- wasm-bindgen "$target_dir/wasm32-unknown-unknown/release/foundation_ui_connectrpc.wasm" \
    --target web --no-typescript --out-dir "$here/target/web/rpc"
