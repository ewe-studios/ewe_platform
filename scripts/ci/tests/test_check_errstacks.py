"""Tests for scripts/ci/check_errstacks.py.

Run: python3 -m unittest discover -s scripts/ci/tests
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "check_errstacks.py"
REPO = Path(__file__).resolve().parents[3]


def run(root: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--root", str(root)],
        capture_output=True,
        text=True,
    )


def write(root: Path, rel: str, text: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


class CheckErrStacks(unittest.TestCase):
    def test_the_repository_is_clean(self):
        result = run(REPO)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_a_planted_thiserror_dependency_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_demo/Cargo.toml",
                '[dependencies]\nthiserror = "2"\n',
            )
            write(root, "backends/foundation_demo/src/lib.rs", "pub fn f() {}\n")
            result = run(root)
            self.assertEqual(result.returncode, 1, result.stdout)
            self.assertIn("declares thiserror", result.stdout)

    def test_a_planted_thiserror_table_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "crates/demo/Cargo.toml",
                "[dependencies.thiserror]\nversion = \"2\"\n",
            )
            write(root, "crates/demo/src/lib.rs", "pub fn f() {}\n")
            result = run(root)
            self.assertEqual(result.returncode, 1, result.stdout)

    def test_unused_errstacks_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_demo/Cargo.toml",
                '[dependencies]\nfoundation_errstacks = "0.0.3"\n',
            )
            write(root, "backends/foundation_demo/src/lib.rs", "pub fn f() {}\n")
            result = run(root)
            self.assertEqual(result.returncode, 1, result.stdout)
            self.assertIn("never uses ErrorTrace", result.stdout)

    def test_used_errstacks_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_demo/Cargo.toml",
                '[dependencies]\nfoundation_errstacks = "0.0.3"\n',
            )
            write(
                root,
                "backends/foundation_demo/src/lib.rs",
                "pub type DemoResult<T> = Result<T, foundation_errstacks::ErrorTrace<DemoError>>;\n",
            )
            self.assertEqual(run(root).returncode, 0)

    def test_foundation_boringtun_is_skipped_by_name(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root,
                "backends/foundation_boringtun/Cargo.toml",
                '[dependencies]\nthiserror = "2"\n',
            )
            write(root, "backends/foundation_boringtun/src/lib.rs", "pub fn f() {}\n")
            self.assertEqual(run(root).returncode, 0)


if __name__ == "__main__":
    unittest.main()
