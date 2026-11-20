#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$script_dir/../.." && pwd)
crate="$root/backends/foundation_ui_wire"
output="${1:-$crate}"
asset="$root/backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js"
export LC_ALL=C

"$root/scripts/toolchain/check.sh" --scope wire
mkdir -p "$output"
output=$(cd "$output" && pwd)
if [[ $# -gt 0 ]]; then
  asset="$output/foundation-ui-wire.js"
fi
cd "$root"
scratch=$(mktemp -d "$output/.ui-wire-generate.XXXXXXXX")
trap 'python3 -I -c "import shutil, sys; shutil.rmtree(sys.argv[1])" "$scratch"' EXIT

schema=("$crate"/schema/{common,state,ops,frame,action,hello}.fbs)
mkdir -p "$scratch/src/generated" "$scratch/generated/"{ts,swift,kotlin}

# A single Rust input produces one index. The two wire roots stay in their own files.
printf 'include "frame.fbs";\ninclude "state.fbs";\n' > "$scratch/wire.fbs"
flatc --rust --gen-all --gen-object-api --rust-module-root-file --filename-suffix '' \
  -I "$crate/schema" -o "$scratch/src/generated" "$scratch/wire.fbs"
python3 -I "$script_dir/verify_unknown_unions.py" "$scratch/src/generated"
# The staged files can be outside the workspace. The repository ignore paths do not apply.
: > "$scratch/rustfmt.toml"
rustfmt --edition 2021 --config-path "$scratch/rustfmt.toml" "$scratch/src/generated/mod.rs"

flatc --ts --gen-object-api -o "$scratch/generated/ts" "${schema[@]}"
flatc --swift -o "$scratch/generated/swift" "${schema[@]}"
flatc --kotlin -o "$scratch/generated/kotlin" "${schema[@]}"

{
  cat "$crate/ts/index.ts"
  for module in "$scratch"/generated/ts/ewe/ui/*.ts; do
    printf "export * from './ewe/ui/%s.js';\n" "$(basename "$module" .ts)"
  done
} > "$scratch/generated/ts/index.ts"

"$script_dir/bundle.sh" "$scratch/generated/ts/index.ts" "$scratch/foundation-ui-wire.js"
python3 -I "$script_dir/publish_generated.py" \
  "$scratch" "$output" "$asset"
