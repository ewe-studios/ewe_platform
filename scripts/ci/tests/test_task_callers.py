"""Check that migrated build and release callers use the registered tasks."""
import pathlib
import re
import tomllib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]


class TaskCallers(unittest.TestCase):
    def test_make_and_mise_dispatch_each_workspace_operation(self):
        make = (ROOT / "Makefile").read_text()
        mise = tomllib.loads((ROOT / "mise.toml").read_text())["tasks"]
        for make_name, mise_name, task in (
            ("build-all", "build:all", "build"),
            ("build-release", "build:release", "build-release"),
            ("build-wasm", "build:wasm", "build-wasm"),
            ("check", "check", "check"),
            ("clippy", "clippy", "clippy"),
            ("fmt", "fmt", "fmt"),
            ("fmt-check", "fmt:check", "fmt-check"),
            ("audit", "audit", "audit"),
            ("doc", "doc", "doc"),
            ("doc-open", "doc:open", "doc-open"),
            ("doc-nostd", "doc:nostd", "doc-nostd"),
            ("publish", "publish", "publish"),
        ):
            with self.subTest(task=task):
                recipe = re.search(rf"(?m)^{make_name}:\n((?:\t[^\n]*\n)+)", make)
                self.assertIsNotNone(recipe)
                prefix = "cargo run --locked --profile uat -p foundation_ci -- task run workspace:"
                suffix = " $(PACKAGES)" if task == "publish" else ""
                self.assertEqual(recipe[1].strip().removeprefix("@"), prefix + task + suffix)
                expected = "build-wasm-matrix" if task == "build-wasm" else task
                command = mise[mise_name]["run"].split()
                packages = []
                if task == "publish":
                    packages = re.search(r"(?m)^PACKAGES = (.+)$", make)[1].split()
                self.assertEqual(command, (prefix + expected).split() + packages)
