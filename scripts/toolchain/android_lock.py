"""Add Android command-line tool checksums to the mise lock file."""

import hashlib
import tomllib
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path
from urllib.parse import urlsplit

REPOSITORY = "https://dl.google.com/android/repository/"
PLATFORMS = ("linux-x64", "macos-arm64", "macos-x64", "windows-x64")


def checksum(url, archive):
    """Check the download against Google's archive manifest and return its SHA-256."""
    expected_size = int(archive.findtext("size"))
    expected_sha1 = archive.find("checksum[@type='sha1']").text
    sha1 = hashlib.sha1()
    sha256 = hashlib.sha256()
    size = 0
    with urllib.request.urlopen(url, timeout=60) as response:
        while chunk := response.read(1024 * 1024):
            size += len(chunk)
            if size > expected_size:
                raise ValueError(f"Download exceeds the size in Google's manifest: {url}")
            sha1.update(chunk)
            sha256.update(chunk)
    if size != expected_size or sha1.hexdigest() != expected_sha1:
        raise ValueError(f"Download differs from Google's manifest: {url}")
    return f"sha256:{sha256.hexdigest()}"


def main():
    root = Path(__file__).resolve().parents[2]
    with (root / "mise.toml").open("rb") as config:
        pin = tomllib.load(config)["tools"]["http:android-sdk"]["version"]
    lock = root / "mise.lock"
    text = lock.read_text()
    entries = tomllib.loads(text)["tools"]["http:android-sdk"]
    entry, = (entry for entry in entries if entry["version"] == pin)

    missing = [
        (platform, entry[f"platforms.{platform}"]["url"])
        for platform in PLATFORMS
        if not entry[f"platforms.{platform}"].get("checksum")
    ]
    if not missing:
        return

    with urllib.request.urlopen(f"{REPOSITORY}repository2-3.xml", timeout=60) as response:
        archives = {
            archive.findtext("url"): archive
            for archive in ET.parse(response).iterfind(".//archives/archive/complete")
        }

    for platform, url in missing:
        filename = urlsplit(url).path.rsplit("/", 1)[-1]
        if url != f"{REPOSITORY}{filename}" or filename not in archives:
            raise ValueError(f"Google's manifest has no archive for {platform}: {url}")
        digest = checksum(url, archives[filename])
        header = f'[tools."http:android-sdk"."platforms.{platform}"]\n'
        section = f'{header}url = "{url}"'
        if text.count(section) != 1:
            raise ValueError(f"Cannot locate the mise lock section for Android on {platform}")
        text = text.replace(section, f'{header}checksum = "{digest}"\nurl = "{url}"')

    lock.write_text(text)


if __name__ == "__main__":
    main()
