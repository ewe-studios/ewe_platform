"""Build an independent application and verify the binary address requested by Chromium."""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--chrome", default=os.environ.get("CHROME_BIN"))
parser.add_argument("--target-dir", type=Path)
parser.add_argument("--temp-dir", type=Path)
args = parser.parse_args()
chrome = args.chrome or shutil.which("chromium") or shutil.which("google-chrome")
if not chrome:
    parser.error("Supply --chrome or set CHROME_BIN")
crate = Path(__file__).resolve().parents[1]
requested = "assets/modules/application.wasm"


def run():
    with tempfile.TemporaryDirectory(dir=args.temp_dir) as tmp:
        project = Path(tmp)
        target = args.target_dir.resolve() if args.target_dir else project / "target"
        for name in ["src", "web"]:
            (project / name).mkdir()
        (project / "Cargo.toml").write_text(f"""
[workspace]
[package]
name = "module-location-fixture"
version = "0.1.0"
edition = "2021"
[lib]
crate-type = ["cdylib", "rlib"]
[features]
default = ["server"]
server = ["foundation_resources/browser-package"]
mobile = []
web = ["dep:wasm-bindgen"]
[build-dependencies]
foundation_resources = {{ path = {json.dumps(str(crate))}, default-features = false, features = ["build"] }}
[target.'cfg(target_family = "wasm")'.dependencies]
wasm-bindgen = {{ version = "0.2", optional = true }}
""")
        (project / "src/lib.rs").write_text("""
#[cfg(feature = "web")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn answer() -> u32 { 42 }
""")
        (project / "web/app.js").write_text("""
import {moduleUrl} from "./ewe-app-config.js";
async function boot() {
    const response = await fetch(moduleUrl);
    if (!response.ok) throw Error(String(response.status));
    await WebAssembly.compileStreaming(response);
    await fetch("./result/ready", {method: "POST"});
}
boot().catch(error => fetch("./result/failed", {method: "POST", body: String(error)}));
""")
        (project / "index.html").write_text('<!doctype html><body><script src="bundle.js"></script></body>')
        manifest = """[[resource]]
name='index.html'
path='index.html'
targets=['web']

[[resource]]
name='build-mode.txt'
generated='build-mode.txt'

[[resource]]
name='mobile.txt'
path='mobile.txt'
targets=['apple']
"""
        (project / "mobile.txt").write_text("native payload")
        (project / "resources.toml").write_text(manifest)
        shutil.copyfile(crate.parents[1] / "Cargo.lock", project / "Cargo.lock")
        public = target / "debug/public/module-location-fixture"

        def build(path, expect_success=True, mobile=False, destination=None):
            option = "" if path is None else f".with_module_path({json.dumps(path)})"
            (project / "build.rs").write_text(f"""
fn main() -> std::io::Result<()> {{
    foundation_resources::package::BrowserPackage::new(std::path::Path::new("web/app.js"))
        .browser_enabled(cfg!(feature="server"))
        .with_dev_preview(cfg!(feature="server"))
        .with_generated("build-mode.txt", if cfg!(feature="server") {{ "server" }} else {{ "mobile" }})
        {option}.build()
}}
""")
            command = ["cargo", "build", "--offline", "--target-dir", target, "--message-format=json"]
            if mobile:
                command.extend(["--no-default-features", "--features", "mobile"])
            env = os.environ.copy()
            for key in ["EWE_RESOURCE_TARGET", "EWE_RESOURCE_DEST", "EWE_RESOURCE_PAYLOAD"]:
                env.pop(key, None)
            if destination is not None:
                env.update(EWE_RESOURCE_TARGET="apple", EWE_RESOURCE_DEST=str(destination),
                           EWE_RESOURCE_PAYLOAD="EweResources")
            result = subprocess.run(command, cwd=project, capture_output=True, text=True, env=env)
            if result.returncode == 0 and not expect_success:
                raise AssertionError(f"Accepted conflicting or invalid module path {path}")
            if result.returncode and expect_success:
                raise RuntimeError(result.stderr)
            if result.returncode:
                if path not in result.stderr + result.stdout:
                    raise AssertionError(result.stderr + result.stdout)
                return
            messages = [json.loads(line) for line in result.stdout.splitlines()]
            outputs = [Path(event["out_dir"]) for event in messages
                       if event.get("reason") == "build-script-executed"
                       and "module-location-fixture" in event["package_id"]]
            assert len(outputs) == 1, outputs
            resources = outputs[0] / "resources" if destination is None else destination / "EweResources"
            assert (resources / "build-mode.txt").read_text() == ("mobile" if mobile else "server")
            if destination is not None:
                assert (resources / "mobile.txt").read_text() == "native payload"
                assert not (resources / "index.html").exists()
                assert not (resources / "bundle.js").exists()
            return resources

        def browser(path):
            assert (public / path).read_bytes()[:8] == b"\0asm\x01\0\0\0"
            requests = []
            finished = threading.Event()
            errors = []

            class Files(SimpleHTTPRequestHandler):
                def do_POST(self):
                    if self.path.endswith("/result/failed"):
                        length = int(self.headers.get("Content-Length", "0"))
                        errors.append(self.rfile.read(min(length, 2048)).decode())
                    elif not self.path.endswith("/result/ready"):
                        self.send_error(404)
                        return
                    self.send_response(204)
                    self.end_headers()
                    finished.set()

                def log_request(self, code="-", size="-"):
                    requests.append((self.path, int(code)))

            with ThreadingHTTPServer(("127.0.0.1", 0), partial(Files, directory=public.parent)) as server:
                thread = threading.Thread(target=server.serve_forever, daemon=True)
                thread.start()
                try:
                    with tempfile.TemporaryDirectory(dir=project) as profile:
                        command = [chrome, "--headless=new", "--disable-gpu", "--no-first-run",
                            "--disable-background-networking", "--user-data-dir=" + profile,
                            f"http://127.0.0.1:{server.server_port}/{public.name}/"]
                        with subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL) as process:
                            try:
                                assert finished.wait(120), requests
                                assert not errors, errors
                            finally:
                                process.terminate()
                                try:
                                    process.wait(timeout=10)
                                except subprocess.TimeoutExpired:
                                    process.kill()
                    assert (f"/{public.name}/{path}", 200) in requests, requests
                    assert [url for url, _ in requests if url.endswith(".js")] == [f"/{public.name}/bundle.js"], requests
                    print(f"Compiled /{path} from one application script", flush=True)
                finally:
                    server.shutdown()
                    thread.join()

        def published_files():
            return {path.relative_to(public): path.read_bytes()
                    for path in public.rglob("*") if path.is_file()}

        before_native = published_files()
        native = build(None, mobile=True)
        assert published_files() == before_native
        build(None)
        browser("module.wasm")
        assert (native / "build-mode.txt").read_text() == "mobile"
        assert (public / "build-mode.txt").read_text() == "server"

        build(requested)
        browser(requested)
        assert not (public / "module.wasm").exists()
        before = published_files()
        build(requested, mobile=True)
        assert published_files() == before
        build(requested, mobile=True, destination=project / "apple-resources")
        assert published_files() == before
        browser(requested)

        for invalid in ["../escape.wasm", "/absolute.wasm", "bundle.js", "bundle.min.js", "index.html"]:
            build(invalid, False)
            assert published_files() == before
        browser(requested)
        print("Preserved browser and native resources in both build orders and after failure.", flush=True)


if __name__ == "__main__":
    run()
