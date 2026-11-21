#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$script_dir/../.." && pwd)
crate="$root/backends/foundation_ui_wire"
scratch=$(mktemp -d)
trap 'python3 -I -c "import shutil, sys; shutil.rmtree(sys.argv[1])" "$scratch"' EXIT

"$script_dir/generate.sh" "$scratch"
compare() {
  local difference
  if ! difference=$(diff -qr "$1" "$2"); then
    printf 'Generated wire code differs: %s\n' "${difference%%$'\n'*}" >&2
    exit 1
  fi
}

compare "$scratch/src/generated" "$crate/src/generated"
compare "$scratch/generated" "$crate/generated"
compare "$scratch/foundation-ui-wire.js" "$root/backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js"
