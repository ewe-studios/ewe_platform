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
        self.assertIn("multi,quic", plan["test"][1])
        for step in ("fmt", "build", "clippy", "wasm"):
            self.assertEqual(plan[step], [])

    def test_a_package_without_the_key_is_tested_on_macos_by_default(self):
        s = settings_for(self.ci, "foundation_signals")
        self.assertEqual(s.macos_runs, s.runs)
        self.assertEqual(len(self.ci.commands("foundation_signals", s, "uat", "macos")["test"]), len(s.runs))

    def test_default_macos_runs_drop_linux_only_feature_sets(self):
        s = settings_for(self.ci, "foundation_deployment_platform")
        self.assertEqual(s.runs, ["", "vms"])
        self.assertEqual(s.macos_runs, [""])

    def test_a_package_with_only_linux_only_runs_has_no_macos_test(self):
        config = tomllib.loads(CONFIG.read_text())
        defaults = config["defaults"]
        s = self.ci.settings_for("p", defaults, {"runs": ["uring", "vfs-fuse,x"]})
        self.assertIsNone(s.macos_runs)

    def test_linux_only_filter_reads_flag_style_entries(self):
        only = ["uring", "vms"]
        self.assertTrue(self.ci.run_is_linux_only("fd,uring", only))
        self.assertTrue(self.ci.run_is_linux_only("--no-default-features --features a,vms", only))
        self.assertFalse(self.ci.run_is_linux_only("--no-run --features docker,integration-tests", only))
        self.assertFalse(self.ci.run_is_linux_only("", only))

    def test_an_override_replaces_the_default(self):
        s = settings_for(self.ci, "foundation_boringtun")
        self.assertEqual(s.macos_runs, ["", "device"])

    def test_an_opt_out_has_no_macos_runs(self):
        s = settings_for(self.ci, "foundation_ai")
        self.assertIsNone(s.macos_runs)
        self.assertTrue(s.macos_opt_out)

    def test_default_macos_env_drops_the_linux_only_variables(self):
        s = settings_for(self.ci, "foundation_proxy")
        self.assertIn("EWE_REQUIRE_IO_URING", s.env)
        self.assertEqual(s.macos_env, {})
        cloudflare = settings_for(self.ci, "foundation_deployment_cloudflare")
        self.assertEqual(cloudflare.macos_env, cloudflare.env)

    def test_linux_commands_ignore_macos_runs(self):
        s = settings_for(self.ci, "foundation_http")
        linux = self.ci.commands("foundation_http", s, "uat")
        self.assertEqual(linux, self.ci.commands("foundation_http", s, "uat", "linux"))
        self.assertEqual(len(linux["test"]), len(s.runs))
        self.assertTrue(linux["fmt"] and linux["build"] and linux["clippy"])

    def test_no_macos_run_names_a_linux_only_feature(self):
        packages = self.ci.load_workspace()
        for name, s in self.ci.load_settings(packages).items():
            for run in s.macos_runs or []:
                for word in LINUX_ONLY:
                    self.assertNotIn(word, run, f"{name}: macos run {run!r}")

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
