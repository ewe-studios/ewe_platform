#!/usr/bin/env bash
#
# Checks that the Firecracker generator is idempotent: a second run gives no
# diff (spec 77 feature 04, invariant I5).
#
# The generator is the crate's build script, because the shared `gen_api`
# command writes `<output-dir>/<provider>/generated/` and the Firecracker code
# lives in `src/native/firecracker/generated/`. The build script declares the
# provider, the specification pin and the destination (root rule 5).
#
# HOW: hash the committed tree, make Cargo run the build script again, and
# hash the tree again. The check fails when the build script reports a
# regeneration or when the two hashes differ.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
generated="$repo_root/backends/foundation_sandbox/src/native/firecracker/generated"
build_script="$repo_root/backends/foundation_sandbox/build.rs"

if command -v sha256sum >/dev/null 2>&1; then
    hasher="sha256sum"
else
    hasher="shasum -a 256"
fi

tree_hash() {
    # shellcheck disable=SC2086 # `hasher` is a command and its option.
    find "$generated" -type f | LC_ALL=C sort | xargs $hasher | $hasher | awk '{print $1}'
}

before="$(tree_hash)"

# Make Cargo see a newer build script and run it again.
touch "$build_script"
output="$(cargo check -p foundation_sandbox --features firecracker 2>&1)" || {
    printf '%s\n' "$output" >&2
    exit 1
}
if printf '%s\n' "$output" | grep -q 'regenerated src/native/firecracker/generated'; then
    printf 'the generator rewrote the committed tree; commit the new output\n' >&2
    exit 1
fi

after="$(tree_hash)"
if [ "$before" != "$after" ]; then
    printf 'the generated tree changed between two runs\n' >&2
    exit 1
fi

printf 'the Firecracker generator is idempotent (%s)\n' "$after"
