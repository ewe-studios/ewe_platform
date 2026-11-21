"""Verify the Swift tag can be reproduced from its recorded source and manifest."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "ui_native"))
from swift_metadata import build_commit, git, release_manifest


class SwiftMetadataTest(unittest.TestCase):
    def test_tag_only_changes_the_archive_checksum(self):
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / "source"
            source.mkdir()
            git(source, "init", "--quiet")
            url = "https://example.invalid/release.zip"
            old = "1" * 64
            new = "2" * 64
            original = f'.binaryTarget(name: "Archive", url: "{url}", checksum: "{old}")\n'
            (source / "Package.swift").write_text(original)
            git(source, "add", "Package.swift")
            git(source, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                "-c", "commit.gpgsign=false", "commit", "-qm", "Initial package")
            base = git(source, "rev-parse", "HEAD")
            self.assertEqual(release_manifest(source, base, url, old), (base, original))
            revision, manifest = release_manifest(source, base, url, new)
            self.assertNotEqual(revision, base)
            self.assertEqual(manifest, original.replace(old, new))
            self.assertEqual(git(source, "status", "--porcelain"), "")
            clone = Path(temp) / "tag"
            subprocess.run(["git", "clone", "--quiet", str(source), clone], check=True)
            self.assertEqual(build_commit(clone, base, manifest), revision)
            self.assertEqual(git(clone, "diff", "--name-only", base), "Package.swift")
