#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
exec python3 -I - "$script_dir/../.." "$@" <<'PY'
import argparse
from pathlib import Path
import re
import sys
import tomllib
from urllib.parse import urlsplit


def check_pin(name, expected, actual):
    if expected != actual:
        print(f"FAIL {name}: expected {expected}, got {actual or 'no version'}")
        return False
    print(f"ok {name}: {actual}")
    return True


def gradle_version(wrapper):
    text = wrapper.read_text()
    urls = re.findall(r"^\s*distributionUrl\s*=\s*(\S+)[ \t]*$", text, re.MULTILINE)
    if len(urls) != 1:
        return None
    file_name = Path(urlsplit(urls[0].replace(r"\:", ":")).path).name
    match = re.fullmatch(r"gradle-(.+)-(?:bin|all)[.]zip", file_name)
    return match[1] if match else None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("project", type=Path, nargs="?", help="Android Gradle project directory")
    args = parser.parse_args()
    project = args.project or args.root / "mobile/android"

    wrapper = project / "gradle/wrapper/gradle-wrapper.properties"
    catalog = project / "gradle/libs.versions.toml"
    missing = [path for path in (wrapper, catalog) if not path.is_file()]
    for path in missing:
        print(f"FAIL Gradle toolchain drift: missing {path}")
    if missing:
        return 1

    with (args.root / "mise.toml").open("rb") as file:
        tools = tomllib.load(file)["tools"]

    def pin(name):
        value = tools[name]
        return value if isinstance(value, str) else value["version"]

    with catalog.open("rb") as file:
        kotlin = tomllib.load(file).get("versions", {}).get("kotlin")
    gradle_ok = check_pin("Gradle wrapper", pin("gradle"), gradle_version(wrapper))
    kotlin_ok = check_pin("Kotlin catalog", pin("kotlin"), kotlin)
    return 0 if gradle_ok and kotlin_ok else 1


sys.exit(main())
PY
