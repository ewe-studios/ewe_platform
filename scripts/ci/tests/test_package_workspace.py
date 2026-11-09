"""Tests for scripts/ci/package_workspace.py (run: python3 -m unittest discover -s scripts/ci/tests)."""

import importlib.util
import json
import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SCRIPT = REPO / "scripts" / "ci" / "package_workspace.py"


def load_script():
    spec = importlib.util.spec_from_file_location("package_workspace_under_test", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def dep(name: str, kind=None, req: str = "*", path: bool = True) -> dict:
    """Return one `cargo metadata` dependency entry."""
    entry = {"name": name, "kind": kind, "req": req}
    if path:
        entry["path"] = f"/ws/{name}"
    return entry


def package(name: str, deps: list, publish=None) -> dict:
    """Return one `cargo metadata` package entry."""
    return {"name": name, "publish": publish, "dependencies": deps}


class DevDependencyCycles(unittest.TestCase):
    def setUp(self):
        self.script = load_script()

    def test_path_and_version_dev_dependency_back_to_the_crate_is_a_cycle(self):
        packages = [
            package("nostd", [dep("core", kind="dev", req="^0.0.19")]),
            package("core", [dep("nostd", req="^0.0.7")]),
        ]
        self.assertEqual(
            self.script.dev_dependency_cycles(packages),
            [["nostd", "core", "nostd"]],
        )

    def test_the_cycle_shows_each_crate_on_an_indirect_path(self):
        packages = [
            package("nostd", [dep("core", kind="dev", req="^1")]),
            package("core", [dep("compact", kind="build", req="^1")]),
            package("compact", [dep("nostd", req="^1")]),
        ]
        self.assertEqual(
            self.script.dev_dependency_cycles(packages),
            [["nostd", "core", "compact", "nostd"]],
        )

    def test_path_only_dev_dependency_is_not_a_cycle(self):
        packages = [
            package("nostd", [dep("core", kind="dev")]),
            package("core", [dep("nostd", req="^0.0.7")]),
        ]
        self.assertEqual(self.script.dev_dependency_cycles(packages), [])

    def test_dev_dependency_with_no_way_back_is_not_a_cycle(self):
        packages = [
            package("nostd", [dep("testing", kind="dev", req="^1")]),
            package("testing", []),
        ]
        self.assertEqual(self.script.dev_dependency_cycles(packages), [])

    def test_dev_edges_do_not_close_a_cycle(self):
        packages = [
            package("nostd", [dep("core", kind="dev", req="^1")]),
            package("core", [dep("nostd", kind="dev", req="^1")]),
        ]
        self.assertEqual(self.script.dev_dependency_cycles(packages), [])

    def test_unpublishable_crate_is_skipped(self):
        packages = [
            package("tool", [dep("core", kind="dev", req="^1")], publish=[]),
            package("core", [dep("tool", req="^1")]),
        ]
        self.assertEqual(self.script.dev_dependency_cycles(packages), [])

    def test_registry_dev_dependency_is_not_a_cycle(self):
        packages = [
            package("nostd", [dep("core", kind="dev", req="^1", path=False)]),
            package("core", [dep("nostd", req="^1")]),
        ]
        self.assertEqual(self.script.dev_dependency_cycles(packages), [])

    def test_the_workspace_has_no_cycle(self):
        out = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            cwd=REPO,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        packages = json.loads(out)["packages"]
        self.assertEqual(self.script.dev_dependency_cycles(packages), [])


if __name__ == "__main__":
    unittest.main()
