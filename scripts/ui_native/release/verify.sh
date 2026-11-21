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
# Read files from the commit used to build these artifacts.
source_revision=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["source_revision"])' "$candidate/native-ui.json")
if [[ "$templates" != android ]]; then
    export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
    git archive "$source_revision" examples/mobile_starter/ios | tar -x -C "$stage"
    project="$stage/examples/mobile_starter/ios/Starter.xcodeproj"
    if ! "$public"; then
        mkdir -p "$stage/swift/.build-inputs"
        git archive "$source_revision" Package.swift mobile/ios | tar -x -C "$stage/swift"
        unzip -q "$candidate/EweCoreFFI.xcframework.zip" -d "$stage/swift/.build-inputs"
        python3 - "$candidate/native-ui.json" "$stage/swift" "$project/project.pbxproj" <<'PYMETA'
from pathlib import Path
import sys, json, re
candidate, package, project = map(Path, sys.argv[1:])
record = json.loads(candidate.read_text())
(package / "Package.swift").write_text(record["swift_manifest"])
text = project.read_text()
url = "https://github.com/" + record["repositories"]["repository"] + ".git"
pattern = r'repositoryURL = "' + re.escape(url) + r'";\s*requirement = \{[^}]*version = ' + re.escape(record["version"]) + r';[^}]*\};'
text, count = re.subn(pattern, 'relativePath = ' + json.dumps(str(package)) + ';', text)
if count != 1:
    raise SystemExit("Expected the starter to declare the public package at the release version")
project.write_text(text.replace("XCRemoteSwiftPackageReference", "XCLocalSwiftPackageReference"))
PYMETA
        swift test --package-path "$stage/swift"
        xcodebuild -project "$project" -scheme Starter -destination 'generic/platform=iOS Simulator' \
            -derivedDataPath "$stage/products" CODE_SIGNING_ALLOWED=NO build
    else
        for destination in 'generic/platform=iOS' 'generic/platform=iOS Simulator'; do
            xcodebuild -project "$project" -scheme Starter -destination "$destination" -derivedDataPath "$stage/products" CODE_SIGNING_ALLOWED=NO build
        done
    fi
fi
if [[ "$templates" != ios ]]; then
    git archive "$source_revision" examples/mobile_starter/android | tar -x -C "$stage"
    extra=()
    if ! "$public"; then
        unzip -q "$candidate/android-publication.zip" -d "$stage/maven"
        extra+=("-PeweRepository=$stage/maven")
    fi
    GRADLE_USER_HOME="$stage/gradle" mise exec -- gradle --no-daemon -p "$stage/examples/mobile_starter/android" \
        "${extra[@]}" :app:assembleDebug
fi
