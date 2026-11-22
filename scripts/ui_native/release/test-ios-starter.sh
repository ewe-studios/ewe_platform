#!/usr/bin/env bash
# Run the package-only starter. Refresh must show a server change from zero to one.
set -euo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$root"
if [[ $(uname -s) != Darwin ]]; then echo "The iOS test requires macOS and Xcode." >&2; exit 1; fi
local_package=
while (($#)); do
    case "$1" in
        --local-package) local_package=$(cd "${2:?Expected a local Swift package checkout}" && pwd); shift 2 ;;
        --help) echo "Usage: $0 [--local-package /path/to/candidate]"; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done
if [[ -z "${DEVELOPER_DIR:-}" && -d /Applications/Xcode.app/Contents/Developer ]]; then
    export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi
simctl=${EWE_SIMCTL_BIN:-$(xcrun -f simctl)}
stage=$(mktemp -d "${TMPDIR:-/tmp}/ewe-ios-starter.XXXXXXXX")
device=${EWE_IOS_DEVICE:-}
created_device=false
server=
cleanup() {
    local status=$?
    if ((status)); then
        for log in host.log build.log screen.json; do
            if [[ -f "$stage/$log" ]]; then tail -c 4000 "$stage/$log" >&2; fi
        done
    fi
    if [[ -n "$server" ]]; then kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; fi
    if [[ $created_device == true ]]; then "$simctl" shutdown "$device" >/dev/null 2>&1 || true; "$simctl" delete "$device" >/dev/null; fi
    rm -rf "$stage"
    return "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

read -r repository version < <(python3 - <<'PY'
import tomllib
with open("release/native-ui.toml", "rb") as f:
    c = tomllib.load(f)
print(c["repository"], c["version"])
PY
)
example="$root/examples/mobile_starter/ios"
mkdir -p "$stage/consumer"
cp -R "$example/Starter.xcodeproj" "$example/App" "$stage/consumer/"
project="$stage/consumer/Starter.xcodeproj"

# The CLI uses simulator accessibility and hardware input. The release hash matches its Homebrew formula.
axe=${EWE_AXE_BIN:-}
if [[ -z "$axe" ]]; then
    archive="$stage/axe.tgz"
    curl --fail --silent --show-error --location https://github.com/cameroncooke/AXe/releases/download/v2.1.1/AXe-macOS-homebrew-v2.1.1.tar.gz -o "$archive"
    echo "e8a4d873fe49e38d70b8fe1bea180658b5b997f5501561755e304641b1395256  $archive" | shasum -a 256 -c -
    mkdir "$stage/driver"
    tar -xzf "$archive" -C "$stage/driver"
    for framework in "$stage/driver"/Frameworks/*.framework; do
        codesign --force --sign - --timestamp=none "$framework"
    done
    axe="$stage/driver/axe"
    codesign --force --sign - --timestamp=none "$axe"
fi

cargo build --locked --profile uat -p ui_todo --bin ui_todo
if [[ ! -s examples/ui_todo/target/web/ui_todo.wasm ]]; then
    examples/ui_todo/build-module.sh
fi
"${CARGO_TARGET_DIR:-$root/target}/uat/ui_todo" 127.0.0.1:0 >"$stage/host.log" 2>&1 &
server=$!
base=
for ((i=0; i<100; i++)); do
    kill -0 "$server"
    base=$(sed -n 's|^Open \(http://127\.0\.0\.1:[0-9]*\)/$|\1|p' "$stage/host.log" | head -n1)
    if [[ -n "$base" ]] && curl -fsS --max-time 2 "$base/open-count" -o "$stage/initial.bin"; then break; fi
    sleep 0.2
done
[[ -s "$stage/initial.bin" ]] || { echo "The test server did not return its first frame." >&2; exit 1; }
python3 - "$project/project.pbxproj" "$local_package" "$repository" "$version" "$stage/consumer/App/App.swift" "$base" <<'PY'
from pathlib import Path
import json, re, sys
project, package, repo, version, app, base = sys.argv[1:]
p = Path(project)
s = p.read_text()
expected_url = "https://github.com/" + repo + ".git"
if expected_url not in s or "version = " + version + ";" not in s:
    raise SystemExit("The starter must declare the public package at the release version")
if re.findall(r"productName = (\w+);", s[s.index("/* Begin XCSwiftPackageProductDependency"):]) != ["EweUI"]:
    raise SystemExit("The starter must use only the public EweUI product")
if package:
    s = s.replace("XCRemoteSwiftPackageReference", "XCLocalSwiftPackageReference")
    s,n = re.subn(r"repositoryURL = [^;]+;\s*requirement = \{[^}]+\};",
                  "relativePath = " + json.dumps(package) + ";", s)
    if n != 1: raise SystemExit("Cannot identify the starter's package reference")
    p.write_text(s)
p = Path(app)
s = p.read_text()
if s.count("http://127.0.0.1:8080") != 1: raise SystemExit("Cannot identify the example server address")
p.write_text(s.replace("http://127.0.0.1:8080",base))
PY

if [[ -z "$device" ]]; then
"$simctl" list -j runtimes > "$stage/runtimes.json"
read -r runtime model < <(python3 - "$stage/runtimes.json" <<'PY'
import json, sys
with open(sys.argv[1]) as f: runtimes = json.load(f)["runtimes"]
for runtime in sorted(runtimes,key=lambda r:tuple(map(int,r["version"].split("."))),reverse=True):
    if not runtime["isAvailable"] or ".iOS-" not in runtime["identifier"]: continue
    phones=[d for d in runtime.get("supportedDeviceTypes",[]) if d.get("productFamily")=="iPhone"]
    if phones:
        print(runtime["identifier"],phones[0]["identifier"]);break
else: raise SystemExit("Install an iOS simulator runtime with iPhone support")
PY
)
device=$("$simctl" create "Ewe package test $(basename "$stage")" "$model" "$runtime")
created_device=true
fi
"$simctl" bootstatus "$device" -b > "$stage/boot.log"
xcodebuild -project "$project" -scheme Starter -destination 'generic/platform=iOS Simulator' \
    -derivedDataPath "$stage/products" ARCHS="$(uname -m)" ONLY_ACTIVE_ARCH=YES CODE_SIGNING_ALLOWED=NO build > "$stage/build.log" 2>&1
app="$stage/products/Build/Products/Debug-iphonesimulator/Starter.app"
"$simctl" install "$device" "$app"
bundle=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Info.plist")
"$simctl" launch "$device" "$bundle"

wait_count() {
    python3 - "$axe" "$device" "$stage/screen.json" "$1" <<'PY'
import json, subprocess, sys, time
exe, device, path, count=sys.argv[1:]
deadline=time.monotonic()+60
def controls(tree):
    for node in tree:
        yield node.get("type"),node.get("AXLabel")
        yield from controls(node.get("children",[]))
while time.monotonic()<deadline:
    try: run=subprocess.run([exe,"describe-ui","--udid",device],capture_output=True,text=True,timeout=15)
    except subprocess.TimeoutExpired: continue
    if run.returncode==0:
        with open(path,"w") as f: f.write(run.stdout)
        labels=set(controls(json.loads(run.stdout)))
        if ("StaticText",count) in labels and ("Button","Refresh") in labels:
            print("The starter shows count " + count); break
    time.sleep(0.35)
else: raise SystemExit("The starter did not display count " + count)
PY
}
wait_count 0
node scripts/ui_native/release/seed-starter.mjs "$base"
"$axe" tap --label Refresh --udid "$device"
wait_count 1
