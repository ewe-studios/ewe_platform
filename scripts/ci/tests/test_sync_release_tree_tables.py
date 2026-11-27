"""The release sync rewrites target-gated dependency tables too (issue #873).

The first sync script rewrote inline tables and plain dependency tables only.
Three Firecracker consumers keep `foundation_netio` in a
`[target.'cfg(...)'.dependencies.X]` table, and the script left them stale
without a word, because its check mode looked only at inline tables.
"""

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SCRIPT = REPO / "scripts" / "ci" / "sync_release_tree.py"

TABLE = """[package]
name = "foundation_vectors"
version = "0.0.7"

[target.'cfg(not(target_family = "wasm"))'.dependencies.foundation_netio]
path = "../foundation_netio"
version = "0.0.18"

[target."cfg(unix)".dependencies.foundation_netio]
path = "../foundation_netio"
version = "0.0.18"
features = ["wasm-fetch"]
"""


def load_script():
    spec = importlib.util.spec_from_file_location("sync_tree_under_test", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class TargetTableSyncTest(unittest.TestCase):
    def test_check_reports_a_stale_target_table(self):
        script = load_script()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "foundation_vectors").mkdir()
            manifest = root / "foundation_vectors" / "Cargo.toml"
            manifest.write_text(TABLE)
            stale = script.stale_paths(root, {"foundation_netio": "0.0.19"})
            self.assertEqual(len(stale), 2, stale)
            self.assertTrue(all("foundation_netio" in line for line in stale))
            self.assertTrue(any("dependencies.foundation_netio" in line for line in stale))

    def test_sync_rewrites_target_tables_and_keeps_the_headers(self):
        script = load_script()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "foundation_vectors").mkdir()
            manifest = root / "foundation_vectors" / "Cargo.toml"
            manifest.write_text(TABLE)
            updated = script.sync_manifest(manifest, {"foundation_netio": "0.0.19"})
            self.assertNotEqual(updated, "")
            manifest.write_text(updated)
            text = manifest.read_text()
            self.assertEqual(text.count('version = "0.0.19"'), 2, text)
            self.assertIn("[target.'cfg(not(target_family = \"wasm\"))'.dependencies.foundation_netio]", text)
            self.assertIn('[target."cfg(unix)".dependencies.foundation_netio]', text)
            self.assertIn('features = ["wasm-fetch"]', text)
            self.assertEqual(script.stale_paths(root, {"foundation_netio": "0.0.19"}), [])


if __name__ == "__main__":
    unittest.main()
