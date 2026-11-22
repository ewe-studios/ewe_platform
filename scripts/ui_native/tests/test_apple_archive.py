"""Check the Apple package layout without a device or a signing key."""
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest
import zipfile

SCRIPT = Path(__file__).resolve().parents[1] / "archive_ios.py"


class AppleArchive(unittest.TestCase):
    def test_archive_contains_the_device_simulator_and_mac_slices(self):
        with tempfile.TemporaryDirectory() as directory:
            stage = Path(directory)
            bundle = stage / "EweCoreFFI.xcframework"
            bundle.mkdir()
            slices = [
                ("ios-arm64", "ios", None, ["arm64"]),
                ("ios-arm64_x86_64-simulator", "ios", "simulator", ["arm64", "x86_64"]),
                ("macos-arm64_x86_64", "macos", None, ["arm64", "x86_64"]),
            ]
            declarations = []
            for identifier, platform, variant, architectures in slices:
                item = {
                    "LibraryIdentifier": identifier,
                    "LibraryPath": "libEweCoreFFI.a",
                    "SupportedPlatform": platform,
                    "SupportedArchitectures": architectures,
                }
                if variant is not None:
                    item["SupportedPlatformVariant"] = variant
                declarations.append(item)
                folder = bundle / identifier
                folder.mkdir()
                (folder / item["LibraryPath"]).write_bytes(identifier.encode())
            info = bundle / "Info.plist"
            info.write_bytes(plistlib.dumps({"AvailableLibraries": declarations}))
            subprocess.run([sys.executable, SCRIPT, stage], check=True)
            with zipfile.ZipFile(stage / "EweCoreFFI.xcframework.zip") as archive:
                for identifier, *_ in slices:
                    path = f"EweCoreFFI.xcframework/{identifier}/libEweCoreFFI.a"
                    self.assertEqual(archive.read(path), identifier.encode())
            info.write_bytes(plistlib.dumps({"AvailableLibraries": declarations[:-1]}))
            invalid = subprocess.run([sys.executable, SCRIPT, stage], capture_output=True, text=True)
            self.assertNotEqual(invalid.returncode, 0)
            self.assertIn("Unexpected XCFramework slices", invalid.stderr)


if __name__ == "__main__":
    unittest.main()
