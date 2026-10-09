#!/usr/bin/env bash
# Rebuild the #[wasm_test] sample fixture and copy it next to the tests.
#
# wasm32-unknown-unknown needs the LLVM codegen backend; this repo's `dev` profile
# uses Cranelift (fast, but no wasm32 support), so we build with the `uat` profile.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cargo build \
  --manifest-path "$here/module/Cargo.toml" \
  --target wasm32-unknown-unknown \
  --profile uat

cp "$here/module/target/wasm32-unknown-unknown/uat/fwt_sample.wasm" \
   "$here/fixtures/fwt_sample.wasm"

echo "fixture updated: $here/fixtures/fwt_sample.wasm"
