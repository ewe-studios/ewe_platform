#!/usr/bin/env python3
"""Package every publishable workspace crate without a crates.io release.

`cargo publish --workspace --dry-run` resolves each dependency as if it were
already on crates.io. This repository keeps its workspace versions ahead of
crates.io until release-plz publishes them (`.github/workflows/release-plz.yaml`),
so that command fails on every run between releases, on a dependency that is
only unpublished, not broken.

This check instead writes a `[patch.crates-io]` table that maps every workspace
crate to its path, then runs `cargo package -p <crate> --no-verify` for each
publishable member. It catches what a release depends on: a bad manifest, a
missing file and a bad include list. It does not build the packaged tarball;
the per-package build, clippy and test jobs already build the same sources, and
release-plz verifies each tarball when it publishes the dependency before its
dependents.

The `[patch.crates-io]` table also hides one class of bug: a publish cycle.
A publish cycle occurs when crate A has a dev-dependency on crate B with a
`path` and a `version`, and B reaches back to A through normal or build
dependencies. On a real publish, cargo must resolve B from crates.io to
package A, but B cannot publish before A. So, before it packages, this script
runs `dev_dependency_cycles` and fails when it finds one. The fix is to remove
the `version` from that dev-dependency: cargo then strips it on publish.

Exit status is 0 only when every publishable crate packages.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


def cargo_metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)


def is_publishable(package: dict) -> bool:
    """A crate is publishable unless its manifest sets `publish = false`."""
    publish = package.get("publish")
    return publish is None or len(publish) > 0


def dev_dependency_cycles(packages: list[dict]) -> list[list[str]]:
    """Find each dev-dependency that makes a publish cycle.

    `packages` is the `packages` list from `cargo metadata --no-deps`. A
    cycle is a publishable crate P with a dev-dependency on a workspace crate
    D, where the dependency has a `path` and a version requirement (`req`)
    other than `*`, and D reaches P through normal or build dependencies.

    Each result is the cycle as a list of crate names: P, D, the crates on
    the path from D back to P, and P again. The list is empty when there is
    no cycle.
    """
    names = {package["name"] for package in packages}

    # Normal and build edges between workspace crates. Dev edges are left out:
    # cargo does not resolve the dev-dependencies of a dependency.
    edges: dict[str, list[str]] = {name: [] for name in names}
    for package in packages:
        for dep in package.get("dependencies", []):
            if dep.get("kind") == "dev" or dep["name"] not in names:
                continue
            if dep["name"] not in edges[package["name"]]:
                edges[package["name"]].append(dep["name"])

    def path_between(start: str, goal: str) -> list[str] | None:
        """Return the shortest normal or build path from start to goal."""
        previous: dict[str, str | None] = {start: None}
        queue = [start]
        while queue:
            current = queue.pop(0)
            if current == goal:
                path = [current]
                while previous[path[-1]] is not None:
                    path.append(previous[path[-1]])
                return list(reversed(path))
            for nxt in edges.get(current, []):
                if nxt not in previous:
                    previous[nxt] = current
                    queue.append(nxt)
        return None

    cycles: list[list[str]] = []
    for package in packages:
        if not is_publishable(package):
            continue
        name = package["name"]
        for dep in package.get("dependencies", []):
            if dep.get("kind") != "dev" or dep["name"] not in names:
                continue
            if not dep.get("path") or dep.get("req", "*") == "*":
                continue
            back = path_between(dep["name"], name)
            if back is not None:
                cycles.append([name, *back])
    return cycles


def report_cycles(cycles: list[list[str]]) -> None:
    """Print each publish cycle and the fix to stderr."""
    for cycle in cycles:
        chain = " -> ".join(cycle[1:])
        print(f"FAILED: publish cycle: {cycle[0]} -(dev)-> {chain}", file=sys.stderr)
        print(
            f"  remove `version` from the `{cycle[1]}` dev-dependency of "
            f"{cycle[0]}, so cargo strips it on publish",
            file=sys.stderr,
        )
    print(f"{len(cycles)} dev-dependency publish cycle(s) found", file=sys.stderr)


def patch_config(packages: list[dict], path: Path) -> None:
    lines = [
        "# Generated by scripts/ci/package_workspace.py.",
        "# Maps each workspace crate to its path, so cargo can resolve a",
        "# dependency that is not on crates.io yet.",
        "[patch.crates-io]",
    ]
    for package in packages:
        manifest_dir = Path(package["manifest_path"]).parent
        lines.append(f'{package["name"]} = {{ path = "{manifest_dir}" }}')
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    metadata = cargo_metadata()
    packages = metadata["packages"]
    publishable = [p for p in packages if is_publishable(p)]
    if not publishable:
        print("no publishable workspace crates", file=sys.stderr)
        return 1

    cycles = dev_dependency_cycles(packages)
    if cycles:
        report_cycles(cycles)
        return 1

    with tempfile.TemporaryDirectory(prefix="ewe-package-") as tmp:
        config = Path(tmp) / "patch.toml"
        patch_config(packages, config)

        failures: list[tuple[str, str]] = []
        for package in publishable:
            name = package["name"]
            print(f"packaging {name} {package['version']}", flush=True)
            result = subprocess.run(
                [
                    "cargo",
                    "package",
                    "--package",
                    name,
                    "--no-verify",
                    "--config",
                    str(config),
                ],
                capture_output=True,
                text=True,
            )
            if result.returncode != 0:
                failures.append((name, (result.stderr or result.stdout).strip()))

    if failures:
        print("", file=sys.stderr)
        for name, output in failures:
            print(f"FAILED: {name}", file=sys.stderr)
            print(output, file=sys.stderr)
            print("", file=sys.stderr)
        print(
            f"{len(failures)} of {len(publishable)} crates failed to package",
            file=sys.stderr,
        )
        return 1

    print(f"packaged {len(publishable)} publishable crates")
    return 0


if __name__ == "__main__":
    sys.exit(main())
