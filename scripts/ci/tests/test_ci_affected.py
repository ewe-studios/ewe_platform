"""Tests for the change-aware package selection (run: python3 -m unittest discover -s scripts/ci/tests)."""

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest import mock

REPO = Path(__file__).resolve().parents[3]
CI = REPO / "scripts" / "ci" / "ci.py"


def load_ci():
    spec = importlib.util.spec_from_file_location("ci_affected_under_test", CI)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


ROOT_TOML = """
[workspace]
members = ["core", "mid", "leaf"]
[workspace.dependencies]
serde = "1"
tokio = "1"
[profile.dev]
opt-level = 0
"""

LOCK = """
[[package]]
name = "serde"
version = "{serde}"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "tokio"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "core"
version = "{core}"
"""

CONFIG = """
[defaults]
fmt = "report"
[packages.leaf]
timeout-minutes = {leaf}
[packages.mid]
timeout-minutes = 30
"""


class Affected(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()
        P = self.ci.Package
        # leaf -> mid -> core; leaf uses serde; mid uses tokio.
        self.packages = {
            "core": P("core", "crates/core", False, dependents={"mid", "leaf"}, external={"serde"}),
            "mid": P("mid", "crates/mid", False, dependents={"leaf"}, external={"serde", "tokio"}, direct_deps={"tokio", "core"}),
            "leaf": P("leaf", "crates/leaf", False, external={"serde", "tokio"}, direct_deps={"mid", "serde"}),
            "alone": P("alone", "crates/alone", False),
        }
        self.packages["core"].direct_deps = {"serde"}

    def reasons(self, files, old=None, new=None):
        old, new = old or {}, new or {}
        changes = self.ci.Changes(files, old.get, new.get)
        return self.ci.affected_with_reasons(changes, self.packages)

    def test_a_leaf_change_marks_only_the_leaf(self):
        self.assertEqual(set(self.reasons(["crates/leaf/src/lib.rs"])), {"leaf"})

    def test_a_core_change_marks_the_dependents(self):
        reasons = self.reasons(["crates/core/src/lib.rs"])
        self.assertEqual(set(reasons), {"core", "mid", "leaf"})
        self.assertIn("depends on core", reasons["leaf"])
        self.assertIn("crates/core/src/lib.rs", reasons["mid"])

    def test_the_longest_directory_prefix_wins(self):
        self.packages["inner"] = self.ci.Package("inner", "crates/core/integration/inner", False)
        self.assertEqual(set(self.reasons(["crates/core/integration/inner/a.rs"])), {"inner"})

    def test_a_lock_change_marks_only_the_users_of_the_crate(self):
        old = {"Cargo.lock": LOCK.format(serde="1.0.1", core="0.1.0")}
        new = {"Cargo.lock": LOCK.format(serde="1.0.1", core="0.1.0").replace("1.0.0", "1.0.9")}
        reasons = self.reasons(["Cargo.lock"], old, new)  # tokio 1.0.0 -> 1.0.9
        self.assertEqual(set(reasons), {"mid", "leaf"})
        self.assertIn("tokio", reasons["mid"])

    def test_a_lock_change_of_a_workspace_crate_marks_nothing(self):
        old = {"Cargo.lock": LOCK.format(serde="1.0.1", core="0.1.0")}
        new = {"Cargo.lock": LOCK.format(serde="1.0.1", core="0.2.0")}
        self.assertEqual(self.reasons(["Cargo.lock"], old, new), {})

    def test_an_unreadable_lock_marks_everything(self):
        self.assertEqual(set(self.reasons(["Cargo.lock"], {"Cargo.lock": "x"}, {"Cargo.lock": "[["})), set(self.packages))

    def test_a_global_file_marks_everything(self):
        for path in ("rust-toolchain.toml", ".cargo/config.toml", "scripts/ci/ci.py", "mise.toml"):
            self.assertEqual(set(self.reasons([path])), set(self.packages), path)

    def test_a_docs_or_prose_change_marks_nothing(self):
        self.assertEqual(self.reasons(["docs/a.txt", "specifications/x/y.rs", "README.md", "crates/leaf/README.md"]), {})

    def test_a_workflow_other_than_check_marks_nothing(self):
        self.assertEqual(self.reasons([".github/workflows/release.yaml"]), {})

    def test_root_manifest_profile_change_marks_everything(self):
        new = ROOT_TOML.replace("opt-level = 0", "opt-level = 1")
        self.assertEqual(set(self.reasons(["Cargo.toml"], {"Cargo.toml": ROOT_TOML}, {"Cargo.toml": new})), set(self.packages))

    def test_a_workspace_dependency_change_marks_its_users(self):
        new = ROOT_TOML.replace('tokio = "1"', 'tokio = "2"')
        reasons = self.reasons(["Cargo.toml"], {"Cargo.toml": ROOT_TOML}, {"Cargo.toml": new})
        self.assertEqual(set(reasons), {"mid", "leaf"})

    def test_config_defaults_change_marks_everything(self):
        old = CONFIG.format(leaf=10)
        new = old.replace('fmt = "report"', 'fmt = "enforce"')
        self.assertEqual(set(self.reasons(["ci/packages.toml"], {"ci/packages.toml": old}, {"ci/packages.toml": new})), set(self.packages))

    def test_one_package_section_marks_only_that_package(self):
        old, new = CONFIG.format(leaf=10), CONFIG.format(leaf=20)
        reasons = self.reasons(["ci/packages.toml"], {"ci/packages.toml": old}, {"ci/packages.toml": new})
        self.assertEqual(set(reasons), {"leaf"})

    def test_a_new_package_section_marks_that_package(self):
        old = CONFIG.format(leaf=10)
        new = old + '\n[packages.alone]\ntimeout-minutes = 5\n'
        reasons = self.reasons(["ci/packages.toml"], {"ci/packages.toml": old}, {"ci/packages.toml": new})
        self.assertEqual(set(reasons), {"alone"})

    def test_the_root_package_owns_only_its_source_directories(self):
        self.packages["root"] = self.ci.Package("root", ".", False)
        self.assertEqual(set(self.reasons(["src/main.rs"])), {"root"})
        self.assertEqual(self.reasons(["docs/x.txt"]), {})


class IfAffected(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()
        self.packages = {
            "a": self.ci.Package("a", "crates/a", False),
            "b": self.ci.Package("b", "crates/b", False),
        }

    def run_with(self, name, reasons):
        with mock.patch.object(self.ci, "load_workspace", return_value=self.packages), \
             mock.patch.object(self.ci, "resolve_base", return_value="BASE"), \
             mock.patch.object(self.ci, "compute_affected", return_value=reasons):
            return self.ci.not_affected(name, None, None)

    def test_an_unaffected_package_is_skipped(self):
        self.assertTrue(self.run_with("b", {"a": "x"}))

    def test_an_affected_package_runs(self):
        self.assertFalse(self.run_with("a", {"a": "x"}))

    def test_a_git_failure_runs_the_package(self):
        with mock.patch.object(self.ci, "load_workspace", return_value=self.packages), \
             mock.patch.object(self.ci, "resolve_base", side_effect=SystemExit("no base")):
            self.assertFalse(self.ci.not_affected("a", None, None))

    def test_the_skip_line_and_exit_code(self):
        with mock.patch.object(self.ci, "not_affected", return_value=True) as skip, \
             mock.patch.object(self.ci, "run_package", side_effect=AssertionError("must not run")):
            args = mock.Mock(if_affected=True, package="b", base=None, head=None, step=None, profile="uat")
            self.assertEqual(self.ci.cmd_run(args), 0)
            skip.assert_called_once()

    def test_without_the_flag_the_package_runs(self):
        with mock.patch.object(self.ci, "run_package", return_value=0) as run:
            args = mock.Mock(if_affected=False, package="b", base=None, head=None, step=None, profile="uat")
            self.assertEqual(self.ci.cmd_run(args), 0)
            run.assert_called_once()


class GitFixture(unittest.TestCase):
    """The git side, on a temporary repository."""

    def test_git_changes_reads_the_diff_and_the_old_and_new_text(self):
        import subprocess, tempfile

        ci = load_ci()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            run = lambda *a: subprocess.run(["git", *a], cwd=root, check=True, capture_output=True, text=True)  # noqa: E731
            run("init", "-q", "-b", "master")
            run("config", "user.email", "t@example.com")
            run("config", "user.name", "t")
            (root / "a").mkdir()
            (root / "a" / "f.rs").write_text("one\n")
            (root / "x.txt").write_text("x\n")
            run("add", "-A")
            run("commit", "-qm", "base")
            (root / "a" / "f.rs").write_text("two\n")
            run("commit", "-qam", "change")
            (root / "new.rs").write_text("untracked\n")
            with mock.patch.object(ci, "ROOT", root):
                committed = ci.git_changes("HEAD~1", "HEAD")
                self.assertEqual(committed.files, ["a/f.rs"])
                self.assertEqual(committed.old("a/f.rs"), "one\n")
                self.assertEqual(committed.new("a/f.rs"), "two\n")
                local = ci.git_changes("HEAD~1", "HEAD", include_local=True)
                self.assertEqual(local.files, ["a/f.rs", "new.rs"])


if __name__ == "__main__":
    unittest.main()
