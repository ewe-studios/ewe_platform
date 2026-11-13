"""Tests for the per-package wasm deny-warnings gate (run: python3 -m unittest discover -s scripts/ci/tests)."""

import importlib.util
import sys
import tomllib
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
CI = REPO / "scripts" / "ci" / "ci.py"
CONFIG = REPO / "ci" / "packages.toml"


def load_ci():
    spec = importlib.util.spec_from_file_location("ci_under_test", CI)
    module = importlib.util.module_from_spec(spec)
    # dataclasses resolves the module's string annotations through sys.modules.
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def settings_for(ci, name: str):
    config = tomllib.loads(CONFIG.read_text())
    return ci.settings_for(name, config.get("defaults", {}), config["packages"][name])


class WasmDenyWarnings(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()

    def test_foundation_connectrpc_wasm_step_denies_warnings(self):
        settings = settings_for(self.ci, "foundation_connectrpc")
        self.assertTrue(settings.wasm_deny_warnings)

        wasm = self.ci.commands("foundation_connectrpc", settings, "uat")["wasm"]

        self.assertTrue(wasm)
        for entry in wasm:
            self.assertIsInstance(entry, self.ci.PlannedCommand)
            self.assertEqual(entry.env, {"RUSTFLAGS": "-D warnings"})
            self.assertEqual(
                entry.argv[:5],
                ["cargo", "check", "--target", "wasm32-unknown-unknown", "-p"],
            )

    def test_a_package_without_the_key_keeps_plain_commands(self):
        settings = settings_for(self.ci, "foundation_connectrpc")
        settings.wasm_deny_warnings = False

        for entry in self.ci.commands("foundation_connectrpc", settings, "uat")["wasm"]:
            self.assertIsInstance(entry, list)

    def test_defaults_do_not_deny_wasm_warnings(self):
        config = tomllib.loads(CONFIG.read_text())
        self.assertNotIn("wasm-deny-warnings", config.get("defaults", {}))


if __name__ == "__main__":
    unittest.main()
