#!/usr/bin/env python3
"""Keep the release branch consistent after a version bump (issue #544).

WHY: release-plz bumps the workspace crate versions. Three kinds of files that
it does not touch then disagree with the tree, and the release PR's own checks
fail:

- path dependencies outside the root workspace (examples, integrations) name
  the old `version`, and a 0.0.x caret requirement excludes the next patch;
- the `backends/foundation_ui_project/templates` pins name the old versions,
  so the template build test fails;
- the nested lockfiles are stale, so the Plan job's `cargo metadata --locked`
  check fails.

WHAT: set every path dependency's requirement to the version in the tree, set
the template pins the same way, then regenerate each nested lockfile.

HOW: parse each manifest with `tomllib` when it parses; fall back to a
line-based rewrite otherwise. `cargo metadata` regenerates a lockfile without
building anything.

Usage:
    python3 scripts/ci/sync_release_tree.py [--check] [--branch BRANCH]

    --check   report the stale files and exit 1 when any is stale; change
              nothing.
    --branch  fetch and check out `BRANCH` from origin before the sync. The
              command then commits the result and pushes it back to the same
              branch. Without it, the working tree is synced in place.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
import tomllib

SKIP_PARTS = {"target", "node_modules", ".git", ".worktrees", ".claude", "tools", "patches"}
TEMPLATE_GLOB = "backends/foundation_ui_project/templates/*/Cargo.toml.template"
INLINE_DEP = re.compile(r"^([A-Za-z0-9_-]+)[ \t]*=[ \t]*\{([^}\n]*)\}[ \t]*$", re.M)
TABLE_DEP = re.compile(
    r"^\[((?:target\.(?:'[^']+'|\"[^\"]+\")\.)?(?:dev-|build-)?dependencies)\."
    r"([A-Za-z0-9_-]+)\]\n((?:(?!\[).*\n?)*)",
    re.M,
)
VERSION = re.compile(r'(version\s*=\s*)"[^"]+"')


def run(cmd: list[str], cwd: pathlib.Path, check: bool = True) -> subprocess.CompletedProcess:
    result = subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, check=False)
    if check and result.returncode != 0:
        message = result.stderr.strip() or result.stdout.strip()
        raise SystemExit(f"{' '.join(cmd)} failed: {message}")
    return result


def manifests(root: pathlib.Path):
    for path in root.rglob("Cargo.toml"):
        if not any(part in SKIP_PARTS for part in path.parts):
            yield path


def package_version(manifest: pathlib.Path) -> tuple[str, str] | None:
    try:
        document = tomllib.loads(manifest.read_text())
    except tomllib.TOMLDecodeError:
        text = manifest.read_text()
        name = re.search(r'^name\s*=\s*"([^"]+)"', text, re.M)
        version = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
        if name and version:
            return name.group(1), version.group(1)
        return None
    package = document.get("package", {})
    if "name" in package and "version" in package:
        return package["name"], package["version"]
    return None


def workspace_members(root: pathlib.Path) -> list[pathlib.Path]:
    """The manifests of the root workspace members, from its `members` globs."""
    document = tomllib.loads((root / "Cargo.toml").read_text())
    members: set[pathlib.Path] = set()
    for pattern in document["workspace"]["members"]:
        for path in root.glob(pattern):
            manifest = path / "Cargo.toml"
            if manifest.is_file():
                members.add(manifest.resolve())
    return sorted(members)


def tree_versions(root: pathlib.Path) -> dict[str, str]:
    """The version of each crate that this repository publishes from the root workspace."""
    versions: dict[str, str] = {}
    for manifest in workspace_members(root):
        found = package_version(manifest)
        if found:
            versions[found[0]] = found[1]
    return versions


def sync_manifest(path: pathlib.Path, versions: dict[str, str]) -> str:
    text = path.read_text()
    original = text

    def inline(match: re.Match) -> str:
        name, body = match.group(1), match.group(2)
        want = versions.get(name)
        if want is None or "path" not in body:
            return match.group(0)
        return f"{name} = {{{VERSION.sub(lambda m: f'{m.group(1)}' + chr(34) + want + chr(34), body)}}}"

    text = INLINE_DEP.sub(inline, text)

    def table(match: re.Match) -> str:
        table, name, body = match.group(1), match.group(2), match.group(3)
        want = versions.get(name)
        if want is None or "path" not in body:
            return match.group(0)
        body = re.sub(r'^version\s*=\s*"[^"]+"', f'version = "{want}"', body, count=1, flags=re.M)
        return f"[{table}.{name}]\n" + body.lstrip("\n")

    text = TABLE_DEP.sub(table, text)
    return text if text != original else ""


def sync_templates(root: pathlib.Path, versions: dict[str, str]) -> list[pathlib.Path]:
    changed = []
    for template in root.glob(TEMPLATE_GLOB):
        text = template.read_text()
        original = text
        for name, version in versions.items():
            if not name.startswith("foundation_"):
                continue
            pattern = rf'(^{re.escape(name)}\s*=\s*\{{[^}}]*version\s*=\s*)"[^"]+"'
            text = re.sub(pattern, rf'\g<1>"{version}"', text, flags=re.M)
        if text != original:
            template.write_text(text)
            changed.append(template)
    return changed


def nested_lockdirs(root: pathlib.Path) -> list[pathlib.Path]:
    locks = run(["git", "ls-files", "*Cargo.lock"], root).stdout.split()
    dirs = []
    for lock in locks:
        if lock == "Cargo.lock":
            continue
        dirs.append(root / pathlib.Path(lock).parent)
    return sorted(set(dirs))


def refresh_locks(root: pathlib.Path, dirs: list[pathlib.Path]) -> list[pathlib.Path]:
    failed = []
    for directory in dirs:
        result = run(
            ["cargo", "metadata", "--format-version", "1", "--manifest-path", "Cargo.toml"],
            directory,
            check=False,
        )
        if result.returncode != 0:
            failed.append(directory)
    return failed


def stale_paths(root: pathlib.Path, versions: dict[str, str]) -> list[str]:
    stale = []
    for manifest in manifests(root):
        text = manifest.read_text()
        for match in INLINE_DEP.finditer(text):
            name, body = match.group(1), match.group(2)
            want = versions.get(name)
            if want is None or "path" not in body:
                continue
            found = re.search(r'version\s*=\s*"([^"]+)"', body)
            if found and found.group(1) != want:
                stale.append(f"{manifest.relative_to(root)}: {name} {found.group(1)} -> {want}")
        # Table forms, including the target-gated ones.
        for match in TABLE_DEP.finditer(text):
            table, name, body = match.group(1), match.group(2), match.group(3)
            want = versions.get(name)
            if want is None or "path" not in body:
                continue
            found = re.search(r'^version\s*=\s*"([^"]+)"', body, re.M)
            if found and found.group(1) != want:
                stale.append(
                    f"{manifest.relative_to(root)}: [{table}.{name}] "
                    f"{found.group(1)} -> {want}"
                )
    return stale


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="report stale files only")
    parser.add_argument("--branch", help="fetch, check out and push this branch")
    parser.add_argument("--repo", default=".", help="repository root (default: .)")
    args = parser.parse_args()

    root = pathlib.Path(args.repo).resolve()
    if args.branch:
        run(["git", "fetch", "origin", args.branch], root)
        run(["git", "checkout", "-B", args.branch, f"origin/{args.branch}"], root)

    versions = tree_versions(root)
    if args.check:
        stale = stale_paths(root, versions)
        for line in stale:
            print(line)
        if stale:
            print(f"{len(stale)} stale path requirement(s); run scripts/ci/sync_release_tree.py")
            return 1
        return 0

    changed = []
    for manifest in manifests(root):
        updated = sync_manifest(manifest, versions)
        if updated:
            manifest.write_text(updated)
            changed.append(manifest.relative_to(root))
    changed.extend(path.relative_to(root) for path in sync_templates(root, versions))

    dirs = nested_lockdirs(root)
    failed = refresh_locks(root, dirs)
    if failed:
        for directory in failed:
            print(f"cargo metadata failed in {directory}", file=sys.stderr)
        return 1

    for path in changed:
        print(f"synced {path}")
    print(f"refreshed {len(dirs)} nested lockfile(s)")

    if args.branch:
        status = run(["git", "status", "--porcelain"], root).stdout.strip()
        if not status:
            print("nothing to sync; the branch is already consistent")
            return 0
        run(["git", "add", "-A"], root)
        run(
            [
                "git",
                "commit",
                "-m",
                "chore(release): sync the requirements, template pins and nested locks",
            ],
            root,
        )
        run(["git", "push", "origin", f"HEAD:{args.branch}"], root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
