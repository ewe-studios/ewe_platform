#!/usr/bin/env bash
# Generate both foreign bindings from the same compiled library.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
out=""
check=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --out) out="${2:?--out needs a build directory}"; shift 2 ;;
        --check) check=true; shift ;;
        *) echo "Usage: $0 [--check] [--out DIRECTORY]" >&2; exit 2 ;;
    esac
done
if [[ -n "$out" ]]; then
    out="$(python3 -c 'import os,sys;print(os.path.abspath(sys.argv[1]))' "$out")"
elif [[ "$check" == false ]]; then
    echo "Specify the binding build directory with --out." >&2
    exit 2
fi
cd "$root"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cargo build --locked --profile uat -p foundation_ui_native --features bindgen --lib --bin uniffi-bindgen --message-format=json-render-diagnostics >"$stage/artifacts"
python3 - "$stage/artifacts" "$stage" <<'PY'
import json
import sys
from pathlib import Path
stage = Path(sys.argv[2])
for line in Path(sys.argv[1]).read_text().splitlines():
    event = json.loads(line)
    if event.get("reason") != "compiler-artifact":
        continue
    if event["target"]["name"] == "foundation_ui_native":
        for filename in event["filenames"]:
            if Path(filename).suffix in {".dll", ".dylib", ".so"}:
                (stage / "library").write_text(filename)
    if event["target"]["name"] == "uniffi-bindgen" and event.get("executable"):
        (stage / "generator").write_text(event["executable"])
PY
generator="$(cat "$stage/generator")"
library="$(cat "$stage/library")"
generate() {
    for language in swift kotlin; do
        mkdir -p "$1/$language"
        "$generator" generate --library "$library" --language "$language" --out-dir "$1/$language" --no-format
    done
}
generate "$stage/current"
if [[ "$check" == true ]]; then
    if [[ -z "$out" ]]; then
        out="$stage/repeated"
        generate "$out"
    fi
    for language in swift kotlin; do diff -ru "$out/$language" "$stage/current/$language"; done
else
    mkdir -p "$out"
    cp -R "$stage/current/." "$out"
fi
