"""Derive a Swift release commit that binds the package to the candidate archive."""
import os
from pathlib import Path
import re
import subprocess
import tempfile

MESSAGE = "Set the native library checksum for this release"


def git(repo, *args, env=None):
    return subprocess.check_output(["git", "-C", repo, *args], env=env, text=True).strip()


def build_commit(repo, source, manifest):
    git(repo, "checkout", "--quiet", "--detach", source)
    package = Path(repo) / "Package.swift"
    if package.read_text() == manifest:
        return source
    package.write_text(manifest)
    git(repo, "add", "--", "Package.swift")
    date = git(repo, "show", "-s", "--format=%ct", source) + " +0000"
    env = os.environ.copy()
    for person in ("AUTHOR", "COMMITTER"):
        for key, value in (("NAME", "Native UI Release"), ("EMAIL", "native-ui-release@users.noreply.github.com"), ("DATE", date)):
            env[f"GIT_{person}_{key}"] = value
    git(repo, "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "commit", "--quiet", "-m", MESSAGE, env=env)
    return git(repo, "rev-parse", "HEAD")


def release_manifest(checkout, source, url, checksum):
    original = (checkout / "Package.swift").read_text()
    if url not in original:
        raise ValueError("Swift package references a different release URL")
    pattern = r'(url:\s*"' + re.escape(url) + r'",\s*checksum:\s*")[a-f0-9]{64}(")'
    manifest, count = re.subn(pattern, lambda match: match[1] + checksum + match[2], original)
    if count != 1:
        raise ValueError("Expected one checksum for the release URL")
    if manifest == original:
        return source, manifest
    with tempfile.TemporaryDirectory() as temp:
        subprocess.run(["git", "clone", "--quiet", "--no-checkout", str(checkout), temp], check=True)
        return build_commit(temp, source, manifest), manifest
