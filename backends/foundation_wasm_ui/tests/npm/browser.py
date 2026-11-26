"""Install one archive outside the checkout and exercise all three browser entries."""
import argparse
import functools
import http.server
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import threading

parser = argparse.ArgumentParser(__doc__)
parser.add_argument("archive", type=pathlib.Path)
parser.add_argument("--chrome", default=os.environ.get("CHROME") or shutil.which("google-chrome") or shutil.which("chromium") or "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
options = parser.parse_args()
archive = options.archive.resolve()
exercise = """
const mount = document.querySelector('output');
if (typeof createApp !== 'function' || typeof WasmLoader !== 'function') throw Error('Missing public export');
const runtime = {
  dispatcher: {setRawHandler() {}},
  callbacks: {invoke(id) {
    if (Number(id) !== 7) throw Error('Unexpected action');
    mount.textContent = 'Updated';
  }}
};
registerFrameApp(runtime, {document});
window.addEventListener('load', () => {
document.querySelector('button').click();
if (mount.textContent === 'Updated') document.body.dataset.verified = 'yes';
});
"""
class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        if self.path.endswith(".js"):
            self.server.scripts.append(self.path)
        super().do_GET()

with tempfile.TemporaryDirectory(prefix="ewe-npm-consumer-") as tmp:
    root = pathlib.Path(tmp)
    (root / "package.json").write_text(json.dumps({"private": True, "type": "module"}))
    subprocess.run(["npm", "install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", str(archive)], cwd=root, check=True)
    package = root / "node_modules/@ewe-studios/foundation-wasm-ui"
    assert (package / "bundle.min.js").stat().st_size < (package / "bundle.js").stat().st_size
    (root / "entry.js").write_text('import {WasmLoader, createApp, registerFrameApp} from "@ewe-studios/foundation-wasm-ui";\n' + exercise)
    subprocess.run(["npx", "--yes", "esbuild@0.28.2", "entry.js", "--bundle", "--outfile=consumer.js"], cwd=root, check=True)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler, directory=tmp))
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        for filename in ["consumer.js", "bundle.js", "bundle.min.js"]:
            follow = ""
            if filename != "consumer.js":
                shutil.copyfile(package / filename, root / filename)
                follow = "<script>const {createApp, registerFrameApp} = FoundationUiBootstrap; const {WasmLoader} = FoundationWasmRuntime;" + exercise + "</script>"
            (root / "index.html").write_text('<body><button primal:onclick="callback-7">Update</button><output>Initial</output><script src="' + filename + '"></script>' + follow + '</body>')
            server.scripts = []
            with tempfile.TemporaryDirectory(dir=root) as profile:
                run = subprocess.run([options.chrome, "--headless", "--disable-gpu", "--no-sandbox", "--disable-background-networking",
                                      "--user-data-dir=" + profile, "--dump-dom", "http://127.0.0.1:" + str(server.server_port)],
                                     capture_output=True, text=True, check=True, timeout=60)
            assert 'data-verified="yes"' in run.stdout, filename
            assert server.scripts == ["/" + filename], server.scripts
            print(filename, "dispatched callback 7 and rendered Updated")
    finally:
        server.shutdown()
        server.server_close()
        worker.join()
