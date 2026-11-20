#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
crate="$root/backends/foundation_ui_wire"
source="${1:-$crate/generated/ts/index.ts}"
output="${2:-$root/backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js}"
source=$(cd "$(dirname "$source")" && pwd)/$(basename "$source")
output=$(cd "$(dirname "$output")" && pwd)/$(basename "$output")
scratch=$(mktemp -d "$(dirname "$output")/.ui-wire-bundle.XXXXXXXX")
trap 'python3 -I -c "import shutil, sys; shutil.rmtree(sys.argv[1])" "$scratch"' EXIT

NODE_PATH="$crate/ts/node_modules" pnpm --dir "$crate/ts" exec esbuild "$source" \
  --bundle --format=esm --target=es2022 --minify-whitespace --log-level=warning \
  --outfile="$scratch/foundation-ui-wire.js"
mv -f "$scratch/foundation-ui-wire.js" "$output"
