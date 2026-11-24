"""Tests for the clock-params Rust task (spec 76 feature 06)."""

import subprocess
from task_command import binary
import tempfile
import unittest
from pathlib import Path

TASK = "check-clock-params"
REPO = Path(__file__).resolve().parents[3]


def run(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [binary(), TASK, "--root", str(root), *args],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True,
    )


def write(root: Path, rel: str, text: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def allowlist(root: Path, text: str) -> Path:
    path = root / "allow.txt"
    path.write_text(text)
    return path


class CheckClockParams(unittest.TestCase):
    def test_the_repository_is_clean(self):
        result = run(REPO, "--check")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("0 violations", result.stdout)

    def test_a_ctx_parameter_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_x/src/a.rs",
                "fn f(ctx: &Ctx) { let _ = ctx.clock().now(); }\n",
            )
            allow = allowlist(root, "")
            result = run(root, "--check", "--allowlist", str(allow))
            self.assertEqual(result.returncode, 0, result.stdout)

    def test_a_dyn_clock_parameter_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_x/src/a.rs",
                "fn f(clock: &dyn Clock) { clock.now(); }\n",
            )
            result = run(root, "--check")
            self.assertEqual(result.returncode, 1)
            self.assertIn("backends/foundation_x/src/a.rs:1", result.stdout)

    def test_a_real_clock_construction_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "bin/platform/src/main.rs",
                "fn main() { let _ = MonotonicClock::new(); }\n",
            )
            result = run(root, "--check")
            self.assertEqual(result.returncode, 1)
            self.assertIn("bin/platform/src/main.rs:1", result.stdout)

    def test_an_allowlisted_file_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_x/src/a.rs",
                "fn f(clock: &dyn Clock) { clock.now(); }\n",
            )
            allow = allowlist(root, "backends/foundation_x/src/a.rs  # test\n")
            result = run(root, "--check", "--allowlist", str(allow))
            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertIn("1 allowlisted", result.stdout)

    def test_a_stale_allowlist_entry_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_x/src/a.rs",
                "fn f(ctx: &Ctx) {}\n",
            )
            allow = allowlist(root, "backends/foundation_x/src/b.rs  # fixed\n")
            result = run(root, "--check", "--allowlist", str(allow))
            self.assertEqual(result.returncode, 1, result.stdout)
            self.assertIn("matches no hit", result.stdout)

    def test_the_clock_crates_are_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_context/src/lib.rs",
                "fn f(clock: &dyn Clock) { clock.now(); }\n",
            )
            write(
                root,
                "backends/foundation_core/src/valtron/ctx.rs",
                "fn f() { MonotonicClock::new(); }\n",
            )
            allow = allowlist(root, "")
            self.assertEqual(
                run(root, "--check", "--allowlist", str(allow)).returncode, 0
            )

    def test_a_test_tree_is_not_scanned(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_x/tests/a.rs",
                "fn f(clock: &dyn Clock) { clock.now(); }\n",
            )
            allow = allowlist(root, "")
            self.assertEqual(
                run(root, "--check", "--allowlist", str(allow)).returncode, 0
            )

    def test_the_apps_tree_is_scanned(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "apps/answerme-agent/src/main.rs",
                "fn f(clock: &dyn Clock) { clock.now(); }\n",
            )
            result = run(root, "--check")
            self.assertEqual(result.returncode, 1)
            self.assertIn("apps/answerme-agent/src/main.rs:1", result.stdout)


if __name__ == "__main__":
    unittest.main()
