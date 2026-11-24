#!/usr/bin/env python3
"""Publish the workspace crates in an order that includes dev-dependencies.

WHY: `release-plz release` orders crates by their normal dependencies. `cargo
publish` verifies a packaged tarball against crates.io, and that verification
also resolves **dev-dependencies**. When crate A has a dev-dependency on crate B
and B is not on crates.io yet, A fails to package. The release run of
2026-11-24 hit exactly this: `foundation_db 0.0.10` requires
`foundation_nativeapis 0.0.16` as a dev-dependency, and release-plz tried
`foundation_db` first (issue #468).

WHAT: read the workspace graph, topologically sort the workspace members over
normal and build dependencies, then run `cargo publish` for each crate whose
version is not on crates.io. A successful publish is tagged and pushed, as
`release-plz` does (`<crate>-v<version>`).

HOW: the order is the topological order of the normal and build dependency
edges, because those are acyclic. Dev-dependencies form cycles in this
workspace (`foundation_nativeapis` and `foundation_db` need each other), and
cargo's package verification resolves dev-dependencies from crates.io. A crate
that fails on such a cycle is published once with `--no-verify`; the log names
the reason. A published version is skipped. A rate limit (HTTP 429) sleeps for
the time crates.io asks for, and retries.

Usage:
    python3 scripts/ci/publish_crates.py [--dry-run] [--deadline-seconds N]

    CARGO_REGISTRY_TOKEN must name a token with publish rights. The token is
    not needed with `--dry-run`.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections import defaultdict, deque

USER_AGENT = "cargo/1.0"


def run(cmd: list[str], check: bool = True) -> subprocess.CompletedProcess:
    result = subprocess.run(cmd, text=True, capture_output=True, check=False)
    if check and result.returncode != 0:
        message = (result.stderr or result.stdout).strip()
        raise SystemExit(f"{' '.join(cmd)} failed: {message}")
    return result


def index_path(name: str) -> str:
    lowered = name.lower()
    if len(lowered) == 1:
        return f"1/{lowered}"
    if len(lowered) == 2:
        return f"2/{lowered}"
    if len(lowered) == 3:
        return f"3/{lowered[0]}/{lowered}"
    return f"{lowered[:2]}/{lowered[2:4]}/{lowered}"


def index_versions(name: str) -> list[dict]:
    request = urllib.request.Request(
        f"https://index.crates.io/{index_path(name)}", headers={"User-Agent": USER_AGENT}
    )
    try:
        body = urllib.request.urlopen(request, timeout=30).read().decode()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return []
        raise
    return [json.loads(line) for line in body.splitlines() if line.strip()]


def is_published(name: str, version: str) -> bool:
    return any(entry["vers"] == version and not entry.get("yanked") for entry in index_versions(name))


def publishes(package: dict) -> bool:
    publish = package.get("publish")
    return publish is None or len(publish) > 0


def order(metadata: dict) -> list[dict]:
    """Topological order of the workspace members over normal and build dependencies.

    Dev-dependencies are left out: they form cycles in this workspace, and a
    crate can be built before the crates it uses only in tests.
    """
    members = set(metadata["workspace_members"])
    packages = {package["id"]: package for package in metadata["packages"]}
    by_name = {package["name"]: package["id"] for package in metadata["packages"] if package["id"] in members}

    edges: dict[str, set[str]] = {pid: set() for pid in members}
    for pid in members:
        package = packages[pid]
        for dependency in package["dependencies"]:
            if dependency.get("path") is None or dependency.get("kind") == "dev":
                continue
            target = by_name.get(dependency["name"])
            if target is not None and target != pid:
                edges[pid].add(target)

    # Kahn's algorithm; ties are broken by name so a run is repeatable.
    dependents: dict[str, set[str]] = defaultdict(set)
    remaining = {pid: len(edges[pid]) for pid in members}
    for pid, targets in edges.items():
        for target in targets:
            dependents[target].add(pid)
    ready = deque(sorted((pid for pid, n in remaining.items() if n == 0), key=lambda p: packages[p]["name"]))
    ordered: list[str] = []
    while ready:
        pid = ready.popleft()
        ordered.append(pid)
        for dependent in sorted(dependents[pid], key=lambda p: packages[p]["name"]):
            remaining[dependent] -= 1
            if remaining[dependent] == 0:
                ready.append(dependent)
    if len(ordered) != len(members):
        cycle = sorted(packages[pid]["name"] for pid, n in remaining.items() if n > 0)
        raise SystemExit(
            "the workspace graph has a dependency cycle; cargo cannot publish it: "
            + ", ".join(cycle)
        )
    return [packages[pid] for pid in ordered]


def retry_after(output: str) -> int | None:
    for pattern in (r"try again after .*?(\d+) (?:seconds|minutes)", r"retry-after[: ]+(\d+)", r"(\d+) seconds"):
        match = re.search(pattern, output, re.IGNORECASE)
        if match:
            value = int(match.group(1))
            return value * 60 if "minute" in match.group(0).lower() else value
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="print the order; publish nothing")
    parser.add_argument("--deadline-seconds", type=int, default=18000, help="stop after this many seconds")
    parser.add_argument("--repo", default=".", help="repository root (default: .)")
    parser.add_argument("--no-tags", action="store_true", help="publish without creating git tags")
    args = parser.parse_args()

    metadata = json.loads(
        run(["cargo", "metadata", "--format-version", "1"], check=True).stdout
    )
    ordered = [package for package in order(metadata) if publishes(package)]
    print(f"{len(ordered)} publishable workspace crates, in dependency order")

    started = time.monotonic()
    published_now = 0
    for package in ordered:
        name, version = package["name"], package["version"]
        if is_published(name, version):
            continue
        print(f"publish {name} {version}")
        if args.dry_run:
            continue
        while True:
            if time.monotonic() - started > args.deadline_seconds:
                print("deadline reached; the next run continues", file=sys.stderr)
                return 2
            result = run(["cargo", "publish", "-p", name], check=False)
            output = f"{result.stdout}\n{result.stderr}"
            if result.returncode == 0:
                break
            lowered = output.lower()
            if "already uploaded" in lowered or "already exists" in lowered:
                break
            if "429" in output or "too many" in lowered or "rate limit" in lowered:
                wait = retry_after(output) or 60
                print(f"rate limited; waiting {wait}s")
                time.sleep(wait)
                continue
            missing = re.search(
                r"failed to select a version for the requirement `([A-Za-z0-9_-]+) = ", output
            )
            if missing and missing.group(1) in {package["name"] for package in ordered}:
                # A dev-dependency cycle: cargo cannot verify this tarball until
                # the other crate exists, and the other crate cannot publish
                # until this one does. Ship it unverified, once, and say so.
                print(
                    f"warning: {name} and {missing.group(1)} form a dev-dependency "
                    f"cycle; publishing {name} with --no-verify",
                    file=sys.stderr,
                )
                result = run(["cargo", "publish", "-p", name, "--no-verify"], check=False)
                output = f"{result.stdout}\n{result.stderr}"
                if result.returncode == 0:
                    break
            print(output.strip(), file=sys.stderr)
            return 1

        # The sparse index updates a moment after the upload; wait for it so the
        # next crate's verification sees this version.
        for _ in range(60):
            if is_published(name, version):
                break
            time.sleep(2)
        else:
            print(f"{name} {version} is not visible in the index yet", file=sys.stderr)
            return 1

        published_now += 1
        if not args.no_tags:
            tag = f"{name}-v{version}"
            if run(["git", "tag", "-l", tag], check=True).stdout.strip() == "":
                run(["git", "tag", tag], check=True)
            run(["git", "push", "origin", tag], check=True)

    print(f"published {published_now} crate(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
