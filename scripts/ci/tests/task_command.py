"""Build the independent Rust task command once for the Python test fixtures."""

import json
import subprocess
from functools import cache
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


@cache
def binary():
    build = subprocess.run(
        ["cargo", "build", "--locked", "--profile", "uat", "-p", "foundation_ci", "--message-format=json"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    for line in build.stdout.splitlines():
        event = json.loads(line)
        if event.get("target", {}).get("name") == "foundation-ci" and event.get("executable"):
            return event["executable"]
    raise RuntimeError("Cargo did not return the foundation-ci executable")
