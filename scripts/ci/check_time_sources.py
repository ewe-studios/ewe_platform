#!/usr/bin/env python3
"""Fail if a crate reads a time source directly (spec 70 feature 117).

AGENTS rule 11 says side effects are injected, and names clocks first. Module
code reads the time from the `Clock` in the `Ctx`, never from the global. The
`Clock` trait has `now()`, `elapsed()` and `wall_now()`, and every task has a
`Ctx`.

This gate is the ratchet. It scans the first-party `src/` trees for the five
direct reads:

* `SystemTime::now`, `Utc::now`, `Local::now`, `OffsetDateTime::now` (wall)
* `Instant::now` (monotonic)

A file outside the allowlist fails the gate on its first direct read.

`scripts/ci/time_sources_allowlist.txt` names every file that still has a
direct read. Each line is a path with a reason after `#`. An entry that ends in
`/` is a directory prefix. Feature 117 and its phases B, C and D delete their
own entries, so the count only goes down.

A single line may opt out with `// time-source: <reason>`; the marker counts as
an allowed read and the script reports the total. Every marker needs a backlog
entry in `docs/bug-backlog.md`.

`foundation_boringtun` is skipped by name: it is a published BSD-3-Clause fork
of Cloudflare's boringtun, not first-party code. It keeps upstream's clock
calls, like its error shape.

    python3 scripts/ci/check_time_sources.py --root . --check
    python3 scripts/ci/check_time_sources.py            # per-file inventory

Standard library only.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ALLOWLIST = Path(__file__).resolve().parent / "time_sources_allowlist.txt"

# The first-party source trees. `backends/` holds the platform crates.
SCAN_GLOBS = (
    "backends/*/src/**/*.rs",
    "crates/*/src/**/*.rs",
    "bin/*/src/**/*.rs",
    "infrastructure/*/src/**/*.rs",
)

# A published upstream fork. It is not first-party code and keeps upstream's
# clock calls, like its error shape (`check_errstacks.py` skips it too).
SKIP_CRATES = {"foundation_boringtun"}

WALL = re.compile(r"\b(?:SystemTime|Utc|Local|OffsetDateTime)::now\b")
MONOTONIC = re.compile(r"\bInstant::now\b")
MARKER = "// time-source:"


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


def is_allowed(rel: str, allowlist: list[str]) -> bool:
    """A file is allowed by an exact path or by a trailing-slash prefix."""
    for entry in allowlist:
        if entry.endswith("/"):
            if rel.startswith(entry):
                return True
        elif rel == entry:
            return True
    return False


def source_files(root: Path):
    """Yield each first-party source file, relative to `root`."""
    seen: set[Path] = set()
    for pattern in SCAN_GLOBS:
        for path in sorted(root.glob(pattern)):
            if path in seen or not path.is_file():
                continue
            seen.add(path)
            rel = path.relative_to(root)
            crate = rel.parts[1] if len(rel.parts) > 1 else ""
            if crate in SKIP_CRATES:
                continue
            yield rel


def scan(root: Path, allowlist: list[str]) -> dict[str, object]:
    """Count every direct read and collect the unallowlisted ones."""
    reads = 0
    marked = 0
    allowed_reads = 0
    files = 0
    violations: list[str] = []
    inventory: list[tuple[str, int]] = []

    for rel in source_files(root):
        path = root / rel
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        rel_text = rel.as_posix()
        allowed = is_allowed(rel_text, allowlist)
        file_reads = 0
        for number, line in enumerate(text.splitlines(), start=1):
            if not (WALL.search(line) or MONOTONIC.search(line)):
                continue
            reads += 1
            file_reads += 1
            if MARKER in line:
                marked += 1
                continue
            if allowed:
                allowed_reads += 1
                continue
            violations.append(f"{rel_text}:{number}: {line.strip()}")
        if file_reads:
            files += 1
            inventory.append((rel_text, file_reads))

    return {
        "reads": reads,
        "marked": marked,
        "allowed": allowed_reads,
        "files": files,
        "violations": sorted(violations),
        "inventory": inventory,
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
    summary = (
        f"{result['reads']} direct reads in {result['files']} files: "
        f"{result['allowed']} allowlisted, {result['marked']} marked, "
        f"{len(violations)} violations"
    )

    if violations:
        print("time source guard: a crate reads a clock directly.")
        print("Read the clock from the Ctx: `ctx.clock().now()` for the monotonic clock,")
        print("`ctx.clock().wall_now()` for the wall clock. Pass `&dyn Clock` to a plain")
        print("function, or store `Arc<dyn Clock>` on a struct. Found:")
        for item in violations:
            print(f"  {item}")
        print(f"time source guard: {summary}")
        return 1

    if not args.check:
        for rel, count in result["inventory"]:
            print(f"  {rel}: {count}")
    print(f"time source guard: ok ({summary})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
