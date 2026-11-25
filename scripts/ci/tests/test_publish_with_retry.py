"""Tests for scripts/ci/publish_with_retry.py.

Run: python3 -m unittest discover -s scripts/ci/tests

Each test runs the script with a fake publish command. The fake command reads a
plan file: one line per attempt, "ok", "fail", "fail+N" (publishes N crates,
then fails) or "429 <seconds>" (fails with a crates.io retry time that many
seconds from now). It appends one line to a progress file per published crate.
"""

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "publish_with_retry.py"

FAKE = textwrap.dedent(
    """
    import email.utils, sys, time
    from pathlib import Path
    d = Path(sys.argv[1])
    n = int((d / "count").read_text()) if (d / "count").exists() else 0
    (d / "count").write_text(str(n + 1))
    plan = (d / "plan").read_text().splitlines()
    step = plan[min(n, len(plan) - 1)].split()
    def publish(k):
        with open(d / "progress", "a") as f:
            f.write("crate\\n" * k)
    if step[0] == "ok":
        publish(1)
        sys.exit(0)
    if step[0].startswith("fail"):
        publish(int(step[0].split("+")[1]) if "+" in step[0] else 0)
        print("error: something broke")
        sys.exit(1)
    if step[0] == "429":
        publish(1)
        when = email.utils.formatdate(time.time() + float(step[1]), usegmt=True)
        print("the remote server responded with an error (status 429 Too Many Requests): "
              f"You have published too many new crates in a short period of time. "
              f"Please try again after {when} and see https://crates.io/docs/rate-limits")
        sys.exit(1)
    """
)


class PublishWithRetry(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        (self.dir / "fake.py").write_text(FAKE)
        (self.dir / "progress").write_text("")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def run_script(self, plan: list[str], *extra: str) -> subprocess.CompletedProcess:
        (self.dir / "plan").write_text("\n".join(plan) + "\n")
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--dry-sleep",
                "--progress-cmd",
                f"cat {self.dir / 'progress'}",
                *extra,
                "--",
                sys.executable,
                str(self.dir / "fake.py"),
                str(self.dir),
            ],
            capture_output=True,
            text=True,
        )

    def attempts(self) -> int:
        return int((self.dir / "count").read_text())

    def waits(self, out: str) -> list[str]:
        return [line for line in out.splitlines() if line.startswith("waiting ")]

    def test_success_on_the_first_attempt_does_not_wait(self) -> None:
        r = self.run_script(["ok"])
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertEqual(self.attempts(), 1)
        self.assertEqual(self.waits(r.stdout), [])

    def test_waits_until_the_retry_time_that_crates_io_gives(self) -> None:
        r = self.run_script(["429 600", "ok"], "--retry-margin", "5")
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertEqual(self.attempts(), 2)
        [wait] = self.waits(r.stdout)
        seconds = int(wait.split()[1].rstrip("s"))
        self.assertTrue(600 <= seconds <= 606, wait)
        self.assertIn("the server's retry time", wait)

    def test_backoff_doubles_then_starts_again_at_the_base(self) -> None:
        r = self.run_script(
            ["fail"] * 6 + ["ok"],
            "--base-delay",
            "10",
            "--max-delay",
            "40",
            "--max-failures",
            "10",
        )
        self.assertEqual(r.returncode, 0, r.stdout)
        got = [w.split()[1] for w in self.waits(r.stdout)]
        self.assertEqual(got, ["10s", "20s", "40s", "10s", "20s", "40s"])

    def test_stops_after_max_failures_with_no_progress(self) -> None:
        r = self.run_script(["fail"], "--base-delay", "1", "--max-delay", "2", "--max-failures", "3")
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertEqual(self.attempts(), 3)
        self.assertIn("3 times in a row with no progress", r.stdout)

    def test_progress_resets_the_failure_count_and_the_backoff(self) -> None:
        r = self.run_script(
            ["fail", "fail", "fail+1", "fail", "fail", "ok"],
            "--base-delay",
            "10",
            "--max-delay",
            "80",
            "--max-failures",
            "3",
        )
        self.assertEqual(r.returncode, 0, r.stdout)
        got = [w.split()[1] for w in self.waits(r.stdout)]
        # fail, fail -> 10, 20; fail+1 is progress -> reset -> 10; then 20, 40.
        self.assertEqual(got, ["10s", "20s", "10s", "20s", "40s"])

    def test_stops_when_the_next_wait_passes_the_deadline(self) -> None:
        r = self.run_script(["429 600", "ok"], "--deadline", "300")
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertEqual(self.attempts(), 1)
        self.assertIn("ends after the deadline", r.stdout)

    def test_a_command_is_required(self) -> None:
        r = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("publish command", r.stderr)


if __name__ == "__main__":
    unittest.main()
