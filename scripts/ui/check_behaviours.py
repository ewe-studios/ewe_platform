#!/usr/bin/env python3
"""Check the native behaviour declarations against a sibling renderer checkout."""
import argparse
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MATRIX = ROOT / "backends/foundation_ui_components/behaviours.toml"


def check(matrix, platform, package):
    suffix = ".swift" if platform == "ios" else ".kt"
    source = package / ("Sources/EweUI" if platform == "ios" else "ui/src/main/kotlin")
    definitions = set()
    for path in source.rglob("*" + suffix):
        definitions.update(re.findall(r"\b(?:class|struct|object)\s+([A-Za-z0-9_]+)", path.read_text()))
    rows = matrix["behaviours"]
    problems = []
    for name, row in rows.items():
        if bool(row.get("shared")) == bool(row.get("custom")):
            problems.append(f"{name}: expected one wire form")
        binding = row.get(platform)
        if binding and binding not in definitions:
            problems.append(f"{name}: {platform} binding `{binding}` not found")
    for component, config in matrix["components"].items():
        for name in config["behaviours"]:
            if name not in rows:
                problems.append(f"{component}: unknown behaviour `{name}`")
            elif config["native"] and platform not in rows[name]:
                problems.append(f"{component}: behaviour `{name}` has no {platform} form")
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=("ios", "android"))
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--package", type=Path)
    args = parser.parse_args()
    package = args.package
    if package is None:
        common = subprocess.check_output(["git", "-C", ROOT, "rev-parse", "--path-format=absolute", "--git-common-dir"],text=True).strip()
        sibling = "ewe-ui-swift" if args.platform == "ios" else "ewe-ui-kotlin"
        package = Path(common).parent.parent / sibling
    problems = check(tomllib.loads(args.matrix.read_text()), args.platform, package)
    for problem in problems:
        print(problem)
    return int(bool(problems))


if __name__ == "__main__":
    raise SystemExit(main())
