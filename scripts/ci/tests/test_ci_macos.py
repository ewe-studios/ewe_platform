"""Tests for the per-package macOS test runs (run: python3 -m unittest discover -s scripts/ci/tests)."""

import importlib.util
import sys
import tomllib
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
CI = REPO / "scripts" / "ci" / "ci.py"
CONFIG = REPO / "ci" / "packages.toml"

# Feature names that build only on Linux. They must never run on macOS.
LINUX_ONLY = ("uring", "ptrace", "fuse", "preload")


def load_ci():
    spec = importlib.util.spec_from_file_location("ci_macos_under_test", CI)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def settings_for(ci, name: str, **extra):
    config = tomllib.loads(CONFIG.read_text())
    package = {**config["packages"].get(name, {}), **extra}
    return ci.settings_for(name, config.get("defaults", {}), package)


class MacosRuns(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()

    def test_macos_commands_use_macos_runs_and_only_the_test_step(self):
        s = settings_for(self.ci, "foundation_http")
        plan = self.ci.commands("foundation_http", s, "uat", "macos")
        self.assertEqual(len(plan["test"]), len(s.macos_runs))
        self.assertIn("multi", plan["test"][1])
        for step in ("fmt", "build", "clippy", "wasm"):
            self.assertEqual(plan[step], [])

    def test_a_package_without_the_key_has_no_macos_test(self):
        s = settings_for(self.ci, "foundation_signals")
        self.assertIsNone(s.macos_runs)
        self.assertEqual(self.ci.commands("foundation_signals", s, "uat", "macos")["test"], [])

    def test_linux_commands_ignore_macos_runs(self):
        s = settings_for(self.ci, "foundation_http")
        linux = self.ci.commands("foundation_http", s, "uat")
        self.assertEqual(linux, self.ci.commands("foundation_http", s, "uat", "linux"))
        self.assertEqual(len(linux["test"]), len(s.runs))
        self.assertTrue(linux["fmt"] and linux["build"] and linux["clippy"])

    def test_no_macos_run_names_a_linux_only_feature(self):
        config = tomllib.loads(CONFIG.read_text())
        for name, table in config["packages"].items():
            for run in table.get("macos-runs", []):
                for word in LINUX_ONLY:
                    self.assertNotIn(word, run, f"{name}: macos-runs entry {run!r}")

    def test_macos_env_replaces_the_linux_env(self):
        s = settings_for(self.ci, "foundation_nativeapis")
        self.assertIn("EWE_REQUIRE_IO_URING", s.env)
        self.assertEqual(s.macos_env, {})

    def test_nativeapis_macos_runs_include_the_macos_watcher(self):
        s = settings_for(self.ci, "foundation_nativeapis")
        self.assertIn("watcher-macos", s.macos_runs)

    def test_bad_macos_runs_is_rejected(self):
        for bad in ([], "x", [1]):
            with self.assertRaises(SystemExit):
                settings_for(self.ci, "foundation_http", **{"macos-runs": bad})

    def test_host_platform_follows_sys_platform(self):
        original = sys.platform
        try:
            sys.platform = "darwin"
            self.assertEqual(self.ci.host_platform(), "macos")
            sys.platform = "linux"
            self.assertEqual(self.ci.host_platform(), "linux")
        finally:
            sys.platform = original


if __name__ == "__main__":
    unittest.main()
