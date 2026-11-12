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

    def test_a_package_without_the_key_has_no_windows_test(self):
        s = settings_for(self.ci, "foundation_signals")
        self.assertIsNone(s.windows_runs)
        self.assertEqual(self.ci.commands("foundation_signals", s, "uat", "windows")["test"], [])

    def test_linux_and_macos_commands_ignore_windows_runs(self):
        s = settings_for(self.ci, "foundation_reactor")
        self.assertEqual(
            self.ci.commands("foundation_reactor", s, "uat"),
            self.ci.commands("foundation_reactor", s, "uat", "linux"),
        )
        self.assertEqual(len(self.ci.commands("foundation_reactor", s, "uat", "macos")["test"]), len(s.macos_runs))

    def test_no_windows_run_names_a_unix_only_feature(self):
        config = tomllib.loads(CONFIG.read_text())
        for name, table in config["packages"].items():
            for run in table.get("windows-runs", []):
                for word in UNIX_ONLY:
                    self.assertNotIn(word, run.split(), f"{name}: windows-runs entry {run!r}")
                    self.assertNotIn(word, run, f"{name}: windows-runs entry {run!r}")

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

    def test_run_package_skips_a_package_without_windows_runs(self):
        self.assertEqual(self.ci.run_package("foundation_signals", ["test"], "uat", "windows"), 0)

    def test_display_command_quotes_for_the_host(self):
        self.assertEqual(self.ci.display_command(["cargo", "test", "a b"]), "cargo test 'a b'" if os.name != "nt" else 'cargo test "a b"')


class PlanOutputs(unittest.TestCase):
    def test_plan_writes_windows_and_macos_counts(self):
        import subprocess

        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "out"
            env = {**os.environ, "GITHUB_OUTPUT": str(out)}
            proc = subprocess.run(
                [sys.executable, str(CI), "plan", "--packages", "foundation_reactor foundation_signals"],
                cwd=REPO, env=env, capture_output=True, text=True,
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            lines = dict(l.split("=", 1) for l in out.read_text().splitlines())
            self.assertEqual(lines["windows-count"], "1")
            self.assertEqual(lines["macos-count"], "1")
            self.assertEqual([i["package"] for i in json.loads(lines["windows-matrix"])["include"]], ["foundation_reactor"])


class PlatformGuard(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)

    def check(self, source: str, **table):
        (self.root / "pkg" / "src").mkdir(parents=True, exist_ok=True)
        (self.root / "pkg" / "src" / "lib.rs").write_text(source)
        pkg = {"runs": [""], **table}
        packages = {"pkg": self.ci.Package(name="pkg", dir="pkg", needs_llama=False)}
        settings = {"pkg": self.ci.settings_for("pkg", {}, pkg)}
        return self.ci.platform_guard(packages, settings, self.root)

    def test_windows_cfg_without_runs_fails(self):
        errors = self.check('#[cfg(target_os = "windows")]\nfn a() {}\n')
        self.assertEqual(len(errors), 1)
        self.assertIn("windows-runs", errors[0])

    def test_bare_windows_cfg_counts(self):
        self.assertEqual(len(self.check("#[cfg(all(windows, feature = \"x\"))]\nfn a() {}\n")), 1)

    def test_macos_cfg_without_runs_fails(self):
        self.assertIn("macos-runs", self.check('fn a() { if cfg!(target_vendor = "apple") {} }\n')[0])

    def test_runs_pass(self):
        self.assertEqual(self.check('#[cfg(windows)]\nfn a() {}\n', **{"windows-runs": [""]}), [])

    def test_opt_out_with_reason_passes(self):
        self.assertEqual(
            self.check('#[cfg(windows)]\nfn a() {}\n', **{"windows-tests": "none", "reason": "needs a GPU"}), []
        )

    def test_opt_out_without_reason_is_rejected(self):
        with self.assertRaises(SystemExit):
            self.check('fn a() {}\n', **{"windows-tests": "none"})

    def test_opt_out_and_runs_exclude_each_other(self):
        with self.assertRaises(SystemExit):
            self.check('fn a() {}\n', **{"windows-tests": "none", "reason": "x", "windows-runs": [""]})

    def test_cfg_in_a_comment_does_not_count(self):
        source = '// #[cfg(target_os = "windows")]\n/// cfg(windows)\n/* cfg(target_os = "macos") */\nfn a() {}\n'
        self.assertEqual(self.check(source), [])

    def test_feature_named_windows_does_not_count(self):
        self.assertEqual(self.check('#[cfg(feature = "windows")]\nfn a() {}\n'), [])

    def test_tests_directory_is_scanned(self):
        (self.root / "pkg" / "tests").mkdir(parents=True)
        (self.root / "pkg" / "tests" / "t.rs").write_text('#[cfg(windows)]\n#[test]\nfn t() {}\n')
        self.assertEqual(len(self.check("fn a() {}\n")), 1)

    def test_the_current_tree_passes_the_guard(self):
        packages = self.ci.load_workspace()
        settings = self.ci.load_settings(packages)
        self.assertEqual(self.ci.platform_guard(packages, settings), [])


if __name__ == "__main__":
    unittest.main()
