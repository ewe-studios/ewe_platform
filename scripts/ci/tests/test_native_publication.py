"""Check release conflict detection and recovery without publishing packages."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[2] / "ui_native/release/publish.py"
spec = importlib.util.spec_from_file_location("publication", SOURCE)
publication = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publication)


class PublicationTests(unittest.TestCase):
    def test_changed_tag_fails_before_a_public_write(self):
        with patch.object(publication, "lookup", return_value={"object": {"type": "commit", "sha": "old"}}), \
             patch.object(publication, "gh") as gh:
            with self.assertRaisesRegex(ValueError, "Tag conflict"):
                publication.add_tag("org/repo", "0.1.0", "new")
            gh.assert_not_called()

    def test_existing_asset_is_immutable(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(publication, "gh") as gh:
            asset = Path(temp) / "test.zip"
            asset.write_bytes(b"test")
            assets = {asset.name: {"digest": "sha256:" + publication.digest(asset)}}
            publication.upload("org/repo", "tag", asset, assets)
            asset.write_bytes(b"different")
            with self.assertRaisesRegex(ValueError, "Asset conflict"):
                publication.upload("org/repo", "tag", asset, assets)
            gh.assert_not_called()

    def test_resume_uses_the_published_deployment(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            cfg = publication.tomllib.loads((publication.ROOT / "release/native-ui.toml").read_text())
            files = [directory / cfg["apple_archive"], directory / "android-publication.zip"]
            for file in files:
                file.write_text(file.name)
            record = {
                "repositories": {k: v for k, v in cfg.items() if k.endswith("repository")},
                "version": cfg["version"], "source_revision": "a",
                "swift_tag_revision": "a", "central_signed": True,
                "sha256": {f.name: publication.digest(f) for f in files},
            }
            manifest = directory / "native-ui.json"
            manifest.write_text(json.dumps(record))
            name = f"native-ui-v{cfg['version']}-" + publication.digest(manifest)
            receipt = {"name": name, "id": "2d3dc597-fff6-46af-9287-3a5cb6afdd76"}
            assets = [{"name": p.name, "digest": "sha256:" + publication.digest(p)} for p in [*files, manifest]]
            assets.append({"name": "central-deployment.json", "digest": ""})

            def lookup(route):
                if "/releases/" in route:
                    return {"draft": False, "assets": assets}
                return {"sha": "commit"}

            def gh(*args):
                self.assertEqual(args[:2], ("release", "download"))
                (Path(args[-1]) / "central-deployment.json").write_text(json.dumps(receipt))

            with patch.dict("os.environ", CENTRAL_PASSWORD="test", CENTRAL_USERNAME="test"), \
                 patch.object(publication, "gh", side_effect=gh), patch.object(publication, "lookup", side_effect=lookup), \
                 patch.object(publication, "check_tag", return_value=True), \
                 patch.object(publication, "central", return_value=json.dumps({"deploymentName": name, "deploymentState": "PUBLISHED"})) as portal:
                publication.publish(directory, 10, 1)
            self.assertEqual(portal.call_count, 1)
            self.assertEqual(portal.call_args.args[0], "status?id=" + receipt["id"])
