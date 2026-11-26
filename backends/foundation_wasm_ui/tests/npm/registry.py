"""Exercise the release client with a disposable loopback registry and dummy credentials."""
import argparse
import base64
import hashlib
import http.server
import json
import os
import pathlib
import shutil
import sys
import subprocess
import tarfile
import tempfile
import threading

parser = argparse.ArgumentParser(__doc__)
parser.add_argument("binary", type=pathlib.Path)
parser.add_argument("archive", type=pathlib.Path)
options = parser.parse_args()
binary, archive = options.binary.resolve(), options.archive.resolve()
with tarfile.open(archive) as package:
    info = json.load(package.extractfile("package/package.json"))
name, version = info["name"], info["version"]
integrity = "sha512-" + base64.b64encode(hashlib.file_digest(archive.open("rb"), "sha512").digest()).decode()
env_value = "npm_dummy_environment_012"
file_value = "npm_dummy_file_034"

class Registry(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def response(self, code, payload):
        data = json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        # npm also checks package metadata inside the authenticated publish operation.
        if self.headers.get("Authorization"):
            assert self.headers.get("Authorization") == "Bearer " + self.server.credential
        if self.server.integrity is None:
            return self.response(404, {"error": "not found"})
        dist = {"integrity": self.server.integrity, "tarball": self.server.url + "/runtime.tgz"}
        self.response(200, {"name": name, "dist-tags": {"latest": version}, "versions": {version: {"name": name, "version": version, "dist": dist}}})

    def do_PUT(self):
        assert self.headers.get("Authorization") == "Bearer " + self.server.credential
        content = self.rfile.read(int(self.headers["Content-Length"]))
        assert all(v.encode() not in content for v in (env_value, file_value))
        payload = json.loads(content)
        attachment, = payload["_attachments"].values()
        assert base64.b64decode(attachment["data"]) == archive.read_bytes()
        self.server.integrity = integrity
        self.server.writes += 1
        self.response(201, {"ok": True})

server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Registry)
server.url = "http://127.0.0.1:" + str(server.server_port)
worker = threading.Thread(target=server.serve_forever, daemon=True)
worker.start()
try:
    with tempfile.TemporaryDirectory(prefix="ewe-release-") as tmp:
        root = pathlib.Path(tmp)
        npm = shutil.which("npm")
        if os.name == "posix":
            wrapper = root / "npm"
            wrapper.write_text("#!" + sys.executable + "\nimport os, sys\n"
                               + "assert not any(token in arg for token in " + repr((env_value, file_value)) + " for arg in sys.argv)\n"
                               + "if 'publish' not in sys.argv: assert 'NPM_TOKEN' not in os.environ\n"
                               + "os.execv(" + repr(npm) + ", [" + repr(npm) + "] + sys.argv[1:])\n")
            wrapper.chmod(0o755)
        marker = root / "executed"
        userenv = root / ".userenv"
        server.writes = 0
        server.integrity = None
        def run(value=None, ok=True):
            env = dict(os.environ, HOME=tmp, USERPROFILE=tmp, PATH=tmp + os.pathsep + os.environ["PATH"])
            env.pop("NPM_TOKEN", None)
            if value is not None:
                env["NPM_TOKEN"] = value
            command = [str(binary), "task", "run", "wasm-bundle:npm-publish", "--archive", str(archive),
                       "--integrity", integrity, "--tag", "foundation_wasm_ui-v" + version, "--commit", "a"*40,
                       "--registry", server.url]
            result = subprocess.run(command, env=env, cwd=root, capture_output=True, text=True, timeout=60)
            output = result.stdout + result.stderr
            assert all(v not in output and v not in command for v in (env_value, file_value))
            assert (result.returncode == 0) == ok, output
            return output

        assert "Missing NPM_TOKEN" in run(ok=False)
        userenv.write_text('UNUSED=$(touch "' + str(marker) + '")\nexport NPM_TOKEN="' + file_value + '"\n')
        server.credential = env_value
        run(env_value)
        assert server.writes == 1
        run(env_value)
        assert server.writes == 1
        server.integrity = "sha512-other"
        assert "different integrity" in run(env_value, ok=False)
        server.integrity = None
        server.credential = file_value
        run("")
        assert server.writes == 2
        userenv.write_text('NPM_TOKEN=$(touch "' + str(marker) + '")\n')
        assert "Missing NPM_TOKEN" in run(ok=False)
        assert not marker.exists()
        print("Environment precedence, file fallback, missing token, registry retry and conflict passed.")
finally:
    server.shutdown()
    server.server_close()
    worker.join()
