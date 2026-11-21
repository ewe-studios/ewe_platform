#!/usr/bin/env bash
# Build the native library for iOS devices and simulators.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
if [[ $(uname -s) != Darwin ]]; then
  echo "The iOS archive requires Xcode on macOS." >&2
  exit 1
fi
if ! xcodebuild -version >/dev/null 2>&1 && [[ -z "${DEVELOPER_DIR:-}" ]]; then
  export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi
xcodebuild -version
out=${1:-"$root/target/native-ui/apple"}
mkdir -p "$out"
out=$(cd "$out" && pwd)
stage=$(mktemp -d "$out/.build-XXXXXX")
trap 'rm -rf "$stage"' EXIT
scripts/ui_native/generate-bindings.sh --out "$stage/bindings"
mkdir -p "$stage/headers" "$stage/device" "$stage/simulator" "$stage/mac"
cp "$stage/bindings/swift/EweCoreFFI.h" "$stage/headers/"
cp "$stage/bindings/swift/EweCoreFFI.modulemap" "$stage/headers/module.modulemap"
for target in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin x86_64-apple-darwin; do
  sdk=iphonesimulator
  if [[ "$target" == aarch64-apple-ios ]]; then sdk=iphoneos; fi
  if [[ "$target" == *-darwin ]]; then sdk=macosx; fi
  MACOSX_DEPLOYMENT_TARGET=14.0 SDKROOT=$(xcrun --sdk "$sdk" --show-sdk-path) IPHONEOS_DEPLOYMENT_TARGET=17.0 cargo build --locked --release --lib --target "$target" -p foundation_ui_native \
    --message-format=json-render-diagnostics > "$stage/products.jsonl"
  python3 - "$stage/products.jsonl" "$stage/$target.a" <<'PY'
import json
from pathlib import Path
import shutil
import sys

for line in Path(sys.argv[1]).read_text().splitlines():
    event = json.loads(line)
    if event.get("reason") == "compiler-artifact" and event["target"]["name"] == "foundation_ui_native":
        archive = next(name for name in event["filenames"] if name.endswith(".a"))
        shutil.copyfile(archive, sys.argv[2])
        break
else:
    sys.exit("Cargo did not produce the native static library.")
PY
done
cp "$stage/aarch64-apple-ios.a" "$stage/device/libEweCoreFFI.a"
lipo -create "$stage/aarch64-apple-ios-sim.a" "$stage/x86_64-apple-ios.a" -output "$stage/simulator/libEweCoreFFI.a"
lipo -create "$stage/aarch64-apple-darwin.a" "$stage/x86_64-apple-darwin.a" -output "$stage/mac/libEweCoreFFI.a"
xcodebuild -create-xcframework \
  -library "$stage/device/libEweCoreFFI.a" -headers "$stage/headers" \
  -library "$stage/simulator/libEweCoreFFI.a" -headers "$stage/headers" \
  -library "$stage/mac/libEweCoreFFI.a" -headers "$stage/headers" \
  -output "$stage/EweCoreFFI.xcframework"
python3 - "$stage" <<'PY'
from pathlib import Path
import plistlib
import shutil
import sys
import zipfile

stage = Path(sys.argv[1])
bundle = stage / "EweCoreFFI.xcframework"
info = plistlib.loads((bundle / "Info.plist").read_bytes())
slices = {(item["SupportedPlatform"], item.get("SupportedPlatformVariant"), tuple(sorted(item["SupportedArchitectures"])))
          for item in info["AvailableLibraries"]}
if slices != {("ios", None, ("arm64",)), ("ios", "simulator", ("arm64", "x86_64")), ("macos", None, ("arm64", "x86_64"))}:
    sys.exit(f"Unexpected XCFramework slices: {slices}")
with zipfile.ZipFile(stage / "EweCoreFFI.xcframework.zip", "w", allowZip64=True) as archive:
    for path in sorted(bundle.rglob("*")):
        if path.is_file():
            item = zipfile.ZipInfo(path.relative_to(stage).as_posix())
            item.compress_type = zipfile.ZIP_DEFLATED
            item.external_attr = 0o100644 << 16
            with path.open("rb") as source, archive.open(item, "w") as output:
                shutil.copyfileobj(source, output)
PY
mv "$stage/EweCoreFFI.xcframework.zip" "$out/"
swift package compute-checksum "$out/EweCoreFFI.xcframework.zip"
