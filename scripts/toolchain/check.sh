#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
exec python3 -I - "$script_dir/../../mise.toml" "$@" <<'PY'
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib


def read_version(command, pattern, env=None):
    if shutil.which(command[0]) is None:
        return None, "not found"
    try:
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=True, env=env)
    except (OSError, subprocess.CalledProcessError):
        return None, "version command failed"
    match = re.search(pattern, result.stdout, re.MULTILINE)
    if match is None:
        return None, "could not read version"
    return match[1], None


def report(name, version, error):
    if error is not None:
        print(f"FAIL {name}: {error}")
        return False
    print(f"ok {name}: {version}")
    return True


def check(name, expected, command, pattern):
    actual, error = read_version(command, pattern)
    if error is None and actual != expected:
        error = f"expected {expected}, got {actual}"
    return report(name, actual, error)


def version_parts(version):
    parts = tuple(int(part) for part in version.split("."))
    return parts + (0,) * (3 - len(parts))


def check_android_packages(env):
    packages = [
        "platform-tools",
        f'platforms/android-{env["ANDROID_COMPILE_SDK"]}',
        f'build-tools/{env["ANDROID_BUILD_TOOLS"]}',
        f'ndk/{env["ANDROID_NDK"]}',
    ]
    if env.get("ANDROID_UI_COMPILE_SDK"):
        packages.append(f'platforms/android-{env["ANDROID_UI_COMPILE_SDK"]}')
    sdk_root = os.environ.get("ANDROID_HOME")
    if not sdk_root:
        return report("android packages", None, "ANDROID_HOME is absent; run: mise exec -- scripts/toolchain/check.sh --scope android")
    missing = [package for package in packages if not (Path(sdk_root) / package / "source.properties").is_file()]
    if missing:
        return report("android packages", None, f"missing {', '.join(missing)}; run: mise run toolchain:android")
    return report("android packages", ", ".join(packages), None)


def check_sdk(expected):
    executable = shutil.which("sdkmanager")
    if executable is None:
        return report("android-sdk", None, "sdkmanager not found")
    executable = Path(executable)
    if executable.parent.name == "shims" and shutil.which("mise"):
        try:
            executable = Path(subprocess.check_output(["mise", "which", "sdkmanager"], text=True).strip()).resolve()
        except (OSError, subprocess.CalledProcessError):
            return report("android-sdk", None, "mise could not resolve sdkmanager")
    metadata = executable.resolve().parent.parent / "source.properties"
    try:
        match = re.search(r"^Pkg[.]Revision\s*=\s*(\S+)", metadata.read_text(), re.MULTILINE)
    except OSError:
        match = None
    if match is None:
        return report("android-sdk", None, f"could not read the revision in {metadata}")
    actual = match[1]
    error = None if actual == expected else f"expected {expected}, got {actual}"
    return report("android-sdk", actual, error)


def check_xcode(minimum):
    command = ["xcodebuild", "-version"]
    pattern = r"^Xcode (\d+(?:[.]\d+)*)[ \t]*$"
    actual, error = read_version(command, pattern)
    if error is not None and not os.environ.get("DEVELOPER_DIR"):
        default_xcode = "/Applications/Xcode.app/Contents/Developer"
        if Path(default_xcode).is_dir():
            actual, error = read_version(command, pattern, dict(os.environ, DEVELOPER_DIR=default_xcode))
    if error is not None:
        error += "; set DEVELOPER_DIR to the Xcode developer directory"
    elif version_parts(actual) < version_parts(minimum):
        error = f"expected at least {minimum}, got {actual}"
    return report("xcode", actual, error)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("config", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--scope", choices=("full", "wire", "android", "apple"), default="full")
    parser.add_argument("--without-android-packages", action="store_true", help="skip the Android SDK packages")
    args = parser.parse_args()

    with args.config.open("rb") as config_file:
        config = tomllib.load(config_file)

    def pin(name):
        value = config["tools"][name]
        return value if isinstance(value, str) else value["version"]

    checks = []
    if args.scope in ("full", "wire"):
        checks.append(check("flatc", pin("github:google/flatbuffers"), ["flatc", "--version"], r"^flatc(?: version)? (\S+)"))

    if args.scope in ("full", "android"):
        java_vendor, java_version = pin("java").split("-", 1)
        java_build = java_version.removesuffix(".0.LTS")
        java_home = os.environ.get("JAVA_HOME")
        java_bin = Path(java_home) / "bin" if java_home else None
        java = str(java_bin / "java") if java_bin else "java"
        javac = str(java_bin / "javac") if java_bin else "javac"
        checks.extend([
            check("java", java_build, [java, "-version"], rf"(?i)^OpenJDK Runtime Environment {re.escape(java_vendor)}-(\S+)"),
            check("javac", java_build.split("+", 1)[0], [javac, "-version"], r"^javac (\S+)"),
            check("gradle", pin("gradle"), ["gradle", "--version"], r"^Gradle (\S+)"),
            check("kotlin", pin("kotlin"), ["kotlinc", "-version"], r"\bkotlinc-jvm (\S+)"),
        ])
        if shutil.which("cargo-ndk") is None:
            checks.append(report("cargo-ndk", None, "not found"))
        else:
            checks.append(check("cargo-ndk", pin("github:bbqsrc/cargo-ndk"), ["cargo", "ndk", "--version"], r"^cargo-ndk (\S+)"))

    if args.scope in ("full", "android", "apple"):
        macos = subprocess.check_output(["uname", "-s"], text=True).strip() == "Darwin"
        if args.scope == "apple" and not macos:
            checks.append(report("xcode", None, "requires macOS"))
        elif args.scope in ("full", "apple"):
            if macos:
                checks.append(check_xcode(config["env"]["XCODE_MIN"]))
            else:
                print("SKIP xcode: requires macOS")

        if args.scope == "android" or (args.scope == "full" and macos):
            checks.append(check_sdk(pin("http:android-sdk")))
            if args.without_android_packages:
                print("SKIP android packages: check disabled")
            else:
                checks.append(check_android_packages(config["env"]))
        elif args.scope == "full":
            print("SKIP android packages: use --scope android to check")

    return 0 if all(checks) else 1


sys.exit(main())
PY
