#!/usr/bin/env bash
# Synchronize generated Kotlin readers, native bindings and shared fixtures.
set -euo pipefail
worktree="$(cd "$(dirname "$0")/../.." && pwd)"
package="$worktree/mobile/android"
mode="${1:-}"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
"$worktree/scripts/ui_native/generate-bindings.sh" --out "$stage"
sync_tree() {
    if [[ "$mode" == --check ]]; then
        diff -ru "$1" "$2"
    else
        mkdir -p "$2"
        rsync -a --delete "$1/" "$2/"
    fi
}
sync_tree "$worktree/backends/foundation_ui_wire/generated/kotlin" "$package/core/src/generated/kotlin"
sync_tree "$stage/kotlin" "$package/binding/src/generated/kotlin"
sync_tree "$worktree/backends/foundation_ui_wire/fixtures/native" "$package/core/src/test/resources/native"
mkdir -p "$stage/forms"
cp "$worktree/backends/foundation_ui_wire/fixtures/frame_ops_all_forms.bin" "$stage/forms/"
sync_tree "$stage/forms" "$package/core/src/test/resources/forms"
mkdir -p "$stage/fixtures"
for name in frame_state_custom.bin state_counter_custom.json action_increment_custom.bin; do
    cp "$worktree/backends/foundation_ui_wire/fixtures/$name" "$stage/fixtures/"
done
sync_tree "$stage/fixtures" "$package/core/src/test/resources/signed"
cd "$worktree"
cargo build --locked --profile uat -p ui_todo --lib --no-default-features --features mobile --message-format=json-render-diagnostics >"$stage/todo-build.json"
library=$(python3 - "$stage/todo-build.json" <<'PY'
from pathlib import Path
import sys, json
for line in Path(sys.argv[1]).read_text().splitlines():
    item = json.loads(line)
    if item.get("target", {}).get("name") == "ui_todo":
        for f in item.get("filenames", []):
            if Path(f).suffix in (".so", ".dylib", ".dll"):
                print(f)
PY
)
"${CARGO_TARGET_DIR:-$worktree/target}/uat/uniffi-bindgen" generate --library "$library" --crate ui_todo --language kotlin --out-dir "$stage/todo" --no-format
sync_tree "$stage/todo" "$worktree/examples/ui_todo/android/generated"
