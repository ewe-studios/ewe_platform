"""The publisher removes a cyclic dev-dependency for one publish (issue #468).

`cargo publish` resolves every dependency, dev-dependencies included, when it
prepares the package; `--no-verify` does not skip that. The publisher comments
the one dev-dependency out, publishes, and restores the manifest. These tests
check the edit and the restore on both manifest forms.
"""

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SCRIPT = REPO / "scripts" / "ci" / "publish_crates.py"

INLINE = """[package]
name = "foundation_db"
version = "0.0.10"

[dependencies]
serde = "1"

[dev-dependencies]
foundation_nativeapis = { workspace = true }
tempfile = "3"
"""

TABLE = """[package]
name = "foundation_db"
version = "0.0.10"

[dev-dependencies.foundation_nativeapis]
path = "../foundation_nativeapis"
version = "0.0.16"

[dev-dependencies.tempfile]
version = "3"
"""


def load_script():
    spec = importlib.util.spec_from_file_location("publish_crates_under_test", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class WithoutDevDependencyTest(unittest.TestCase):
    def apply(self, text):
        script = load_script()
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp) / "Cargo.toml"
            manifest.write_text(text)
            original = script.without_dev_dependency(manifest, "foundation_nativeapis")
            edited = manifest.read_text()
            if original is not None:
                manifest.write_text(original)
            return original, edited, manifest.read_text()

    def test_inline_dev_dependency_is_commented_and_restored(self):
        original, edited, restored = self.apply(INLINE)
        self.assertEqual(original, INLINE)
        self.assertIn("# foundation_nativeapis = { workspace = true }", edited)
        self.assertIn('tempfile = "3"', edited)
        self.assertEqual(restored, INLINE)

    def test_table_dev_dependency_is_commented_and_restored(self):
        original, edited, restored = self.apply(TABLE)
        self.assertEqual(original, TABLE)
        self.assertIn("# [dev-dependencies.foundation_nativeapis]", edited)
        self.assertIn('# version = "0.0.16"', edited)
        self.assertIn("[dev-dependencies.tempfile]", edited)
        self.assertEqual(restored, TABLE)

    def test_absent_dependency_changes_nothing(self):
        original, edited, restored = self.apply(INLINE)
        script = load_script()
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp) / "Cargo.toml"
            manifest.write_text(INLINE)
            self.assertIsNone(script.without_dev_dependency(manifest, "other"))
            self.assertEqual(manifest.read_text(), INLINE)


if __name__ == "__main__":
    unittest.main()
