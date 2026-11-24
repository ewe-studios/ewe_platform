"""Launch a generated mobile app and verify the installed identity and packaged resources."""

import argparse
import os
import plistlib
from pathlib import Path
import subprocess
import tomllib
from zipfile import ZipFile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", type=Path)
parser.add_argument("project", type=Path)
parser.add_argument("--target", choices=["android", "ios"], required=True)
parser.add_argument("--device", required=True)
parser.add_argument(
    "--signed-bundle",
    action="store_true",
    help="Also build the signed Android release using the configured test key.",
)
args = parser.parse_args()
if args.signed_bundle and args.target != "android":
    parser.error("--signed-bundle requires Android")
root = args.project.resolve()
binary = args.binary.resolve()
with (root / "ewe-ui.toml").open("rb") as source:
    app = tomllib.load(source)
env = dict(os.environ, EWE_UI_BIN=str(binary))
expected_title = (root / app["shared"] / "assets/title.txt").read_bytes()
subprocess.run(
    [binary, "run", "--target", args.target, "--device", args.device],
    cwd=root,
    env=env,
    check=True,
)
if args.target == "ios":
    if "native" in app:
        generated = root / app["ios"]["directory"] / "generated"
        module = app["native"]["swift_ffi"]
        assert (generated / module / "module.modulemap").is_file()
        with (generated / f"{module}.xcframework" / "Info.plist").open("rb") as file:
            slices = plistlib.load(file)["AvailableLibraries"]
        assert slices and all("HeadersPath" not in part for part in slices)
    bundle = subprocess.check_output(
        ["xcrun", "simctl", "get_app_container", args.device, app["app_id"], "app"],
        env=env,
        text=True,
    )
    assert (
        Path(bundle.strip()) / "EweResources/app/title.txt"
    ).read_bytes() == expected_title
else:
    sdk_path = env.get("ANDROID_HOME") or env["ANDROID_SDK_ROOT"]
    adb = Path(sdk_path) / "platform-tools" / ("adb.exe" if os.name == "nt" else "adb")
    subprocess.run(
        [adb, "-s", args.device, "shell", "pidof", app["app_id"]], check=True
    )
    module = app["android"]["module"]
    builds = [(f"apk/debug/{module}-debug.apk", "")]
    if args.signed_bundle:
        subprocess.run(
            [binary, "build", "--target", "android", "--release"],
            cwd=root,
            env=env,
            check=True,
        )
        builds.append((f"bundle/release/{module}-release.aab", "base/"))
    libraries = list(app["android"]["framework_libraries"])
    if "native" in app:
        libraries.append("lib" + app["native"]["library"] + ".so")
    for path, prefix in builds:
        artifact = root / app["android"]["directory"] / module / "build/outputs" / path
        with ZipFile(artifact) as archive:
            for abi in app["android"]["architectures"]:
                for library in libraries:
                    assert archive.namelist().count(f"{prefix}lib/{abi}/{library}") == 1
            assert (
                archive.read(f"{prefix}assets/EweResources/app/title.txt")
                == expected_title
            )
print(f"Verified {app['app_id']} on {args.device}.")
