#!/usr/bin/env bash
# Build a native package candidate from three clean source checkouts.
set -euo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$root"
config=release/native-ui.toml
out=target/native-ui/candidate
while (($#)); do
    case "$1" in
        --config) config=$2 ;;
        --output) out=$2 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
    shift 2
done
[[ "$config" == release/native-ui.toml ]] || { echo "Use release/native-ui.toml" >&2; exit 2; }
: "${EWE_UI_SWIFT:?Set EWE_UI_SWIFT to the Swift source checkout}"
: "${EWE_UI_KOTLIN:?Set EWE_UI_KOTLIN to the Kotlin source checkout}"
for repo in "$root" "$EWE_UI_SWIFT" "$EWE_UI_KOTLIN"; do
    [[ -z $(git -C "$repo" status --porcelain) ]] || { echo "Commit the changes in $repo" >&2; exit 1; }
done
python3 - "$EWE_UI_SWIFT" "$EWE_UI_KOTLIN" <<'PYPINS'
from pathlib import Path
import subprocess, sys, tomllib
pins = tomllib.loads(Path("release/native-ui.toml").read_text())
for name, checkout in zip(("swift", "kotlin"), sys.argv[1:]):
    commit = subprocess.check_output(["git", "-C", checkout, "rev-parse", "HEAD"], text=True).strip()
    if pins[name + "_revision"] != commit:
        raise SystemExit(f"Check out the pinned {name} revision before building")
PYPINS
mise exec -- scripts/toolchain/check.sh --scope apple
mise exec -- scripts/toolchain/check.sh --scope android
inputs=$(mktemp -d)
trap 'rm -rf "$inputs"' EXIT
scripts/ui_native/build-ios.sh "$inputs/apple"
scripts/ui_native/build-android.sh
mise exec -- "$EWE_UI_KOTLIN/gradlew" -p "$EWE_UI_KOTLIN" :core:test :ui:testDebugUnitTest publishToMavenLocal "-Dmaven.repo.local=$inputs/maven"
args=(--swift "$EWE_UI_SWIFT" --kotlin "$EWE_UI_KOTLIN" --maven "$inputs/maven" --apple "$inputs/apple/EweCoreFFI.xcframework.zip" --out "$out")
if [[ -n ${NATIVE_SIGNING_KEY:-} ]]; then args+=(--signing-key "$NATIVE_SIGNING_KEY"); fi
python3 scripts/ui_native/stage_release.py "${args[@]}"
