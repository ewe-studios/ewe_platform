"""Tests for scripts/ci/check_time_sources.py (run: python3 -m pytest scripts/ci/tests -q)."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "check_time_sources.py"
REPO = Path(__file__).resolve().parents[3]

PATTERNS = (
    "SystemTime::now()",
    "Utc::now()",
    "Local::now()",
    "OffsetDateTime::now()",
    "Instant::now()",
)


def run(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--root", str(root), *args],
        capture_output=True,
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


class CheckTimeSources(unittest.TestCase):
    def test_the_repository_is_clean(self):
        result = run(REPO, "--check")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("0 violations", result.stdout)

    def test_a_clean_file_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "backends/foundation_x/src/a.rs", "fn f(clock: &dyn Clock) { clock.now(); }\n")
            result = run(root, "--check")
            self.assertEqual(result.returncode, 0, result.stdout)

    def test_each_forbidden_pattern_fails(self):
        for pattern in PATTERNS:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                write(root, "backends/foundation_x/src/a.rs", f"fn f() {{ {pattern}; }}\n")
                result = run(root, "--check")
                self.assertEqual(result.returncode, 1, pattern)
                self.assertIn("backends/foundation_x/src/a.rs:1", result.stdout)

    def test_an_allowlisted_file_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "backends/foundation_x/src/a.rs", "fn f() { Instant::now(); }\n")
            allow = allowlist(root, "backends/foundation_x/src/a.rs  # test\n")
            result = run(root, "--check", "--allowlist", str(allow))
            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertIn("1 allowlisted", result.stdout)

    def test_a_directory_prefix_is_allowed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "backends/foundation_x/src/deep/a.rs", "fn f() { Instant::now(); }\n")
            allow = allowlist(root, "backends/foundation_x/src/  # the real backend\n")
            self.assertEqual(run(root, "--check", "--allowlist", str(allow)).returncode, 0)

    def test_a_marked_line_passes_and_is_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_x/src/a.rs",
                "fn f() { Instant::now(); } // time-source: the host entry point\n",
            )
            result = run(root, "--check")
            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertIn("1 marked", result.stdout)

    def test_a_vendored_crate_is_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "backends/foundation_boringtun/src/a.rs", "fn f() { Instant::now(); }\n")
            self.assertEqual(run(root, "--check").returncode, 0)

    def test_a_test_tree_is_not_scanned(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "backends/foundation_x/tests/a.rs", "fn f() { Instant::now(); }\n")
            self.assertEqual(run(root, "--check").returncode, 0)


if __name__ == "__main__":
    unittest.main()
