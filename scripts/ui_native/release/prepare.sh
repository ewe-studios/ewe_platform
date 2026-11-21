#!/usr/bin/env bash
# Build a native package candidate from a clean monorepo checkout.
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
core_revision=$(git rev-parse HEAD)
[[ -z $(git status --porcelain) ]] || { echo "Commit the release inputs before building" >&2; exit 1; }
mise exec -- scripts/toolchain/check.sh --scope apple
mise exec -- scripts/toolchain/check.sh --scope android
inputs=$(mktemp -d)
trap 'rm -rf "$inputs"' EXIT
scripts/ui_native/build-ios.sh "$inputs/apple"
scripts/ui_native/build-android.sh
mise exec -- mobile/android/gradlew -p mobile/android :core:test :ui:testDebugUnitTest publishToMavenLocal "-Dmaven.repo.local=$inputs/maven"
args=(--maven "$inputs/maven" --apple "$inputs/apple/EweCoreFFI.xcframework.zip" --out "$out")
if [[ -n ${NATIVE_SIGNING_KEY:-} ]]; then args+=(--signing-key "$NATIVE_SIGNING_KEY"); fi
if [[ $(git rev-parse HEAD) != "$core_revision" ]]; then
    echo "The core revision changed during the build" >&2
    exit 1
fi
python3 scripts/ui_native/stage_release.py "${args[@]}"
