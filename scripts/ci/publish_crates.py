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
cargo's package preparation resolves **every** dependency, including
dev-dependencies, from crates.io; `--no-verify` skips the build, not that
resolution. For a crate that fails on such a cycle, the script comments out
that one dev-dependency in the manifest, publishes the crate, and restores the
manifest at once. Dev-dependencies are not needed by consumers. A published
version is skipped. A rate limit (HTTP 429) sleeps for the time crates.io asks
for, and retries.

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
from pathlib import Path

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



def without_dev_dependency(manifest: Path, name: str) -> str | None:
    """Comment out one dev-dependency and return the original text.

    Returns `None` when the manifest names no such dev-dependency. The caller
    restores the returned text after the publish, so a failed run leaves the
    tree unchanged.
    """
    text = manifest.read_text()
    lines = text.splitlines(keepends=True)
    output: list[str] = []
    section: str | None = None
    commenting = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("["):
            section = stripped
            commenting = False
        if section in {"[dev-dependencies]", "[dev_dependencies]"}:
            if re.match(rf"^{re.escape(name)}\s*=", stripped):
                output.append("# " + line)
                continue
        if section in {f"[dev-dependencies.{name}]", f"[dev_dependencies.{name}]"}:
            commenting = True
        if commenting:
            output.append("# " + line if not stripped.startswith("#") else line)
            continue
        output.append(line)
    new_text = "".join(output)
    if new_text == text:
        return None
    manifest.write_text(new_text)
    return text


def retry_after(output: str) -> int | None:
    """Seconds to wait, from the message crates.io gave."""
    for pattern in (r"try again after .*?(\d+) (?:seconds|minutes)", r"retry-after[: ]+(\d+)", r"(\d+) seconds"):
        match = re.search(pattern, output, re.IGNORECASE)
        if match:
            value = int(match.group(1))
            return value * 60 if "minute" in match.group(0).lower() else value
    # crates.io usually gives a timestamp: "try again after 2026-11-25T03:10:00Z".
    match = re.search(r"try again after (\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)", output)
    if match:
        from datetime import datetime, timezone

        retry_at = datetime.strptime(match.group(1), "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
        wait = int((retry_at - datetime.now(timezone.utc)).total_seconds())
        return max(wait + 5, 30)
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
                # A dev-dependency cycle: cargo resolves the dev-dependency at
                # package preparation, and the other crate cannot exist until
                # this one publishes. Comment the dev-dependency out for this
                # one publish; consumers never need it.
                manifest = Path(package["manifest_path"])
                original = without_dev_dependency(manifest, missing.group(1))
                if original is not None:
                    print(
                        f"warning: {name} and {missing.group(1)} form a dev-dependency "
                        f"cycle; publishing {name} without that dev-dependency",
                        file=sys.stderr,
                    )
                    try:
                        result = run(["cargo", "publish", "-p", name], check=False)
                        output = f"{result.stdout}\n{result.stderr}"
                    finally:
                        manifest.write_text(original)
                    if result.returncode == 0:
                        break
            print(output.strip(), file=sys.stderr)
            return 1

        # The sparse index updates a moment after the upload; wait for it so the
        # next crate's verification sees this version. Ten minutes covers a slow
        # CDN edge.
        for _ in range(300):
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
