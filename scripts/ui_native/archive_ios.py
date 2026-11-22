"""Check each Apple library slice and write a reproducible archive."""
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
