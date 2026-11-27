"""Launch a generated mobile app and verify the installed identity and packaged resources."""

import argparse
import os
import plistlib
from pathlib import Path
import re
import subprocess
import time
import tomllib
import xml.etree.ElementTree as ET
from zipfile import ZipFile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", type=Path)
parser.add_argument("project", type=Path)
parser.add_argument("--target", choices=["android", "ios"], required=True)
parser.add_argument("--device", required=True)
parser.add_argument("--android-server", help="Verify recovery with a reachable HTTP or HTTPS address.")
parser.add_argument("--server-text", help="Text from the first view at that address.")
parser.add_argument("--timeout-seconds", type=float, default=60)
parser.add_argument(
    "--signed-bundle",
    action="store_true",
    help="Also build the signed Android release using the configured test key.",
)
args = parser.parse_args()
if bool(args.android_server) != bool(args.server_text):
    parser.error("--android-server and --server-text must be set together")
if args.android_server and args.target != "android":
    parser.error("--android-server requires Android")
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
    installed = Path(bundle.strip())
    with (installed / "Info.plist").open("rb") as file:
        info = plistlib.load(file)
    assert info["UILaunchScreen"] == {
        "UIColorName": "EweLaunchBackground",
        "UIImageName": "EweSplash",
        "UIImageRespectsSafeAreaInsets": True,
    }
    assert info["CFBundleIcons"]["CFBundlePrimaryIcon"]["CFBundleIconName"] == "AppIcon"
    if app["host"] == "server":
        assert info["NSAppTransportSecurity"] == {
            "NSExceptionDomains": {
                "127.0.0.1": {"NSExceptionAllowsInsecureHTTPLoads": True}
            }
        }
    else:
        assert "NSAppTransportSecurity" not in info
    assert (installed / "Assets.car").is_file()
    assert (
        Path(bundle.strip()) / "EweResources/app/title.txt"
    ).read_bytes() == expected_title
else:
    sdk_path = env.get("ANDROID_HOME") or env["ANDROID_SDK_ROOT"]
    adb = Path(sdk_path) / "platform-tools" / ("adb.exe" if os.name == "nt" else "adb")
    subprocess.run(
        [adb, "-s", args.device, "shell", "pidof", app["app_id"]], check=True
    )
    if app["host"] == "server":
        device = [adb, "-s", args.device]
        component = app["app_id"] + "/" + app["android"]["launch_component"]

        def launch(address):
            command = ["shell", "am", "start", "-S", "-n", component]
            if address is not None:
                command.extend(["--es", "server", address])
            subprocess.run([*device, *command], check=True)

        def expect_text(*expected, absent=()):
            deadline = time.monotonic() + args.timeout_seconds
            while time.monotonic() < deadline:
                dump = subprocess.run(
                    [*device, "exec-out", "uiautomator", "dump", "/dev/tty"],
                    capture_output=True,
                    timeout=max(0.01, deadline - time.monotonic()),
                )
                start = dump.stdout.find(b"<hierarchy")
                end = dump.stdout.find(b"</hierarchy>", start)
                if dump.returncode == 0 and start >= 0 and end >= 0:
                    tree = ET.fromstring(dump.stdout[start:end + len(b"</hierarchy>")])
                    labels = {
                        label: node.get("bounds")
                        for node in tree.iter("node")
                        if node.get("package") == app["app_id"]
                        for key in ("text", "content-desc")
                        if (label := node.get(key))
                    }
                    if labels and all(label in labels for label in expected) and not any(
                        label in labels for label in absent
                    ):
                        return labels
                time.sleep(0.2)
            raise AssertionError(
                f"Missing startup result on {args.device}: expected {expected}, absent {absent}"
            )

        invalid = "Invalid server address. Use an http or https address."
        action = "Use default server"
        launch("invalid")
        labels = expect_text(invalid, action)
        xmin, ymin, xmax, ymax = map(int, re.findall(r"-?\d+", labels[action]))
        subprocess.run(
            [*device, "shell", "input", "tap", str((xmin + xmax) // 2), str((ymin + ymax) // 2)],
            check=True,
        )
        expect_text(absent=(invalid, action))
        if args.android_server:
            launch(args.android_server)
            expect_text(args.server_text)
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
        allow_default_http = app["host"] == "server"
        artifact = root / app["android"]["directory"] / module / "build/outputs" / path
        with ZipFile(artifact) as archive:
            assert any(entry.endswith("/ewe_server_network.xml")
                       for entry in archive.namelist()) == allow_default_http
            for name in ["ewe_launcher.xml", "ewe_launch_icon.png", "ewe_launch_splash.png"]:
                assert any(entry.startswith(prefix + "res/") and entry.endswith("/" + name)
                           for entry in archive.namelist()), name
            for abi in app["android"]["architectures"]:
                for library in libraries:
                    assert archive.namelist().count(f"{prefix}lib/{abi}/{library}") == 1
            assert (
                archive.read(f"{prefix}assets/EweResources/app/title.txt")
                == expected_title
            )
print(f"Verified {app['app_id']} on {args.device}.")
