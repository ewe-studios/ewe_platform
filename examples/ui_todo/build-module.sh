#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
target_dir=${CARGO_TARGET_DIR:-"$here/../../target"}
cargo build --locked --manifest-path "$here/Cargo.toml" --lib --no-default-features --features web \
    --target wasm32-unknown-unknown --release --target-dir "$target_dir"
mkdir -p "$here/target/web"
stage=$(mktemp "$here/target/web/.ui_todo-XXXXXX")
trap 'rm -f "$stage"' EXIT
mise exec -- wasm-bindgen "$target_dir/wasm32-unknown-unknown/release/ui_todo.wasm" \
    --target bundler --no-typescript --out-dir "$target_dir/todo-bindings"
mkdir -p "$here/target/web/bindings"
cargo run --locked --profile uat --manifest-path "$here/../../backends/foundation_wasm_ui/Cargo.toml" \
    --features tasks --bin ewe-wasm-bundle --target-dir "$target_dir" -- bindings \
    "$target_dir/todo-bindings/ui_todo_bg.js" "$here/target/web/bindings/ui_todo_bg.js"
mv "$target_dir/todo-bindings/ui_todo_bg.wasm" "$stage"
chmod 644 "$stage"
mv "$stage" "$here/target/web/ui_todo.wasm"

cargo build --locked --manifest-path "$here/../../backends/foundation_ui_connectrpc/Cargo.toml" --lib --features browser \
    --release --target wasm32-unknown-unknown --target-dir "$target_dir"
mise exec -- wasm-bindgen "$target_dir/wasm32-unknown-unknown/release/foundation_ui_connectrpc.wasm" \
    --target web --no-typescript --out-dir "$here/target/web/rpc"

cargo build --locked --manifest-path "$here/../../backends/foundation_resources/Cargo.toml" --lib --features browser-bindings \
    --release --target wasm32-unknown-unknown --target-dir "$target_dir"
mise exec -- wasm-bindgen "$target_dir/wasm32-unknown-unknown/release/foundation_resources.wasm" \
    --target web --no-typescript --out-dir "$here/target/web/resources-wasm"
