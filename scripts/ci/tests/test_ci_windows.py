"""Tests for the Windows test runs and the platform guard (run: python3 -m unittest discover -s scripts/ci/tests)."""

import importlib.util
import json
import os
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
CI = REPO / "scripts" / "ci" / "ci.py"
CONFIG = REPO / "ci" / "packages.toml"

# Feature names that build only on Unix. They must never run on Windows.
UNIX_ONLY = ("uring", "ptrace", "fuse", "preload", "fd")


def load_ci():
    spec = importlib.util.spec_from_file_location("ci_windows_under_test", CI)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def settings_for(ci, name: str, **extra):
    config = tomllib.loads(CONFIG.read_text())
    package = {**config["packages"].get(name, {}), **extra}
    return ci.settings_for(name, config.get("defaults", {}), package)


class WindowsRuns(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()

    def test_windows_commands_use_windows_runs_and_only_the_test_step(self):
        s = settings_for(self.ci, "foundation_nativeapis")
        plan = self.ci.commands("foundation_nativeapis", s, "uat", "windows")
        self.assertEqual(len(plan["test"]), len(s.windows_runs))
        self.assertIn("ipc", plan["test"][-1])
        for step in ("fmt", "build", "clippy", "wasm"):
            self.assertEqual(plan[step], [])

    def test_a_package_without_the_key_is_tested_on_windows_by_default(self):
        s = settings_for(self.ci, "foundation_signals")
        self.assertEqual(s.windows_runs, s.runs)
        self.assertEqual(len(self.ci.commands("foundation_signals", s, "uat", "windows")["test"]), len(s.runs))

    def test_default_windows_runs_drop_linux_only_feature_sets(self):
        self.assertEqual(settings_for(self.ci, "foundation_deployment_platform").windows_runs, [""])

    def test_the_synthetic_packages_opt_out_with_a_reason(self):
        for name in ("examples", "wasm-tests"):
            s = settings_for(self.ci, name)
            self.assertIsNone(s.windows_runs)
            self.assertTrue(s.windows_opt_out.strip())

    def test_linux_and_macos_commands_ignore_windows_runs(self):
        s = settings_for(self.ci, "foundation_reactor")
        self.assertEqual(
            self.ci.commands("foundation_reactor", s, "uat"),
            self.ci.commands("foundation_reactor", s, "uat", "linux"),
        )
        self.assertEqual(len(self.ci.commands("foundation_reactor", s, "uat", "macos")["test"]), len(s.macos_runs))

    def test_no_windows_run_names_a_unix_only_feature(self):
        packages = self.ci.load_workspace()
        for name, s in self.ci.load_settings(packages).items():
            for run in s.windows_runs or []:
                for word in UNIX_ONLY:
                    self.assertNotIn(word, run.split(), f"{name}: windows run {run!r}")
                    self.assertNotIn(word, run, f"{name}: windows run {run!r}")

    def test_windows_env_replaces_the_linux_env(self):
        s = settings_for(self.ci, "foundation_nativeapis")
        self.assertIn("EWE_REQUIRE_IO_URING", s.env)
        self.assertEqual(s.platform_env("windows"), {})

    def test_bad_windows_runs_is_rejected(self):
        for bad in ([], "x", [1]):
            with self.assertRaises(SystemExit):
                settings_for(self.ci, "foundation_http", **{"windows-runs": bad})

    def test_host_platform_follows_sys_platform(self):
        original = sys.platform
        try:
            for value, expected in (("win32", "windows"), ("darwin", "macos"), ("linux", "linux")):
                sys.platform = value
                self.assertEqual(self.ci.host_platform(), expected)
        finally:
            sys.platform = original

    def test_run_package_skips_an_opted_out_package(self):
        self.assertEqual(self.ci.run_package("wasm-tests", ["test"], "uat", "windows"), 0)

    def test_display_command_quotes_for_the_host(self):
        self.assertEqual(self.ci.display_command(["cargo", "test", "a b"]), "cargo test 'a b'" if os.name != "nt" else 'cargo test "a b"')


class PlanOutputs(unittest.TestCase):
    def plan(self, packages: str) -> dict[str, str]:
        import subprocess

        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "out"
            env = {**os.environ, "GITHUB_OUTPUT": str(out)}
            proc = subprocess.run(
                [sys.executable, str(CI), "plan", "--packages", packages],
                cwd=REPO, env=env, capture_output=True, text=True,
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            return dict(l.split("=", 1) for l in out.read_text().splitlines())

    def test_plan_selects_every_package_on_all_three_oses(self):
        lines = self.plan("foundation_reactor foundation_signals")
        self.assertEqual(lines["count"], "2")
        self.assertEqual(lines["windows-count"], "2")
        self.assertEqual(lines["macos-count"], "2")

    def test_plan_leaves_out_the_opted_out_packages(self):
        lines = self.plan("foundation_ai examples foundation_signals")
        names = [i["package"] for i in json.loads(lines["windows-matrix"])["include"]]
        self.assertEqual(names, ["foundation_ai", "foundation_signals"])
        names = [i["package"] for i in json.loads(lines["macos-matrix"])["include"]]
        self.assertEqual(names, ["foundation_signals"])

    def test_plan_writes_the_max_parallel_limits(self):
        lines = self.plan("foundation_signals")
        self.assertEqual(
            (lines["max-parallel-linux"], lines["max-parallel-macos"], lines["max-parallel-windows"]),
            ("8", "2", "2"),
        )

    def test_the_full_run_fits_one_matrix(self):
        # GitHub allows 256 jobs per matrix.
        lines = self.plan("all")
        for key in ("count", "macos-count", "windows-count"):
            self.assertLess(int(lines[key]), 256)


class PlatformGuard(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)

    def check(self, features: str = "", **table):
        (self.root / "pkg").mkdir(parents=True, exist_ok=True)
        (self.root / "pkg" / "Cargo.toml").write_text(
            '[package]\nname = "pkg"\n[features]\n' + features + '\n[dependencies]\nopt = { version = "1", optional = true }\n'
        )
        pkg = {"runs": [""], **table}
        packages = {"pkg": self.ci.Package(name="pkg", dir="pkg", needs_llama=False)}
        settings = {"pkg": self.ci.settings_for("pkg", {}, pkg)}
        return self.ci.platform_guard(packages, settings, self.root)

    def test_opt_out_with_reason_passes(self):
        self.assertEqual(self.check(**{"windows-tests": "none", "reason": "needs a GPU"}), [])

    def test_opt_out_without_reason_fails(self):
        errors = self.check(**{"windows-tests": "none"})
        self.assertEqual(len(errors), 1)
        self.assertIn("no reason", errors[0])

    def test_blank_reason_fails(self):
        self.assertEqual(len(self.check(**{"macos-tests": "none", "macos-reason": "  "})), 1)

    def test_opt_out_and_runs_exclude_each_other(self):
        with self.assertRaises(SystemExit):
            self.check(**{"windows-tests": "none", "reason": "x", "windows-runs": [""]})

    def test_override_naming_a_real_feature_passes(self):
        self.assertEqual(self.check("a = []\n", **{"macos-runs": ["", "a", "opt"]}), [])

    def test_override_naming_a_missing_feature_fails(self):
        errors = self.check("a = []\n", **{"windows-runs": ["a", "--features nope"]})
        self.assertEqual(len(errors), 1)
        self.assertIn("nope", errors[0])

    def test_a_default_run_is_not_checked_for_features(self):
        self.assertEqual(self.check(runs=["ghost"]), [])

    def test_the_current_tree_passes_the_guard(self):
        packages = self.ci.load_workspace()
        settings = self.ci.load_settings(packages)
        self.assertEqual(self.ci.platform_guard(packages, settings), [])


if __name__ == "__main__":
    unittest.main()
