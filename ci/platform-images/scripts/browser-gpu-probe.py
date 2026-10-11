#!/usr/bin/env python3
"""Check that a headless browser gets WebGL2 and a WebGPU adapter without a GPU.

    python3 browser-gpu-probe.py [--browser google-chrome] [--timeout 90]

Serves a probe page from http://127.0.0.1:<free port>. It has to be served:
`navigator.gpu` only exists in a secure context, which http://127.0.0.1 is
and data:/file: URLs are not. The page creates a WebGL2 context, asks
`navigator.gpu.requestAdapter()` for an adapter, and reports both back with a
request to /result. The browser runs headless with the flags that give it
WebGPU on Linux without a GPU (Dawn's SwiftShader Vulkan adapter, ANGLE on
Vulkan, which is lavapipe in the platform-images base image).

Prints one JSON line {webgl2, webgpu, adapter} and exits 0 only if WebGL2
works and an adapter was returned; 1 if not; 2 if the page never reported.
Standard library only.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

PAGE = b"""<!doctype html>
<meta charset="utf-8">
<title>platform-images gpu probe</title>
<script>
(async () => {
  const report = {webgl2: false, webgpu: "no-api", adapter: null, error: null};
  try {
    report.webgl2 = !!document.createElement("canvas").getContext("webgl2");
    if (navigator.gpu) {
      const adapter = await navigator.gpu.requestAdapter();
      if (adapter) {
        report.webgpu = "adapter";
        const info = adapter.info || {};
        report.adapter = {
          vendor: info.vendor || "", architecture: info.architecture || "",
          description: info.description || "", fallback: !!adapter.isFallbackAdapter,
        };
      } else {
        report.webgpu = "no-adapter";
      }
    }
  } catch (e) {
    report.error = String(e);
  }
  await fetch("/result?r=" + encodeURIComponent(JSON.stringify(report)));
})();
</script>
"""

# Headless Linux, no GPU: WebGPU behind --enable-unsafe-webgpu, Vulkan on,
# Dawn's SwiftShader adapter, ANGLE on Vulkan for WebGL, and SwiftShader
# allowed as WebGL's fallback.
BROWSER_FLAGS = [
    "--headless=new",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--no-default-browser-check",
    "--no-proxy-server",
    "--enable-unsafe-webgpu",
    "--enable-features=Vulkan",
    "--use-angle=vulkan",
    "--use-webgpu-adapter=swiftshader",
    "--enable-unsafe-swiftshader",
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--browser", default="google-chrome")
    parser.add_argument("--timeout", type=float, default=90.0, help="seconds to wait for the page's report")
    args = parser.parse_args()

    result: dict = {}
    reported = threading.Event()

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:  # noqa: N802 (http.server API)
            url = urlparse(self.path)
            if url.path == "/result":
                values = parse_qs(url.query).get("r", [])
                if values:
                    result.update(json.loads(values[0]))
                    reported.set()
                self.send_response(204)
                self.end_headers()
                return
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(PAGE)

        def log_message(self, fmt: str, *a: object) -> None:  # keep stdout for the result
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{server.server_address[1]}/"

    with tempfile.TemporaryDirectory(prefix="gpu-probe-") as profile:
        cmd = [args.browser, *BROWSER_FLAGS, f"--user-data-dir={profile}", url]
        try:
            browser = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        except OSError as e:
            server.shutdown()
            print(json.dumps({"error": f"cannot start {args.browser}: {e}"}))
            return 2
        try:
            got_report = reported.wait(args.timeout)
        finally:
            browser.terminate()
            try:
                _, stderr = browser.communicate(timeout=20)
            except subprocess.TimeoutExpired:
                browser.kill()
                _, stderr = browser.communicate()
            server.shutdown()

    if not got_report:
        print(json.dumps({"error": f"no report from the page within {args.timeout:.0f}s"}))
        tail = "\n".join(stderr.strip().splitlines()[-15:])
        print(f"browser stderr (last lines):\n{tail}", file=sys.stderr)
        return 2
    print(json.dumps(result))
    return 0 if result.get("webgl2") and result.get("webgpu") == "adapter" else 1


if __name__ == "__main__":
    sys.exit(main())
