"""Publish a signed candidate and resume from the release assets."""
import argparse
import base64
import hashlib
import json
import os
import shutil
from pathlib import Path
import subprocess
import tempfile
import sys
import time
import tomllib
from urllib.parse import urlencode
from uuid import UUID
import zipfile

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




def check_signatures(path, fingerprint):
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise ValueError("Android bundle has duplicate members")
        products = {name for name in names if not name.endswith((".asc", ".md5", ".sha1", ".sha256", ".sha512"))}
        signatures = {name[:-4] for name in names if name.endswith(".asc")}
        if not products or products != signatures:
            raise ValueError("Android bundle requires a signature for each product")
        with tempfile.TemporaryDirectory() as directory:
            data = Path(directory) / "product"
            signature = Path(directory) / "signature"
            for name in sorted(products):
                for key, file in [(name, data), (name + ".asc", signature)]:
                    with archive.open(key) as src, file.open("wb") as dest:
                        shutil.copyfileobj(src, dest)
                status = subprocess.check_output(
                    ["gpg", "--batch", "--status-fd", "1", "--verify", signature, data], text=True
                )
                matches = [line.split() for line in status.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
                if not any(fingerprint.upper() in (line[2], line[-1]) for line in matches):
                    raise ValueError(f"Unexpected signer for {name}")

def read_receipt(path, expected_name):
    record = json.loads(path.read_text())
    if record["name"] != expected_name:
        raise ValueError("Central receipt belongs to another candidate")
    return str(UUID(record["id"]))


def store_receipt(path, record):
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as output:
        stage = Path(output.name)
        try:
            json.dump(record, output)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
            output.close()
            stage.replace(path)
        finally:
            stage.unlink(missing_ok=True)


def central_status(deployment, name, token):
    status = json.loads(central("status?id=" + deployment, token))
    if status["deploymentName"] != name:
        raise ValueError("Central deployment name differs from the receipt")
    return status["deploymentState"]


def reconcile(directory, repo, tag, assets, receipt, name, token):
    local = directory / ".central-deployment.json"
    pending = directory / ".central-deployment-pending.json"
    deployment = read_receipt(local, name) if local.is_file() else None
    if receipt.name in assets:
        gh("release", "download", tag, "--repo", repo, "--pattern", receipt.name, "--dir", str(receipt.parent))
        remote = read_receipt(receipt, name)
        if deployment is not None and deployment != remote:
            raise ValueError("Local and remote deployment receipts differ")
        deployment = remote
    published = False
    if deployment is not None:
        published = central_status(deployment, name, token) == "PUBLISHED"
    elif pending.exists() or "central-deployment-pending.json" in assets:
        if not pending.exists():
            gh("release", "download", tag, "--repo", repo, "--pattern", "central-deployment-pending.json",
               "--dir", str(receipt.parent))
            saved = json.loads((receipt.parent / "central-deployment-pending.json").read_text())
            if saved["name"] != name:
                raise ValueError("Central upload attempt belongs to another candidate")
            store_receipt(pending, saved)
        raise RuntimeError(f"Central upload outcome is unresolved. Inspect {pending} and supply the matching receipt.")
    return deployment, local, pending, published

def publish(directory, timeout, interval):
    cfg = tomllib.loads((ROOT / "release/native-ui.toml").read_text())
    manifest = directory / "native-ui.json"
    rec = json.loads(manifest.read_text())
    repo, version = cfg["repository"], cfg["version"]
    tag = "native-ui-v" + version
    source = rec["source_revision"]
    if rec["version"] != version or rec["repositories"] != {k: v for k, v in cfg.items() if k.endswith("repository")}:
        raise ValueError("Release configuration differs from the candidate")
    names = {cfg["apple_archive"], "android-publication.zip"}
    if set(rec["sha256"]) != names or not rec["central_signed"]:
        raise ValueError("Expected a signed Android bundle and an Apple archive")
    for name in names:
        if digest(directory / name) != rec["sha256"][name]:
            raise ValueError(f"Invalid candidate digest: {name}")
    fingerprint = os.environ.get("NATIVE_GPG_FINGERPRINT")
    if not fingerprint:
        raise ValueError("Configure the trusted OpenPGP fingerprint in NATIVE_GPG_FINGERPRINT")
    check_signatures(directory / "android-publication.zip", fingerprint)
    token = base64.b64encode((os.environ["CENTRAL_USERNAME"] + ":" + os.environ["CENTRAL_PASSWORD"]).encode()).decode()
    tags = [(repo, tag, source), (repo, version, rec["swift_tag_revision"])]
    metadata_commit = rec["swift_tag_revision"] != source
    for target, name, sha in tags:
        check_tag(target, name, sha)
    if lookup(f"repos/{repo}/git/commits/{source}") is None:
        raise ValueError(f"Missing commit in {repo}: {source}")
    release = lookup(f"repos/{repo}/releases/tags/{tag}")
    files = [directory / "android-publication.zip", directory / cfg["apple_archive"], manifest]
    assets = {a["name"]: a for a in release["assets"]} if release else {}
    for item in files:
        if item.name in assets and assets[item.name]["digest"] != "sha256:" + digest(item):
            raise ValueError(f"Asset conflict: {item.name}")
    deployment_name = tag + "-" + digest(manifest)
    with tempfile.TemporaryDirectory() as temp:
        receipt = Path(temp) / "central-deployment.json"
        deployment, local_receipt, pending, published = reconcile(
            directory, repo, tag, assets, receipt, deployment_name, token
        )
        if release is None:
            add_tag(*tags[0])
            gh("release", "create", tag, "--repo", repo, "--verify-tag", "--draft", "--title", tag, "--notes", "Native user interface packages.")
            release = lookup(f"repos/{repo}/releases/tags/{tag}")
            if release is None:
                raise RuntimeError("GitHub did not return the draft release")
            assets = {a["name"]: a for a in release["assets"]}
        for item in files:
            upload(repo, tag, item, assets)
        pending_asset = "central-deployment-pending.json"
        pending_remote = pending_asset in assets
        if deployment is None:
            attempt = {"name": deployment_name}
            store_receipt(pending, attempt)
            remote_attempt = Path(temp) / pending_asset
            store_receipt(remote_attempt, attempt)
            upload(repo, tag, remote_attempt, assets)
            pending_remote = True
            query = urlencode({"name": deployment_name, "publishingType": "USER_MANAGED"})
            try:
                deployment = str(UUID(central("upload?" + query, token, ["--form", "bundle=@" + str(directory / "android-publication.zip")])))
            except Exception as error:
                raise RuntimeError(f"Central upload outcome is unresolved. Inspect {pending} before retrying.") from error
        if receipt.name not in assets:
            saved = {"name": deployment_name, "id": deployment}
            store_receipt(local_receipt, saved)
            store_receipt(receipt, saved)
            upload(repo, tag, receipt, assets)
        if pending_remote:
            gh("release", "delete-asset", tag, pending_asset, "--repo", repo, "--yes")
        pending.unlink(missing_ok=True)
        deadline = time.monotonic() + timeout
        while not published:
            if time.monotonic() >= deadline:
                raise TimeoutError("Central deployment is still pending. Resume this candidate.")
            state = central_status(deployment, deployment_name, token)
            if state == "PUBLISHED":
                break
            if state == "VALIDATED":
                central("deployment/" + deployment, token)
            elif state not in {"PENDING", "VALIDATING", "PUBLISHING"}:
                raise ValueError("Unexpected Central deployment state: " + state)
            time.sleep(interval)
    if release["draft"]:
        gh("release", "edit", tag, "--repo", repo, "--draft=false")
    swift_repo, swift_tag, swift_commit = tags[1]
    if metadata_commit and not check_tag(swift_repo, swift_tag, swift_commit):
        with tempfile.TemporaryDirectory() as temp:
            subprocess.run(["git", "clone", "--quiet", "--no-checkout", f"https://github.com/{swift_repo}.git", temp], check=True)
            if build_commit(temp, source, rec["swift_manifest"]) != swift_commit:
                raise ValueError("Swift release metadata differs from the staged commit")
            subprocess.run(["git", "-C", temp, "-c", "credential.helper=", "-c", "credential.helper=!gh auth git-credential",
                            "push", "origin", swift_commit + ":refs/tags/" + swift_tag], check=True)
    else:
        add_tag(*tags[1])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--resume", action="store_true", help="Use the existing release assets when present")
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument("--poll-interval", type=int, default=15)
    args = parser.parse_args()
    publish(args.candidate, args.timeout, args.poll_interval)
