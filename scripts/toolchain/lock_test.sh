#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
python3 - <<'PY'
import re
import sys
import tomllib
from pathlib import Path

with Path("mise.toml").open("rb") as source:
    pins = tomllib.load(source)["tools"]
with Path("mise.lock").open("rb") as source:
    lock = tomllib.load(source)["tools"]

tools = ("github:google/flatbuffers", "java", "gradle", "kotlin", "http:android-sdk", "github:bbqsrc/cargo-ndk")
platforms = ("linux-x64", "macos-arm64", "macos-x64", "windows-x64")
errors = []
for tool in tools:
    if tool not in pins:
        errors.append(f"{tool} is missing from mise.toml")
        continue
    pin = pins[tool]
    version = pin["version"] if isinstance(pin, dict) else pin
    matches = [entry for entry in lock.get(tool, []) if entry["version"] == version]
    if len(matches) != 1:
        errors.append(f"{tool}@{version} needs one matching entry in mise.lock")
        continue
    entry = matches[0]
    for platform in platforms:
        download = entry.get(f"platforms.{platform}", {})
        if not download.get("url", "").startswith("https://"):
            errors.append(f"{tool}@{version} has no HTTPS download for {platform}")
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", download.get("checksum", "")):
            errors.append(f"{tool}@{version} has no SHA-256 checksum for {platform}")

if errors:
    sys.exit("\n".join(errors))
print("PASS mise.lock")
PY
