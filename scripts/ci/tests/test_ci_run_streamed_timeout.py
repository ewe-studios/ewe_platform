"""The timeout of a streamed command kills the whole process tree (issue #803).

Why: the macOS 2/4 shard of the nightly hung for more than three hours. The
command under test had started a child, `proc.kill()` killed only the direct
process, and the child held the output pipe open, so the reader loop never
ended.

This test starts a command that forks a child which holds stdout and then
sleeps. With the fix, `run_streamed` raises `TimeoutExpired` in about a second.
Without it, the test itself hangs until the harness timeout.
"""

import importlib.util
import subprocess
import sys
import time
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
CI = REPO / "scripts" / "ci" / "ci.py"

# The child holds the inherited stdout pipe and sleeps long enough that only the
# timeout can end the command.
FORK_AND_HOLD = (
    "import subprocess, sys, time; "
    "subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); "
    "time.sleep(60)"
)


def load_ci():
    spec = importlib.util.spec_from_file_location("ci_timeout_under_test", CI)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class RunStreamedTimeoutTest(unittest.TestCase):
    def test_a_timeout_ends_a_command_whose_child_holds_the_pipe(self):
        ci = load_ci()
        started = time.monotonic()
        with self.assertRaises(subprocess.TimeoutExpired):
            ci.run_streamed([sys.executable, "-c", FORK_AND_HOLD], {}, 1.0, 20)
        elapsed = time.monotonic() - started
        self.assertLess(
            elapsed, 15.0, f"the timeout returned after {elapsed:.1f} s"
        )

    def test_a_finished_command_returns_its_code_and_lines(self):
        ci = load_ci()
        code, kept = ci.run_streamed(
            [sys.executable, "-c", "print('test x ... FAILED')"], {}, 30.0, 20
        )
        self.assertEqual(code, 0)
        self.assertIn("test x ... FAILED", kept)


if __name__ == "__main__":
    unittest.main()
