#!/usr/bin/env python3
"""Run a publish command again until it succeeds, with rate-limit-aware waits.

The release workflow runs `release-plz release`. That command publishes every
crate that is not on crates.io yet, and it skips the crates that are. So a run
of the same command after a failure continues where the last run stopped.

crates.io limits how many new crates one account can publish in a period. When
the limit stops a publish, the server answers with status 429 and this text:

    Please try again after Tue, 10 Nov 2026 14:19:53 GMT

This script reads that time from the command output and waits until then. When
a failure has no such time, it waits with a cyclic exponential backoff: the wait
doubles from --base-delay up to --max-delay, then starts again at --base-delay.

The script stops with a failure when one of these occurs:

- --max-failures attempts in a row fail and publish nothing new;
- the next wait would end after the --deadline (the job timeout must be longer).

When the script stops early, the next run of the workflow, or a manual re-run of
the failed job, continues the publish.

"Progress" is the line count of --progress-cmd. The default counts the remote git tags,
because release-plz makes one tag for each crate that it publishes.

Run the tests: python3 -m unittest discover -s scripts/ci/tests
"""

from __future__ import annotations

import argparse
import email.utils
import re
import shlex
import subprocess
import sys
import time
from datetime import datetime, timezone

RETRY_AFTER = re.compile(r"try again after ([A-Z][a-z]{2}, \d{1,2} [A-Z][a-z]{2} \d{4} \d{2}:\d{2}:\d{2} GMT)")
RATE_LIMITED = re.compile(r"\b429\b|Too Many Requests")


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument(
        "--max-failures",
        type=int,
        default=5,
        help="stop after this many attempts in a row that fail and publish nothing (default 5)",
    )
    p.add_argument(
        "--base-delay",
        type=float,
        default=30.0,
        help="first backoff wait in seconds when the output gives no retry time (default 30)",
    )
    p.add_argument(
        "--max-delay",
        type=float,
        default=900.0,
        help="largest backoff wait in seconds; the next wait starts again at --base-delay (default 900)",
    )
    p.add_argument(
        "--retry-margin",
        type=float,
        default=5.0,
        help="seconds added to a retry time that the server gives, for clock skew (default 5)",
    )
    p.add_argument(
        "--deadline",
        type=float,
        default=200 * 60.0,
        help="seconds from start after which no new wait begins; keep it below the job timeout (default 12000)",
    )
    p.add_argument(
        "--progress-cmd",
        default="git ls-remote --tags origin",
        help="command whose output line count grows when a crate is published (default: git ls-remote --tags origin)",
    )
    p.add_argument(
        "--dry-sleep",
        action="store_true",
        help="print each wait and do not sleep (for tests)",
    )
    p.add_argument("command", nargs=argparse.REMAINDER, help="the publish command, after --")
    args = p.parse_args(argv)
    if args.command and args.command[0] == "--":
        args.command = args.command[1:]
    if not args.command:
        p.error("give the publish command after --")
    if args.max_failures < 1 or args.base_delay <= 0 or args.max_delay < args.base_delay:
        p.error("need --max-failures >= 1 and 0 < --base-delay <= --max-delay")
    return args


def progress(cmd: str) -> int:
    """Return the line count of the progress command, or -1 when it fails."""
    r = subprocess.run(shlex.split(cmd), capture_output=True, text=True)
    if r.returncode != 0:
        return -1
    return len([line for line in r.stdout.splitlines() if line.strip()])


def run(command: list[str]) -> tuple[int, str]:
    """Run the command, echo its output as it comes, and return the code and the output."""
    proc = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    assert proc.stdout is not None
    lines = []
    for line in proc.stdout:
        sys.stdout.write(line)
        sys.stdout.flush()
        lines.append(line)
    return proc.wait(), "".join(lines)


def retry_after(output: str, now: datetime) -> float | None:
    """Return the seconds until the last retry time in the output, or None when there is none."""
    found = RETRY_AFTER.findall(output)
    if not found:
        return None
    when = email.utils.parsedate_to_datetime(found[-1])
    return max(0.0, (when - now).total_seconds())


class Backoff:
    """Cyclic exponential backoff: base, 2*base, 4*base, ... up to max, then base again."""

    def __init__(self, base: float, maximum: float) -> None:
        self.base = base
        self.maximum = maximum
        self.next = base

    def take(self) -> float:
        wait = self.next
        doubled = self.next * 2
        self.next = doubled if doubled <= self.maximum else self.base
        return wait

    def reset(self) -> None:
        self.next = self.base


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    start = time.monotonic()
    backoff = Backoff(args.base_delay, args.max_delay)
    failures = 0
    attempt = 0
    while True:
        attempt += 1
        before = progress(args.progress_cmd)
        print(f"::group::publish attempt {attempt}", flush=True)
        code, output = run(args.command)
        print("::endgroup::", flush=True)
        if code == 0:
            print(f"publish succeeded on attempt {attempt}", flush=True)
            return 0

        after = progress(args.progress_cmd)
        moved = before >= 0 and after > before
        if moved:
            failures = 0
            backoff.reset()
        else:
            failures += 1
        print(
            f"attempt {attempt} failed with code {code}; published {max(0, after - before) if before >= 0 else '?'} "
            f"new; {failures} failed attempt(s) in a row with no progress",
            flush=True,
        )
        if failures >= args.max_failures:
            print(
                f"::error::publish failed {failures} times in a row with no progress. "
                "The next workflow run or a re-run of this job continues the publish.",
                flush=True,
            )
            return code

        hinted = retry_after(output, datetime.now(timezone.utc))
        if hinted is not None:
            wait = hinted + args.retry_margin
            reason = "the server's retry time"
        else:
            wait = backoff.take()
            reason = "rate limited, no retry time" if RATE_LIMITED.search(output) else "backoff"

        elapsed = time.monotonic() - start
        if elapsed + wait > args.deadline:
            print(
                f"::warning::the next wait ({wait:.0f}s, {reason}) ends after the deadline "
                f"({args.deadline:.0f}s). Stopping. The next workflow run or a re-run of this job continues the publish.",
                flush=True,
            )
            return code

        print(f"waiting {wait:.0f}s ({reason})", flush=True)
        if not args.dry_sleep:
            time.sleep(wait)
        else:
            start -= wait  # A dry wait still uses up the deadline.


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
