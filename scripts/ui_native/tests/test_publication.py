"""Verify release state changes against an in-memory registry."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
ENTRY = ROOT / "scripts/ui_native/release/publish.py"
SPEC = importlib.util.spec_from_file_location("publish", ENTRY)
publication = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publication)
DEPLOYMENT = "46a8097f-a23d-4974-a0c7-c935abf36660"
SIGNER = "A" * 40


class Publication(unittest.TestCase):
    def setUp(self):
        workspace = tempfile.TemporaryDirectory()
        self.addCleanup(workspace.cleanup)
        self.root = Path(workspace.name)
        (self.root / "release").mkdir()
        (self.root / "release/native-ui.toml").write_text(
            'repository = "demo/project"\nversion = "1.2.3"\napple_archive = "Apple.zip"\n'
        )
        self.candidate = self.root / "candidate"
        self.candidate.mkdir()
        files = ("Apple.zip", "android-publication.zip")
        for name in files:
            (self.candidate / name).write_bytes(name.encode())
        with zipfile.ZipFile(self.candidate / "android-publication.zip", "w") as archive:
            archive.writestr("sample/module.pom", b"package")
            archive.writestr("sample/module.pom.asc", b"mock signature")
        record = {
            "version": "1.2.3",
            "repositories": {"repository": "demo/project"},
            "source_revision": "source",
            "swift_tag_revision": "source",
            "central_signed": True,
            "sha256": {name: publication.digest(self.candidate / name) for name in files},
        }
        manifest = self.candidate / "native-ui.json"
        manifest.write_text(json.dumps(record))
        self.name = "native-ui-v1.2.3-" + publication.digest(manifest)
        self.assets = [
            {"name": name, "digest": "sha256:" + publication.digest(self.candidate / name)}
            for name in (*files, manifest.name)
        ]
        self.commands = []
        self.central = []
        self.patches = [
            patch.object(publication, "ROOT", self.root),
            patch.object(publication, "lookup", self.lookup),
            patch.object(publication, "gh", self.github),
            patch.object(publication, "central", self.maven),
            patch.object(publication.subprocess, "check_output", self.gpg),
            patch.dict(publication.os.environ, CENTRAL_USERNAME="mock", CENTRAL_PASSWORD="mock", NATIVE_GPG_FINGERPRINT=SIGNER),
        ]
        for rule in self.patches:
            rule.start()
            self.addCleanup(rule.stop)

    def lookup(self, route):
        if "/git/ref/" in route:
            return {"object": {"type": "commit", "sha": "source"}}
        if "/git/commits/" in route:
            return {"sha": "source"}
        if "/releases/tags/" in route:
            return {"draft": True, "assets": self.assets}
        raise AssertionError(route)

    def github(self, *args):
        self.commands.append(args)
        if args[:2] == ("release", "download"):
            directory = Path(args[args.index("--dir") + 1])
            asset = args[args.index("--pattern") + 1]
            (directory / asset).write_text(json.dumps({"name": self.name, "id": DEPLOYMENT}))
        elif args[:2] == ("release", "upload"):
            source = Path(args[3])
            self.assertEqual(source.name, "central-deployment-pending.json")
            self.assets.append({"name": source.name, "digest": "sha256:" + publication.digest(source)})
        elif args[:2] == ("release", "edit"):
            pass
        else:
            raise AssertionError(args)

    def gpg(self, args, **kwargs):
        self.assertEqual(args[:5], ["gpg", "--batch", "--status-fd", "1", "--verify"])
        self.assertEqual(args[5].read_bytes(), b"mock signature")
        self.assertEqual(args[6].read_bytes(), b"package")
        return f"[GNUPG:] VALIDSIG {SIGNER} signer"

    def maven(self, operation, token, options=()):
        self.central.append(operation)
        self.assertEqual(operation, "status?id=" + DEPLOYMENT)
        return json.dumps({"deploymentName": self.name, "deploymentState": "PUBLISHED"})

    def test_resume_does_not_upload_another_deployment(self):
        self.assets.append({"name": "central-deployment.json", "digest": "unused"})
        publication.publish(self.candidate, 1, 1)
        self.assertEqual([command[:2] for command in self.commands], [("release", "download"), ("release", "edit")])
        self.assertTrue(self.central)
        self.assertTrue(all(operation.startswith("status?") for operation in self.central))

    def test_untrusted_signer_does_not_write(self):
        with patch.dict(publication.os.environ, NATIVE_GPG_FINGERPRINT="B" * 40):
            with self.assertRaisesRegex(ValueError, "Unexpected signer"):
                publication.publish(self.candidate, 1, 1)
        self.assertEqual(self.commands, [])
        self.assertEqual(self.central, [])

    def test_remote_digest_conflict_does_not_write(self):
        self.assets[0]["digest"] = "sha256:other"
        with self.assertRaisesRegex(ValueError, "Asset conflict"):
            publication.publish(self.candidate, 1, 1)
        self.assertEqual(self.commands, [])
        self.assertEqual(self.central, [])

    def test_ambiguous_upload_is_reconciled_before_retry(self):
        with patch.object(publication, "central", side_effect=TimeoutError("Connection interrupted")) as central:
            with self.assertRaisesRegex(RuntimeError, "outcome is unresolved"):
                publication.publish(self.candidate, 1, 1)
            # The next runner has a fresh local workspace and can only see the remote marker.
            (self.candidate / ".central-deployment-pending.json").unlink()
            with self.assertRaisesRegex(RuntimeError, "outcome is unresolved"):
                publication.publish(self.candidate, 1, 1)
            self.assertEqual(central.call_count, 1)
        self.assertEqual([command[:2] for command in self.commands], [("release", "upload"), ("release", "download")])


if __name__ == "__main__":
    unittest.main()
