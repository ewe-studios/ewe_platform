#!/usr/bin/env python3
"""Fail if a crate takes a bare clock or builds the real clock itself.

Root rule 11 (CLAUDE.md) says side effects come from one context bundle,
`foundation_context::Ctx`. Code takes a `Ctx`, not `Arc<dyn Clock>`. It reads
the time with `ctx.clock().now()` or `ctx.clock().wall_now()`.

This gate is the ratchet for `Arc<dyn Clock>`. It scans the first-party `src/`
trees for two patterns:

* `dyn Clock` — a parameter or a struct field that should be a `Ctx`;
* `MonotonicClock::new()` — building the real clock outside a host entry point.

The clock's own crates are skipped: `foundation_context` defines the clock,
`foundation_compact` and `foundation_nostd` sit below it, and
`foundation_core/src/valtron` builds the engine clock.

`scripts/ci/clock_params_allowlist.txt` names every allowed site. Each line is
a path with a reason after `#`. An entry that ends in `/` is a directory
prefix. An entry that matches no hit fails the gate, so the list only shrinks.

    python3 scripts/ci/check_clock_params.py --root . --check
    python3 scripts/ci/check_clock_params.py            # per-file inventory

Standard library only.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ALLOWLIST = Path(__file__).resolve().parent / "clock_params_allowlist.txt"

SCAN_GLOBS = (
    "backends/*/src/**/*.rs",
    "crates/*/src/**/*.rs",
    "bin/*/src/**/*.rs",
    "infrastructure/*/src/**/*.rs",
    "apps/*/src/**/*.rs",
)

# The clock's own crates and the engine that builds it.
SKIP_PREFIXES = (
    "backends/foundation_context/",
    "backends/foundation_compact/",
    "backends/foundation_nostd/",
    "backends/foundation_core/src/valtron/",
)

DYN_CLOCK = re.compile(r"\bdyn\s+Clock\b")
MONOTONIC_NEW = re.compile(r"\bMonotonicClock::new\b")


def load_allowlist(path: Path) -> list[str]:
    """Read the allowlist: one path per line, with an optional `# reason`."""
    entries: list[str] = []
    if not path.exists():
        return entries
    for line in path.read_text().splitlines():
        entry = line.split("#", 1)[0].strip()
        if entry:
            entries.append(entry)
    return entries


def matching_entry(rel: str, allowlist: list[str]) -> str | None:
    """The allowlist entry for `rel`: an exact path or a trailing-slash prefix."""
    for entry in allowlist:
        if entry.endswith("/"):
            if rel.startswith(entry):
                return entry
        elif rel == entry:
            return entry
    return None


def source_files(root: Path):
    """Yield each first-party source file, relative to `root`."""
    seen: set[Path] = set()
    for pattern in SCAN_GLOBS:
        for path in sorted(root.glob(pattern)):
            if path in seen or not path.is_file():
                continue
            seen.add(path)
            rel = path.relative_to(root).as_posix()
            if any(rel.startswith(prefix) for prefix in SKIP_PREFIXES):
                continue
            yield rel


def scan(root: Path, allowlist: list[str]) -> dict[str, object]:
    """Count every hit, the unallowlisted ones, and the unused entries."""
    hits = 0
    allowed_hits = 0
    files = 0
    violations: list[str] = []
    inventory: list[tuple[str, int]] = []
    used: set[str] = set()

    for rel in source_files(root):
        path = root / rel
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        entry = matching_entry(rel, allowlist)
        file_hits = 0
        for number, line in enumerate(text.splitlines(), start=1):
            if not (DYN_CLOCK.search(line) or MONOTONIC_NEW.search(line)):
                continue
            hits += 1
            file_hits += 1
            if entry is not None:
                allowed_hits += 1
                used.add(entry)
                continue
            violations.append(f"{rel}:{number}: {line.strip()}")
        if file_hits:
            files += 1
            inventory.append((rel, file_hits))

    return {
        "hits": hits,
        "allowed": allowed_hits,
        "files": files,
        "violations": sorted(violations),
        "inventory": inventory,
        "unused": sorted(set(allowlist) - used),
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--allowlist", type=Path, default=ALLOWLIST)
    parser.add_argument(
        "--check",
        action="store_true",
        help="print only the verdict and the summary (the CI mode)",
    )
    args = parser.parse_args()

    result = scan(args.root.resolve(), load_allowlist(args.allowlist))
    violations = result["violations"]
    unused = result["unused"]
    summary = (
        f"{result['hits']} clock-parameter hits in {result['files']} files: "
        f"{result['allowed']} allowlisted, {len(violations)} violations"
    )

    if violations:
        print("clock parameter guard: a crate takes a bare clock or builds one.")
        print("Take `foundation_context::Ctx` and read `ctx.clock().now()` or")
        print("`ctx.clock().wall_now()`. A host entry point builds the first Ctx.")
        print("Found:")
        for item in violations:
            print(f"  {item}")
        print(f"clock parameter guard: {summary}")
        return 1

    if unused:
        print("clock parameter guard: an allowlist entry matches no hit.")
        print("Delete the entry, or fix the path. The entry outlived its fix. Found:")
        for item in unused:
            print(f"  {item}")
        print(f"clock parameter guard: {len(unused)} unused allowlist entries")
        return 1

    if not args.check:
        for rel, count in result["inventory"]:
            print(f"  {rel}: {count}")
    print(f"clock parameter guard: ok ({summary})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
