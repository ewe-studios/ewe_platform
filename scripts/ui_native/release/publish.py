"""Publish a signed candidate and resume from the release assets."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import sys
import time
import tomllib
from urllib.parse import urlencode
from uuid import UUID

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts/ui_native"))
from swift_metadata import build_commit

PORTAL = "https://central.sonatype.com/api/v1/publisher/"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def gh(*args):
    return subprocess.check_output(["gh", *args], text=True).strip()


def lookup(route):
    result = subprocess.run(["gh", "api", route], capture_output=True, text=True)
    if result.returncode == 0:
        return json.loads(result.stdout)
    if "(HTTP 404)" in result.stderr:
        return None
    raise RuntimeError("GitHub lookup failed: " + result.stderr.strip())


def check_tag(repo, tag, revision):
    ref = lookup(f"repos/{repo}/git/ref/tags/{tag}")
    if ref:
        obj = ref["object"]
        while obj["type"] == "tag":
            obj = lookup(f"repos/{repo}/git/tags/{obj['sha']}")["object"]
        if obj["type"] != "commit" or obj["sha"] != revision:
            raise ValueError(f"Tag conflict for {repo} at {tag}")
    return ref is not None


def add_tag(repo, tag, revision):
    if not check_tag(repo, tag, revision):
        gh("api", "--method", "POST", f"repos/{repo}/git/refs",
           "-f", "ref=refs/tags/" + tag, "-f", "sha=" + revision)


def upload(repo, tag, path, assets):
    if path.name in assets:
        if assets[path.name]["digest"] != "sha256:" + digest(path):
            raise ValueError(f"Asset conflict: {path.name}")
    else:
        gh("release", "upload", tag, str(path), "--repo", repo)


def central(operation, token, options=()):
    result = subprocess.run(["curl", "--fail", "--silent", "--show-error", "--request", "POST",
                             "--header", "@-", *options, PORTAL + operation],
                            input="Authorization: Bearer " + token + "\n",
                            text=True, capture_output=True, check=True)
    return result.stdout.strip()


def publish(directory, timeout, interval):
    cfg = tomllib.loads((ROOT / "release/native-ui.toml").read_text())
    manifest = directory / "native-ui.json"
    rec = json.loads(manifest.read_text())
    repo, version = cfg["repository"], cfg["version"]
    tag = "native-ui-v" + version
    revisions = rec["revisions"]
    if rec["version"] != version or rec["repositories"] != {k: v for k, v in cfg.items() if k.endswith("repository")}:
        raise ValueError("Release configuration differs from the candidate")
    names = {cfg["apple_archive"], "android-publication.zip"}
    if set(rec["sha256"]) != names or not rec["central_signed"]:
        raise ValueError("Expected a signed Android bundle and an Apple archive")
    for name in names:
        if digest(directory / name) != rec["sha256"][name]:
            raise ValueError(f"Invalid candidate digest: {name}")
    token = base64.b64encode((os.environ["CENTRAL_USERNAME"] + ":" + os.environ["CENTRAL_PASSWORD"]).encode()).decode()
    tags = [(repo, tag, revisions["core"]), (cfg["swift_repository"], version, rec["swift_tag_revision"]),
            (cfg["kotlin_repository"], version, revisions["kotlin"])]
    metadata_commit = rec["swift_tag_revision"] != revisions["swift_source"]
    for target, name, sha in tags:
        check_tag(target, name, sha)
        input_sha = revisions["swift_source"] if target == cfg["swift_repository"] else sha
        if lookup(f"repos/{target}/git/commits/{input_sha}") is None:
            raise ValueError(f"Missing commit in {target}: {sha}")
    release = lookup(f"repos/{repo}/releases/tags/{tag}")
    files = [directory / "android-publication.zip", directory / cfg["apple_archive"], manifest]
    if release is None:
        add_tag(*tags[0])
        gh("release", "create", tag, "--repo", repo, "--verify-tag", "--draft", "--title", tag, "--notes", "Native user interface packages.")
        release = lookup(f"repos/{repo}/releases/tags/{tag}")
    assets = {a["name"]: a for a in release["assets"]}
    for item in files:
        if item.name in assets and assets[item.name]["digest"] != "sha256:" + digest(item):
            raise ValueError(f"Asset conflict: {item.name}")
    for item in files:
        upload(repo, tag, item, assets)
    deployment_name = tag + "-" + digest(manifest)
    with tempfile.TemporaryDirectory() as temp:
        receipt = Path(temp) / "central-deployment.json"
        if receipt.name in assets:
            gh("release", "download", tag, "--repo", repo, "--pattern", receipt.name, "--dir", temp)
            saved = json.loads(receipt.read_text())
            if saved["name"] != deployment_name:
                raise ValueError("Central receipt belongs to another candidate")
            deployment = str(UUID(saved["id"]))
        else:
            query = urlencode({"name": deployment_name, "publishingType": "USER_MANAGED"})
            deployment = str(UUID(central("upload?" + query, token, ["--form", "bundle=@" + str(directory / "android-publication.zip")])))
            receipt.write_text(json.dumps({"name": deployment_name, "id": deployment}) + "\n")
            upload(repo, tag, receipt, assets)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            status = json.loads(central("status?id=" + deployment, token))
            if status["deploymentName"] != deployment_name:
                raise ValueError("Central deployment name differs from the receipt")
            state = status["deploymentState"]
            if state == "PUBLISHED":
                break
            if state == "VALIDATED":
                central("deployment/" + deployment, token)
            elif state not in {"PENDING", "VALIDATING", "PUBLISHING"}:
                raise ValueError("Unexpected Central deployment state: " + state)
            time.sleep(interval)
        else:
            raise TimeoutError("Central deployment is still pending. Resume this candidate.")
    if release["draft"]:
        gh("release", "edit", tag, "--repo", repo, "--draft=false")
    swift_repo, swift_tag, swift_commit = tags[1]
    if metadata_commit and not check_tag(swift_repo, swift_tag, swift_commit):
        with tempfile.TemporaryDirectory() as temp:
            subprocess.run(["git", "clone", "--quiet", "--no-checkout", f"https://github.com/{swift_repo}.git", temp], check=True)
            if build_commit(temp, revisions["swift_source"], rec["swift_manifest"]) != swift_commit:
                raise ValueError("Swift release metadata differs from the staged commit")
            subprocess.run(["git", "-C", temp, "-c", "credential.helper=", "-c", "credential.helper=!gh auth git-credential",
                            "push", "origin", swift_commit + ":refs/tags/" + swift_tag], check=True)
    else:
        add_tag(*tags[1])
    add_tag(*tags[2])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--resume", action="store_true", help="Use the existing release assets when present")
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument("--poll-interval", type=int, default=15)
    args = parser.parse_args()
    publish(args.candidate, args.timeout, args.poll_interval)
