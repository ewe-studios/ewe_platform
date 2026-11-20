#!/usr/bin/env bash
# Verify the installed compiler with a small schema for each target language.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
"$here/check.sh" --scope wire
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cat >"$work/probe.fbs" <<'SCHEMA'
namespace toolchain;
table Probe { sequence: uint; }
root_type Probe;
SCHEMA

for language in rust ts swift kotlin; do
  flatc "--$language" -o "$work/$language" "$work/probe.fbs"
  if [[ -z $(find "$work/$language" -type f -size +0c -print -quit) ]]; then
    echo "Missing $language bindings" >&2
    exit 1
  fi
  echo "Generated $language bindings"
done
