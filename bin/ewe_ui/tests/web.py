"""Build an installed CLI's browser starter online and offline, then exercise its Rust counter."""
import argparse
import functools
import http.server
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", type=Path)
parser.add_argument("chromium", type=Path)
parser.add_argument("--archive", type=Path, help="Serve an unreleased runtime archive on loopback")
parser.add_argument("--workspace", type=Path, help="Use local Rust crates for an unreleased version")
options = parser.parse_args()
binary = options.binary.resolve()
chrome = options.chromium.resolve()


def serve(directory):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(
        http.server.SimpleHTTPRequestHandler, directory=directory
    ))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def counter(project):
    original = (project / "index.html").read_bytes()
    button = b'<script>setInterval(()=>{const b=document.querySelector("button");if(!b.disabled&&document.querySelector("output").value==="0") b.click();},25)</script>'
    (project / "test.html").write_bytes(original + button)
    server = serve(project)
    try:
        with tempfile.TemporaryDirectory() as profile:
            result = subprocess.run([chrome, "--headless", "--disable-gpu", "--disable-background-networking",
                                     "--user-data-dir=" + profile, "--virtual-time-budget=5000", "--dump-dom",
                                     f"http://127.0.0.1:{server.server_port}/test.html"],
                                    check=True, capture_output=True, text=True, timeout=60)
            assert "<output>1</output>" in result.stdout, result.stdout
    finally:
        server.shutdown()
        server.server_close()
        (project / "test.html").unlink()


with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    subprocess.run([binary, "new", "--name", "FieldNotes", "--target", "web"], cwd=root, check=True)
    project = root / "FieldNotes"
    web = project / "web"
    if options.workspace:
        (project / ".cargo").mkdir(exist_ok=True)
        (project / ".cargo/config.toml").write_text("[net]\noffline = true\n")
        meta = subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"],
                                       cwd=options.workspace)
        sources = {p["name"]: Path(p["manifest_path"]).parent for p in json.loads(meta)["packages"]
                   if p["name"].startswith("foundation_")}
        with (web / "Cargo.toml").open("a") as manifest:
            manifest.write("\n[patch.crates-io]\n")
            for name, source in sorted(sources.items()):
                manifest.write(name + " = { path = " + json.dumps(str(source)) + " }\n")
    registry = None
    if options.archive:
        archive = options.archive.resolve(strict=True)
        registry = serve(archive.parent)
        lock = web / "package-lock.json"
        packages = json.loads(lock.read_bytes())
        packages["packages"]["node_modules/@ewe-studios/foundation-wasm-ui"]["resolved"] = (
            f"http://127.0.0.1:{registry.server_port}/{archive.name}")
        lock.write_text(json.dumps(packages))
    env = dict(os.environ, npm_config_cache=str(root / "npm-cache"))
    env.pop("CARGO_NET_OFFLINE", None)
    env.pop("EWE_UI_OFFLINE", None)
    cmd = [binary, "build", "--target", "web"]
    try:
        subprocess.run(cmd, cwd=project, env=env, check=True)
    finally:
        if registry:
            registry.shutdown()
            registry.server_close()
    assets = web / "dist"
    expected = {name: (assets / name).read_bytes() for name in ["bundle.js", "module.wasm", "index.html", "app/title.txt"]}
    for name in expected:
        (assets / name).unlink()
    (web / "package-lock.json").touch()
    subprocess.run(cmd + ["--offline"], cwd=project, env=env, check=True)
    assert expected == {name: (assets / name).read_bytes() for name in expected}
    counter(assets)
    print("Both builds produced identical browser files. Chromium clicked the Rust counter.")
