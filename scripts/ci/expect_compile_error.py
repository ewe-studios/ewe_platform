#!/usr/bin/env python3
"""Run a build that must fail with a named compile error.

Usage::

    expect_compile_error.py "<message>" -- <command> [args...]

WHY: some feature configurations are deliberately unsupported, and the crate
refuses them with a `compile_error!` that names the minimum feature set. CI
must check that refusal: if the build starts to succeed, the check is stale, and
if it fails for another reason, the message no longer matches the requirement.

Exit 0 when the command fails and its output contains `message`. Exit 1 when the
command succeeds, fails without the message, or cannot run. The output is always
printed, so the job log shows the compiler's reason.
"""

import subprocess
import sys


def main() -> int:
    args = sys.argv[1:]
    if "--" not in args:
        print("usage: expect_compile_error.py <message> -- <command...>", file=sys.stderr)
        return 2
    split = args.index("--")
    message = " ".join(args[:split])
    command = args[split + 1 :]
    if not message or not command:
        print("usage: expect_compile_error.py <message> -- <command...>", file=sys.stderr)
        return 2

    try:
        result = subprocess.run(command, capture_output=True, text=True)
    except OSError as error:
        print(f"error: could not run {command[0]}: {error}", file=sys.stderr)
        return 1

    output = result.stdout + result.stderr
    print(output, end="")
    if result.returncode == 0:
        print(
            f"error: `{' '.join(command)}` succeeded; expected the compile error: {message}",
            file=sys.stderr,
        )
        return 1
    if message not in output:
        print(
            f"error: `{' '.join(command)}` failed without the expected message: {message}",
            file=sys.stderr,
        )
        return 1
    print(f"ok: the build failed with the expected message: {message}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
