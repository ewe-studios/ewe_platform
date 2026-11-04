#!/usr/bin/env bash
# Run the full CI pipeline (fmt, build, clippy, test, wasm) inside the Linux CI
# image. Mounts the repository read-write so the image sees the latest code, and
# drives the same `scripts/ci/ci.py` that GitHub Actions runs per package.
#
# Usage:
#   scripts/ci/linux.sh <package>     one package (as CI runs it)
#   PACKAGE=<package> scripts/ci/linux.sh
#   scripts/ci/linux.sh               every package (like a full CI run)
#
# Env overrides:
#   CI_IMAGE        image to run (default: ewestudios/ewe-platform-ci:latest)
#   CI_PLATFORM     container platform (default: linux/amd64, matching CI)
#   CI_TARGET_VOLUME cargo target volume (default: ewe-cargo-amd64 for amd64,
#                    ewe-cargo otherwise) — keeps builds incremental across runs

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE="${1:-${PACKAGE:-all}}"
IMAGE="${CI_IMAGE:-ewestudios/ewe-platform-ci:latest}"
PLATFORM="${CI_PLATFORM:-linux/amd64}"

# Pick a target volume that matches the platform, so arm64 and amd64 artifacts
# never share a cache (a mismatch aborts with "exec format error").
if [[ -n "${CI_TARGET_VOLUME:-}" ]]; then
  TARGET_VOLUME="$CI_TARGET_VOLUME"
elif [[ "$PLATFORM" == *amd64* ]]; then
  TARGET_VOLUME="ewe-cargo-amd64"
else
  TARGET_VOLUME="ewe-cargo"
fi

docker run --rm --platform "$PLATFORM" \
  -v "${ROOT}:/w" -w /w \
  -v "${TARGET_VOLUME}:/target" \
  -e CARGO_TARGET_DIR=/target \
  -e PACKAGE="$PACKAGE" \
  "$IMAGE" \
  bash -c '
    set -euo pipefail
    nu ci/platform-images/scripts/link-submodules.nu
    if [[ "$PACKAGE" == "all" ]]; then
      names=$(python3 scripts/ci/ci.py plan --packages all 2>/dev/null \
        | python3 -c "import sys,json; d=json.load(sys.stdin); print(\" \".join(x[\"package\"] for x in d[\"include\"]))")
      failed=0
      for p in $names; do
        echo ""
        echo "=========== $p ==========="
        if ! python3 scripts/ci/ci.py run "$p" --profile uat; then
          echo "FAILED: $p"
          failed=1
        fi
      done
      exit $failed
    else
      python3 scripts/ci/ci.py run "$PACKAGE" --profile uat
    fi
  '
