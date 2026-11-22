#!/usr/bin/env python3
"""Check the errstacks error standard (spec 70 feature 67).

Every first-party crate reports errors through `foundation_errstacks`:
its fallible API returns `Result<T, ErrorTrace<CrateError>>`, where
`CrateError` is a `derive_more` enum. Two conditions fail this check:

1. A crate manifest names `thiserror`.
2. A crate declares `foundation_errstacks` but its sources do not use `ErrorTrace`.
   A dependency for tests can appear in tests, examples, or benchmarks.

`foundation_boringtun` is skipped by name: it is a published BSD-3-Clause
fork of Cloudflare's boringtun. It keeps `thiserror` and the upstream error
shape, and it is not first-party code.

    python3 scripts/ci/check_errstacks.py              # check the repository
    python3 scripts/ci/check_errstacks.py --root DIR    # check another tree

Standard library only.
"""

from __future__ import annotations

import argparse
import tomllib
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST_GLOBS = ("backends/*/Cargo.toml", "crates/*/Cargo.toml")

# The published upstream fork keeps thiserror and its error shape.
SKIP = {"foundation_boringtun"}

def manifests(root: Path):
    for pattern in MANIFEST_GLOBS:
        yield from sorted(root.glob(pattern))


def uses_errstacks(src: Path) -> bool:
    if not src.is_dir():
        return False
    return any("ErrorTrace" in path.read_text(errors="replace") for path in src.rglob("*.rs"))


def dependency_scopes(manifest: dict, package: str) -> set[str]:
    scopes = set()
    for group in (manifest, *manifest.get("target", {}).values()):
        for scope in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, options in group.get(scope, {}).items():
                actual = options.get("package", name) if isinstance(options, dict) else name
                if actual == package:
                    scopes.add(scope)
    return scopes


def violations(root: Path) -> list[str]:
    found: list[str] = []
    for manifest in manifests(root):
        crate = manifest.parent.name
        if crate in SKIP:
            continue
        rel = manifest.relative_to(root).as_posix()
        config = tomllib.loads(manifest.read_text())
        if dependency_scopes(config, "thiserror"):
            found.append(f"{rel}: declares thiserror (use foundation_errstacks)")
        scopes = dependency_scopes(config, "foundation_errstacks")
        if not scopes:
            continue
        sources = ["src"]
        if scopes == {"dev-dependencies"}:
            sources.extend(("tests", "examples", "benches"))
        if not any(uses_errstacks(manifest.parent / source) for source in sources):
            names = ", ".join(f"{source}/" for source in sources)
            found.append(f"{rel}: declares foundation_errstacks but {names} never uses ErrorTrace")
    return sorted(found)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()

    found = violations(args.root.resolve())
    if not found:
        print("errstacks guard: ok (every first-party crate uses foundation_errstacks)")
        return 0
    print("errstacks guard: the crates must use the foundation_errstacks error standard.")
    print("Found:")
    for item in found:
        print(f"  {item}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
