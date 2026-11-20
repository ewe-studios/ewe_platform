#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cargo build --manifest-path "$here/module/Cargo.toml" --target wasm32-unknown-unknown --profile uat --target-dir "$here/module/target"
