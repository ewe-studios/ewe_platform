"""Tests for scripts/ci/check_executor_wake.py (run: python3 -m unittest discover -s scripts/ci/tests)."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "check_executor_wake.py"
REPO = Path(__file__).resolve().parents[3]


def run(root: Path, allowlist: Path | None = None) -> subprocess.CompletedProcess:
    cmd = [sys.executable, str(SCRIPT), "--root", str(root)]
    if allowlist is not None:
        cmd += ["--allowlist", str(allowlist)]
    return subprocess.run(cmd, capture_output=True, text=True)


def write(root: Path, rel: str, text: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


class CheckExecutorWake(unittest.TestCase):
    def test_the_repository_is_clean(self):
        result = run(REPO)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_a_planted_outside_call_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "backends/foundation_core/src/valtron/ok.rs", "fn a() { request_executor_resume(); }\n")
            write(root, "backends/foundation_reactor/src/bad.rs", "fn b() {\n    foundation_core::valtron::request_executor_resume();\n}\n")
            result = run(root)
            self.assertEqual(result.returncode, 1, result.stdout)
            self.assertIn("backends/foundation_reactor/src/bad.rs:2", result.stdout)
            self.assertNotIn("valtron/ok.rs", result.stdout)

    def test_each_wake_function_is_caught(self):
        for name in ("request_executor_resume", "resume_idle", "idle_wake"):
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                write(root, "backends/foundation_http/src/x.rs", f"use foundation_core::{name};\n")
                self.assertEqual(run(root).returncode, 1, name)

    def test_valtron_itself_and_unrelated_words_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "backends/foundation_core/src/valtron/executors/idle_wake.rs", "fn resume_idle() {}\n")
            write(root, "backends/foundation_http/src/x.rs", "fn idle_wakeups() {}\nfn resume_idle_x() {}\n")
            self.assertEqual(run(root).returncode, 0)

    def test_a_listed_exception_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "backends/foundation_wasm/src/host.rs", "fn f() { request_executor_resume(); }\n")
            allow = root / "allow.txt"
            allow.write_text("backends/foundation_wasm/src/host.rs  # the JS host hands control back\n")
            self.assertEqual(run(root, allow).returncode, 0)
            self.assertEqual(run(root).returncode, 1)


if __name__ == "__main__":
    unittest.main()
