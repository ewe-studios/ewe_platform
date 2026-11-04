#!/usr/bin/env python3
"""Fail if the executor wake functions are used outside valtron.

Spec 70 feature 80: waking the executor is valtron's job. A source that wants a
parked task to run again registers the task's `ReadinessWaker`
(`EventReadiness::register`) and calls `wake()` on it, or uses a valtron construct
(`ReadinessNotifier`, `EventQueue`, `ResumeSignal`). It never calls an executor
function. `request_executor_resume`, `resume_idle` and `idle_wake` are private to
`backends/foundation_core/src/valtron/`.

Any exception is listed, one `path  # reason` per line, in
`scripts/ci/executor_wake_allowlist.txt`. The default answer is "no exception".

    python3 scripts/ci/check_executor_wake.py            # check the repository
    python3 scripts/ci/check_executor_wake.py --root DIR  # check another tree (the test)

Standard library only.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ALLOWLIST = Path(__file__).resolve().parent / "executor_wake_allowlist.txt"
# The one place the wake functions may be named.
VALTRON = "backends/foundation_core/src/valtron/"
PATTERN = re.compile(r"\b(request_executor_resume|resume_idle|idle_wake)\b")
SKIP_DIRS = {"target", ".git", "node_modules", ".claude"}


def load_allowlist(path: Path) -> set[str]:
    allowed: set[str] = set()
    if not path.exists():
        return allowed
    for line in path.read_text().splitlines():
        entry = line.split("#", 1)[0].strip()
        if entry:
            allowed.add(entry)
    return allowed


def rust_files(root: Path):
    for path in root.rglob("*.rs"):
        parts = set(path.relative_to(root).parts)
        if parts & SKIP_DIRS:
            continue
        yield path


def violations(root: Path, allowed: set[str]) -> list[str]:
    found: list[str] = []
    for path in rust_files(root):
        rel = path.relative_to(root).as_posix()
        if rel.startswith(VALTRON) or rel in allowed:
            continue
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        for number, line in enumerate(text.splitlines(), start=1):
            if PATTERN.search(line):
                found.append(f"{rel}:{number}: {line.strip()}")
    return sorted(found)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--allowlist", type=Path, default=ALLOWLIST)
    args = parser.parse_args()

    found = violations(args.root.resolve(), load_allowlist(args.allowlist))
    if not found:
        print("executor wake guard: ok (no use outside backends/foundation_core/src/valtron/)")
        return 0
    print("executor wake guard: the executor wake functions are private to valtron.")
    print("Register the task's ReadinessWaker (EventReadiness::register) and call wake(), or use")
    print("a valtron construct (ReadinessNotifier, EventQueue, ResumeSignal). Found:")
    for item in found:
        print(f"  {item}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
