"""Tests for the manifest contract checks (spec 78).

Run: python3 -m unittest discover -s scripts/ci/tests
"""

import importlib.util
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MODULE = REPO / "scripts" / "ci" / "manifest_contract.py"
CI = REPO / "scripts" / "ci" / "ci.py"
SPEC = REPO / "specifications" / "78-manifest-contract"


def load(module_path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, module_path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def load_manifest_contract():
    return load(MODULE, "manifest_contract_under_test")


def load_ci():
    return load(CI, "ci_manifest_under_test")


class SchemaAndFixtures(unittest.TestCase):
    def setUp(self):
        self.mc = load_manifest_contract()

    def test_the_schema_and_every_fixture_pass(self):
        self.assertEqual(self.mc.check_schema(out=lambda *_a: None), 0)

    def test_each_valid_fixture_passes_the_schema_and_the_semantic_rules(self):
        schema = self.mc.load_json(SPEC / "manifest.schema.json")
        for path in sorted((SPEC / "fixtures" / "valid").glob("*.json")):
            with self.subTest(path.name):
                self.assertEqual(self.mc.manifest_faults(self.mc.load_json(path), schema), [])

    def test_each_invalid_fixture_fails_at_the_recorded_path(self):
        schema = self.mc.load_json(SPEC / "manifest.schema.json")
        for name, (expected_path, _code) in self.mc.EXPECTED_INVALID.items():
            with self.subTest(name):
                doc = self.mc.load_json(SPEC / "fixtures" / "invalid" / name)
                paths = {fault.path for fault in self.mc.manifest_faults(doc, schema)}
                self.assertIn(expected_path, paths)

    def test_a_duplicate_name_is_a_semantic_fault(self):
        doc = self.mc.load_json(SPEC / "fixtures" / "invalid" / "v5-duplicate-name.json")
        faults = self.mc.semantic_faults(doc)
        self.assertEqual([fault.path for fault in faults], ["/entries/1/name"])

    def test_an_unknown_source_kind_is_a_semantic_fault(self):
        doc = self.mc.load_json(SPEC / "fixtures" / "invalid" / "v6-unknown-kind.json")
        faults = self.mc.semantic_faults(doc)
        self.assertEqual([fault.path for fault in faults], ["/entries/0/source/kind"])

    def test_an_unknown_field_is_accepted(self):
        schema = self.mc.load_json(SPEC / "manifest.schema.json")
        doc = self.mc.load_json(SPEC / "fixtures" / "valid" / "unknown-field.json")
        self.assertEqual(self.mc.manifest_faults(doc, schema), [])

    def test_json_equality_keeps_true_apart_from_one(self):
        self.assertFalse(self.mc.json_equal(True, 1))
        self.assertFalse(self.mc.json_equal(1, True))
        self.assertTrue(self.mc.json_equal(1, 1.0))
        self.assertTrue(self.mc.json_equal({"a": [1]}, {"a": [1.0]}))

    def test_the_schema_document_check_rejects_a_wrong_draft(self):
        schema = self.mc.load_json(SPEC / "manifest.schema.json")
        schema["$schema"] = "http://json-schema.org/draft-07/schema#"
        self.assertTrue(self.mc.schema_faults(schema))

    def test_the_schema_document_check_rejects_a_dangling_reference(self):
        schema = self.mc.load_json(SPEC / "manifest.schema.json")
        schema["$defs"]["entry"]["properties"]["source"]["$ref"] = "#/$defs/missing"
        faults = self.mc.schema_faults(schema)
        self.assertTrue(any("does not resolve" in fault.message for fault in faults))

    def test_every_documented_default_is_in_the_schema(self):
        schema = self.mc.load_json(SPEC / "manifest.schema.json")
        definitions = schema["$defs"]
        expected = {
            ("deploySystemd", "args"): [],
            ("deploySystemd", "env"): {},
            ("deploySystemd", "user"): "ewe",
            ("deploySystemd", "restart"): "on-failure",
            ("deployDocker", "ports"): [],
            ("deployDocker", "env"): {},
            ("deployDocker", "volumes"): [],
            ("deployDocker", "restart"): "unless-stopped",
            ("proxy", "weight"): 1,
            ("proxyRoute", "path_prefix"): "/",
            ("probeTiming", "period_secs"): 10,
            ("probeTiming", "timeout_secs"): 1,
            ("probeTiming", "failure_threshold"): 3,
            ("probeTiming", "success_threshold"): 1,
            ("probeTiming", "initial_delay_secs"): 0,
        }
        for (definition, field), value in expected.items():
            with self.subTest(definition=definition, field=field):
                self.assertEqual(definitions[definition]["properties"][field]["default"], value)


class FixtureCopies(unittest.TestCase):
    def setUp(self):
        self.mc = load_manifest_contract()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.spec = self.root / "specifications" / "78-manifest-contract"
        (self.spec / "fixtures" / "valid").mkdir(parents=True)
        shutil.copy(
            SPEC / "fixtures" / "valid" / "empty.json",
            self.spec / "fixtures" / "valid" / "empty.json",
        )
        self.consumer = (
            self.root
            / "backends"
            / "foundation_installer"
            / "tests"
            / "fixtures"
            / "manifest"
        )
        self.copy = self.consumer / "valid" / "empty.json"
        self.copy.parent.mkdir(parents=True)

    def test_no_consumer_directories_is_not_a_fault(self):
        self.assertEqual(self.mc.copy_faults(self.root, self.spec), [])

    def test_an_identical_copy_passes(self):
        shutil.copy(self.spec / "fixtures" / "valid" / "empty.json", self.copy)
        self.assertEqual(self.mc.copy_faults(self.root, self.spec), [])

    def test_a_one_byte_drift_fails_and_names_the_copy(self):
        shutil.copy(self.spec / "fixtures" / "valid" / "empty.json", self.copy)
        self.copy.write_bytes(self.copy.read_bytes().replace(b"1", b"2", 1))
        faults = self.mc.copy_faults(self.root, self.spec)
        self.assertEqual(len(faults), 1)
        self.assertIn("backends/foundation_installer/tests/fixtures/manifest/valid/empty.json", faults[0])
        self.assertIn("drifted", faults[0])

    def test_a_copy_without_a_canonical_file_fails(self):
        self.copy = self.consumer / "valid" / "not-canonical.json"
        self.copy.write_text("{}\n")
        faults = self.mc.copy_faults(self.root, self.spec)
        self.assertEqual(len(faults), 1)
        self.assertIn("no canonical file", faults[0])

    def test_the_check_copies_command_fails_on_drift(self):
        shutil.copy(self.spec / "fixtures" / "valid" / "empty.json", self.copy)
        self.copy.write_text('{"manifest_version": 9}\n')
        self.assertEqual(self.mc.check_copies(self.root, self.spec, out=lambda *_a: None), 1)

    def test_the_check_copies_command_passes_on_an_identical_copy(self):
        shutil.copy(self.spec / "fixtures" / "valid" / "empty.json", self.copy)
        self.assertEqual(self.mc.check_copies(self.root, self.spec, out=lambda *_a: None), 0)


class CommandLine(unittest.TestCase):
    def test_a_flag_is_required(self):
        with self.assertRaises(SystemExit):
            load_manifest_contract().main([])


class CiSelection(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()

    def test_a_canonical_fixture_change_selects_the_manifest_contract_package(self):
        packages = {
            "manifest-contract": self.ci.Package(
                "manifest-contract", "specifications/78-manifest-contract", False
            )
        }
        changes = self.ci.Changes(
            ["specifications/78-manifest-contract/fixtures/valid/complete.json"]
        )
        self.assertEqual(
            set(self.ci.affected_with_reasons(changes, packages)), {"manifest-contract"}
        )

    def test_a_consumer_copy_change_selects_the_check_and_the_reader(self):
        packages = {
            "manifest-contract": self.ci.Package(
                "manifest-contract", "specifications/78-manifest-contract", False
            ),
            "foundation_installer": self.ci.Package(
                "foundation_installer", "backends/foundation_installer", False
            ),
        }
        changes = self.ci.Changes(
            ["backends/foundation_installer/tests/fixtures/manifest/valid/complete.json"]
        )
        self.assertEqual(
            set(self.ci.affected_with_reasons(changes, packages)),
            {"manifest-contract", "foundation_installer"},
        )

    def test_the_plan_builds_the_two_checks(self):
        packages = self.ci.load_workspace()
        self.assertIn("manifest-contract", packages)
        settings = self.ci.load_settings(packages)["manifest-contract"]
        plan = self.ci.commands("manifest-contract", settings, "uat", "linux")
        self.assertEqual(
            plan["build"],
            [
                [sys.executable, "scripts/ci/manifest_contract.py", "--check-schema"],
                [sys.executable, "scripts/ci/manifest_contract.py", "--check-copies"],
            ],
        )
        for step in ("fmt", "clippy", "test", "wasm"):
            self.assertEqual(plan[step], [], step)


if __name__ == "__main__":
    unittest.main()
