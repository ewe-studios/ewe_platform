#!/usr/bin/env python3
"""Verify and stage one native release from the three source checkouts."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile
import tomllib
import xml.etree.ElementTree as ET
import zipfile

if __package__:
    from .swift_metadata import release_manifest
else:
    from swift_metadata import release_manifest

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / "release/native-ui.toml"


def sha(path, algorithm="sha256"):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, algorithm).hexdigest()


def revision(root):
    def git(*args):
        return subprocess.check_output(["git", "-C", root, *args], text=True).strip()
    if git("status", "--porcelain"):
        raise ValueError(f"Commit the release inputs in {root}")
    return git("rev-parse", "--verify", "HEAD^{commit}")


def verify_apple(path, config):
    with zipfile.ZipFile(path) as archive:
        info = plistlib.loads(archive.read("EweCoreFFI.xcframework/Info.plist"))
        found = set()
        for item in info["AvailableLibraries"]:
            platform = item.get("SupportedPlatformVariant", item["SupportedPlatform"])
            found.update((platform, arch) for arch in item["SupportedArchitectures"])
            base = "EweCoreFFI.xcframework/" + item["LibraryIdentifier"] + "/"
            if not archive.getinfo(base + item["LibraryPath"]).file_size:
                raise ValueError("Empty Apple library")
            for file in ("module.modulemap", "EweCoreFFI.h"):
                archive.getinfo(base + item["HeadersPath"] + "/" + file)
        if found != {("ios", "arm64"), ("simulator", "arm64"), ("simulator", "x86_64"), ("macos", "arm64"), ("macos", "x86_64")}:
            raise ValueError(f"Incorrect Apple library variants: {found}")
    if path.name != config["apple_archive"]:
        raise ValueError("Unexpected Apple artifact name")


def maven_inputs(repository, config):
    group, version = config["maven_group"], config["version"]
    modules = config["maven_modules"]
    files = []
    for module in modules:
        directory = repository / group.replace(".", "/") / module / version
        stem = f"{module}-{version}"
        pom = directory / (stem + ".pom")
        tree = ET.parse(pom)
        text = lambda key: tree.findtext("{*}" + key)
        if (text("groupId"), text("artifactId"), text("version")) != (group, module, version):
            raise ValueError(f"Unexpected coordinates in {pom}")
        for key in ("name", "description", "url", "licenses/{*}license/{*}name",
                    "developers/{*}developer/{*}name", "scm/{*}connection"):
            if not text(key):
                raise ValueError(f"Missing {key} in {pom}")
        kind = text("packaging") or "jar"
        if kind != ("jar" if module == "ewe-ui-core" else "aar"):
            raise ValueError(f"Incorrect package type in {pom}")
        primary = directory / (stem + "." + kind)
        if module == "ewe-ui-binding":
            with zipfile.ZipFile(primary) as aar:
                for abi in config["android_targets"]:
                    if not aar.getinfo(f"jni/{abi}/libfoundation_ui_native.so").file_size:
                        raise ValueError(f"Empty library for {abi}")
        if module == "ewe-ui":
            deps = {(d.findtext("{*}groupId"), d.findtext("{*}artifactId"), d.findtext("{*}version"))
                    for d in tree.findall("{*}dependencies/{*}dependency")}
            for dep in modules:
                if dep != module and (group, dep, version) not in deps:
                    raise ValueError(f"Missing runtime dependency {dep} in {pom}")
        for item in [pom, primary, directory / (stem + "-sources.jar"), directory / (stem + "-javadoc.jar")]:
            if not item.is_file() or not item.stat().st_size:
                raise ValueError(f"Missing publication input {item}")
            files.append(item)
        metadata = directory / (stem + ".module")
        if not metadata.is_file():
            raise ValueError(f"Missing Gradle module metadata: {metadata}")
        files.append(metadata)
    return sorted(files)


def pack_maven(stage, repository, config, signing_key):
    inputs = maven_inputs(repository, config)
    with tempfile.TemporaryDirectory(dir=stage) as temp:
        root = Path(temp)
        for source in inputs:
            target = root / source.relative_to(repository)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            if signing_key:
                subprocess.run(["gpg", "--batch", "--armor", "--detach-sign", "--local-user", signing_key,
                                "--output", str(target) + ".asc", target], check=True)
            for algorithm in ("md5", "sha1", "sha256", "sha512"):
                Path(str(target) + "." + algorithm).write_text(sha(target, algorithm))
        bundle = stage / "android-publication.zip"
        with zipfile.ZipFile(bundle, "w") as archive:
            for path in sorted(root.rglob("*")):
                if path.is_file():
                    info = zipfile.ZipInfo(path.relative_to(root).as_posix())
                    info.compress_type = zipfile.ZIP_DEFLATED
                    info.external_attr = 0o100644 << 16
                    with archive.open(info, "w") as entry, path.open("rb") as source:
                        shutil.copyfileobj(source, entry)
        return bundle


def wire_version(file, name):
    match = re.search(rf"pub const {name}: u(?:16|32) = (\d+);", (ROOT / file).read_text())
    if match is None:
        raise ValueError(f"Missing {name} in {file}")
    return int(match.group(1))


def stage_release(args):
    config = tomllib.loads(CONFIG.read_text())
    revisions = {key: revision(path) for key, path in
                 (("core", ROOT), ("swift_source", args.swift), ("kotlin", args.kotlin))}
    config_version = tomllib.loads((ROOT / "backends/foundation_ui_native/Cargo.toml").read_text())["package"]["version"]
    if config_version != config["version"]:
        raise ValueError("The native crate and release versions differ")
    verify_apple(args.apple, config)
    url = f"https://github.com/{config['repository']}/releases/download/native-ui-v{config['version']}/{args.apple.name}"
    for key, revision_key in (("swift_revision", "swift_source"), ("kotlin_revision", "kotlin")):
        if revisions[revision_key] != config[key]:
            raise ValueError(f"Source does not match {key}")
    swift_commit, swift_manifest = release_manifest(args.swift, revisions["swift_source"], url, sha(args.apple))
    args.out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=args.out) as temp:
        stage = Path(temp)
        apple = stage / args.apple.name
        shutil.copyfile(args.apple, apple)
        android = pack_maven(stage, args.maven, config, args.signing_key)
        mise = tomllib.loads((ROOT / "mise.toml").read_text())
        record = {
            "version": config["version"], "repositories": {k: v for k, v in config.items() if k.endswith("repository")},
            "revisions": revisions, "schema_version": wire_version("backends/foundation_ui_wire/src/lib.rs", "SCHEMA_VERSION"),
            "vocabulary_version": wire_version("backends/foundation_ui_wire/src/vocab/version.rs", "VOCABULARY_VERSION"),
            "swift_tag_revision": swift_commit, "swift_manifest": swift_manifest,
            "rust_toolchain": tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"],
            "platform_tools": {k: v for k, v in mise["env"].items() if k in ("ANDROID_NDK", "ANDROID_UI_COMPILE_SDK", "ANDROID_GRADLE_PLUGIN", "XCODE_MIN")},
            "tools": {k: v for k, v in mise["tools"].items() if k in ("java","gradle","kotlin") or "flatbuffers" in k},
            "swift_product": config["swift_product"],
            "maven_coordinates": [f"{config['maven_group']}:{m}:{config['version']}" for m in config["maven_modules"]],
            "central_signed": bool(args.signing_key), "sha256": {p.name: sha(p) for p in (apple, android)},
        }
        manifest = stage / "native-ui.json"
        manifest.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
        for item in (apple, android, manifest):
            existing = args.out / item.name
            if existing.exists() and sha(existing) != sha(item):
                raise ValueError(f"Different release input for {existing}")
        for item in (apple, android, manifest):
            item.replace(args.out / item.name)
    return record


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for opt in ("swift", "kotlin", "maven", "apple", "out"):
        parser.add_argument("--" + opt, type=Path, required=True)
    parser.add_argument("--signing-key", help="Fingerprint of the OpenPGP release key")
    try:
        print(json.dumps(stage_release(parser.parse_args()), indent=2))
    except (ValueError, FileNotFoundError, KeyError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from error
