"""Sign the verified Maven files from the read-only build job."""
import argparse
import json
from pathlib import Path
import shutil
import sys
import tempfile
import subprocess
import tomllib
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from stage_release import CONFIG, pack_maven, sha


def reuse_signed(unsigned, signed, fingerprint):
    """Verify each saved file and its signer before reusing an earlier bundle."""
    with zipfile.ZipFile(unsigned) as original, zipfile.ZipFile(signed) as saved:
        names = set(original.namelist())
        products = {name for name in names if not name.endswith((".md5", ".sha1", ".sha256", ".sha512"))}
        if set(saved.namelist()) != names | {name + ".asc" for name in products}:
            raise ValueError("Signed bundle has different members")
        for name in names:
            with original.open(name) as a, saved.open(name) as b:
                while True:
                    block = a.read(65536)
                    if block != b.read(65536):
                        raise ValueError(f"Signed file differs from the preview: {name}")
                    if not block:
                        break
        with tempfile.TemporaryDirectory() as stage:
            for name in products:
                file = Path(stage) / "file"
                signature = Path(stage) / "signature"
                for key, path in [(name, file), (name + ".asc", signature)]:
                    with saved.open(key) as src, path.open("wb") as dest:
                        shutil.copyfileobj(src, dest)
                status = subprocess.check_output(["gpg", "--batch", "--status-fd", "1", "--verify", signature, file], text=True)
                valid = [line.split() for line in status.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
                if not any(fingerprint.upper() in (row[2], row[-1]) for row in valid):
                    raise ValueError(f"Unexpected signer for {name}")


def sign(candidate, output, fingerprint, previous=None):
    config = tomllib.loads(CONFIG.read_text())
    record = json.loads((candidate / "native-ui.json").read_text())
    if output.exists():
        raise ValueError("Use a new directory for the signed candidate")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as temp:
        stage = Path(temp)
        for name, digest in record["sha256"].items():
            if sha(candidate / name) != digest:
                raise ValueError("Candidate digest mismatch for " + name)
        ready = stage / "signed"
        ready.mkdir()
        archive = config["apple_archive"]
        shutil.copyfile(candidate / archive, ready / archive)
        repo = stage / "maven"
        with zipfile.ZipFile(candidate / "android-publication.zip") as bundle:
            bundle.extractall(repo)
        if previous:
            reuse_signed(candidate / "android-publication.zip", previous, fingerprint)
            android = ready / "android-publication.zip"
            shutil.copyfile(previous, android)
        else:
            android = pack_maven(ready, repo, config, fingerprint)
        record["sha256"][android.name] = sha(android)
        record["central_signed"] = True
        (ready / "native-ui.json").write_text(json.dumps(record, sort_keys=True, indent=2) + "\n")
        ready.rename(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--key", required=True)
    parser.add_argument("--previous", type=Path)
    options = parser.parse_args()
    sign(options.candidate, options.out, options.key, options.previous)
