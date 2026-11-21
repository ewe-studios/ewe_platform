#!/usr/bin/env bash
# Build a classic browser entry that contains its module dependencies.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
output=backends/foundation_wasm_ui/runtimes/foundation-ui-runtime.js
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
cat >"$scratch/entry.js" <<ENTRY
import "$root/backends/foundation_wasm/runtime/foundation-wasm.js";
import "$root/backends/foundation_wasm_ui/runtimes/foundation-ui-bootstrap.js";
import "$root/backends/foundation_wasm_ui/runtimes/platform-scheme-interceptor.js";
ENTRY
pnpm --dir "$root/backends/foundation_ui_wire/ts" exec esbuild "$scratch/entry.js" \
  --bundle --format=iife --target=es2022 --legal-comments=inline --minify --log-level=warning \
  --outfile="$scratch/bundle.js"
if [[ "${1:-}" == --check ]]; then
  cmp "$scratch/bundle.js" "$output"
else
  cp "$scratch/bundle.js" "$output"
fi
