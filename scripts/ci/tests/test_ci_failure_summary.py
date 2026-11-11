"""Tests for the end-of-run failure summary (run: python3 -m unittest discover -s scripts/ci/tests)."""

import contextlib
import importlib.util
import io
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

REPO = Path(__file__).resolve().parents[3]
CI = REPO / "scripts" / "ci" / "ci.py"

FAKE_TEST_RUN = """
print("running 2 tests")
print("test good ... ok")
print("test bad::case ... FAILED")
print("---- bad::case stdout ----")
print("thread 'bad::case' panicked at src/lib.rs:7:5:")
print("assertion failed: boom")
print("noise after the panic")
raise SystemExit(1)
"""


def load_ci():
    spec = importlib.util.spec_from_file_location("ci_summary_under_test", CI)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class FailureSummary(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()

    def run_fake(self, script, limit=200, timeout_minutes=1):
        ci = self.ci
        settings = ci.settings_for("fake", {}, {"failure-lines": limit, "timeout-minutes": timeout_minutes})
        # Run the script from a file, so the echoed command line does not hold its text.
        tmp = tempfile.NamedTemporaryFile("w", suffix=".py", delete=False)
        tmp.write(script)
        tmp.close()
        self.addCleanup(Path(tmp.name).unlink)
        cmd = [sys.executable, tmp.name]
        packages = {"fake": object()}
        out = io.StringIO()
        with mock.patch.object(ci, "load_workspace", return_value=packages), \
             mock.patch.object(ci, "load_settings", return_value={"fake": settings}), \
             mock.patch.object(ci, "commands", return_value={"test": [cmd]}), \
             contextlib.redirect_stdout(out):
            code = ci.run_package("fake", ["test"], "uat")
        return code, out.getvalue()

    def test_summary_lists_failed_test_and_panic(self):
        code, out = self.run_fake(FAKE_TEST_RUN)
        self.assertEqual(code, 1)
        summary = out[out.index("failed command(s)"):]
        self.assertIn("    test bad::case ... FAILED", summary)
        self.assertIn("    ---- bad::case stdout ----", summary)
        self.assertIn("    thread 'bad::case' panicked at src/lib.rs:7:5:", summary)
        self.assertIn("    assertion failed: boom", summary)
        self.assertNotIn("noise after the panic", summary)
        self.assertNotIn("test good ... ok", summary)

    def test_output_is_still_streamed(self):
        _, out = self.run_fake(FAKE_TEST_RUN)
        self.assertIn("noise after the panic", out)

    def test_failure_lines_limit_caps_kept_lines(self):
        _, out = self.run_fake(FAKE_TEST_RUN, limit=1)
        summary = out[out.index("failed command(s)"):]
        self.assertIn("    test bad::case ... FAILED", summary)
        self.assertNotIn("assertion failed: boom", summary)

    def test_passing_command_returns_zero(self):
        code, out = self.run_fake("print('test a ... ok')")
        self.assertEqual(code, 0)
        self.assertNotIn("failed command(s)", out)

    def test_timeout_kills_process_and_reports(self):
        ci = self.ci
        with self.assertRaises(subprocess.TimeoutExpired):
            ci.run_streamed([sys.executable, "-c", "import time; time.sleep(30)"], {}, 0.5, 10)

    def test_failure_lines_default_is_200(self):
        self.assertEqual(self.ci.settings_for("fake", {}, {}).failure_lines, 200)


if __name__ == "__main__":
    unittest.main()
