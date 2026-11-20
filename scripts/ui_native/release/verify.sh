#!/usr/bin/env bash
# Build the starter applications from the selected package candidate.
set -euo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$root"
candidate=target/native-ui/candidate
templates=both
public=false
while (($#)); do
    case "$1" in
        --candidate) candidate=$2; shift 2 ;;
        --templates) templates=$2; shift 2 ;;
        --public) public=true; shift ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done
case "$templates" in both|ios|android) ;; *) exit 2 ;; esac
candidate=$(cd "$candidate" && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
python3 - "$candidate" <<'PY'
import hashlib,json,sys
from pathlib import Path
root = Path(sys.argv[1])
record = json.loads((root / "native-ui.json").read_text())
for name, digest in record["sha256"].items():
    with (root / name).open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != digest:
            raise SystemExit("Artifact digest mismatch: " + name)
PY
if [[ "$templates" != android ]]; then
    : "${EWE_UI_SWIFT:?Set EWE_UI_SWIFT to the Swift checkout}"
    export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
    git -C "$EWE_UI_SWIFT" archive HEAD Examples/Starter | tar -x -C "$stage"
    project="$stage/Examples/Starter/Starter.xcodeproj"
    if ! "$public"; then
        git clone --quiet --local "$EWE_UI_SWIFT" "$stage/swift"
        mkdir -p "$stage/swift/.build-inputs"
        unzip -q "$candidate/EweCoreFFI.xcframework.zip" -d "$stage/swift/.build-inputs"
        python3 - "$candidate/native-ui.json" "$stage/swift/Package.swift" <<'PYMETA'
from pathlib import Path
import sys, json
Path(sys.argv[2]).write_text(json.loads(Path(sys.argv[1]).read_text())["swift_manifest"])
PYMETA
        swift test --package-path "$stage/swift"
        EWE_UI_PACKAGE="$stage/swift" "$stage/swift/scripts/build-example" Starter CODE_SIGNING_ALLOWED=NO build
    else
        for destination in 'generic/platform=iOS' 'generic/platform=iOS Simulator'; do
            xcodebuild -project "$project" -scheme Starter -destination "$destination" -derivedDataPath "$stage/products" CODE_SIGNING_ALLOWED=NO build
        done
    fi
fi
if [[ "$templates" != ios ]]; then
    : "${EWE_UI_KOTLIN:?Set EWE_UI_KOTLIN to the Kotlin checkout}"
    git -C "$EWE_UI_KOTLIN" archive HEAD examples/starter_android | tar -x -C "$stage"
    extra=()
    if ! "$public"; then
        unzip -q "$candidate/android-publication.zip" -d "$stage/maven"
        extra+=("-PeweRepository=$stage/maven")
    fi
    GRADLE_USER_HOME="$stage/gradle" mise exec -- gradle --no-daemon -p "$stage/examples/starter_android" \
        "${extra[@]}" :app:assembleDebug
fi
