#!/usr/bin/env python3
"""Check that a headless browser gets WebGL2 and a WebGPU adapter without a GPU.

    python3 browser-gpu-probe.py [--browser google-chrome] [--timeout 90]

Serves a probe page from http://127.0.0.1:<free port>. It has to be served:
`navigator.gpu` only exists in a secure context, which http://127.0.0.1 is
and data:/file: URLs are not. The page creates a WebGL2 context, asks
`navigator.gpu.requestAdapter()` for an adapter, and reports both back with a
request to /result.

The browser runs twice, with a flag set for each API, because the flags that
give WebGPU an adapter without a GPU are not the ones that give WebGL2 a
context (ANGLE on Vulkan has no WebGL2 context headless without a GPU):
  webgl   ANGLE on SwiftShader
  webgpu  WebGPU enabled, Vulkan on, Dawn's SwiftShader adapter
WebGL2 must work in the first pass and an adapter must come back in the
second. On a failure the report says why: the WebGL context-creation error,
whether the page was a secure context, the requestAdapter() error, and the
browser's own GPU feature status (chrome://gpu) for that flag set.

Prints one JSON line and exits 0 only if both pass; 1 if either fails; 2 if
the browser could not start or a page never reported. Standard library only.
"""

from __future__ import annotations

import argparse
import json
import re
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
  const r = {secure_context: window.isSecureContext, webgl2: false, webgl_renderer: null, webgl_error: null,
             webgl1: false, webgpu: "no-api", adapter: null, webgpu_error: null};
  try {
    const canvas = document.createElement("canvas");
    let creationError = null;
    canvas.addEventListener("webglcontextcreationerror", (e) => { creationError = e.statusMessage || "(no status message)"; });
    const gl = canvas.getContext("webgl2");
    r.webgl2 = !!gl;
    if (gl) {
      const dbg = gl.getExtension("WEBGL_debug_renderer_info");
      r.webgl_renderer = String(gl.getParameter(dbg ? dbg.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
    } else {
      r.webgl_error = creationError || "getContext('webgl2') returned null";
      r.webgl1 = !!document.createElement("canvas").getContext("webgl");
    }
  } catch (e) {
    r.webgl_error = "exception: " + e;
  }
  try {
    if (navigator.gpu) {
      const adapter = await navigator.gpu.requestAdapter();
      if (adapter) {
        r.webgpu = "adapter";
        const info = adapter.info || {};
        r.adapter = {vendor: info.vendor || "", architecture: info.architecture || "",
                     description: info.description || "", fallback: !!adapter.isFallbackAdapter};
      } else {
        r.webgpu = "no-adapter";
      }
    }
  } catch (e) {
    r.webgpu = "error";
    r.webgpu_error = String(e);
  }
  await fetch("result?r=" + encodeURIComponent(JSON.stringify(r)));  // relative: /<pass>/result
})();
</script>
"""

BASE_FLAGS = [
    "--headless=new",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--no-default-browser-check",
    "--no-proxy-server",
]
PASSES = {
    # WebGL2 through ANGLE on SwiftShader (what headless Chrome falls back
    # to without a GPU, made explicit and allowed).
    "webgl": ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
    # WebGPU through Dawn's SwiftShader Vulkan adapter.
    "webgpu": ["--enable-unsafe-webgpu", "--enable-features=Vulkan",
               "--use-webgpu-adapter=swiftshader", "--enable-unsafe-swiftshader"],
}


class Probe:
    """An HTTP server for the probe page that collects one report per pass."""

    def __init__(self) -> None:
        self.reports: dict[str, dict] = {}
        self.events: dict[str, threading.Event] = {name: threading.Event() for name in PASSES}
        probe = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self) -> None:  # noqa: N802 (http.server API)
                url = urlparse(self.path)
                name = url.path.strip("/").split("/")[0]
                if url.path.endswith("/result") and name in probe.events:
                    values = parse_qs(url.query).get("r", [])
                    if values:
                        probe.reports[name] = json.loads(values[0])
                        probe.events[name].set()
                    self.send_response(204)
                    self.end_headers()
                    return
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.end_headers()
                self.wfile.write(PAGE)

            def log_message(self, fmt: str, *a: object) -> None:  # keep stdout for the result
                pass

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def url(self, name: str) -> str:
        # Each pass gets its own path, so the page's relative /result goes to it.
        return f"http://127.0.0.1:{self.server.server_address[1]}/{name}/"


def run_browser(browser: str, flags: list[str], target: str, until: threading.Event | None,
                timeout: float) -> tuple[bool, str, str]:
    """Run the browser on target until `until` is set (or, without one, until it
    exits). Returns (finished, stdout, stderr)."""
    with tempfile.TemporaryDirectory(prefix="gpu-probe-") as profile:
        cmd = [browser, *BASE_FLAGS, *flags, f"--user-data-dir={profile}", target]
        proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if until is None:
            try:
                out, err = proc.communicate(timeout=timeout)
                return True, out, err
            except subprocess.TimeoutExpired:
                proc.kill()
                out, err = proc.communicate()
                return False, out, err
        finished = until.wait(timeout)
        proc.terminate()
        try:
            out, err = proc.communicate(timeout=20)
        except subprocess.TimeoutExpired:
            proc.kill()
            out, err = proc.communicate()
        return finished, out, err


def gpu_status(browser: str, flags: list[str]) -> list[str]:
    """The browser's GPU feature status lines (chrome://gpu) for a flag set."""
    ok, dom, _ = run_browser(browser, [*flags, "--virtual-time-budget=5000", "--dump-dom"], "chrome://gpu", None, 60)
    if not ok or not dom:
        return ["(chrome://gpu could not be read)"]
    text = re.sub(r"<[^>]+>", "\n", dom)
    lines = [ln.strip() for ln in text.splitlines() if ln.strip()]
    wanted = re.compile(r"webgl|webgpu|vulkan|angle|swiftshader|canvas|rasteri|disabled|hardware|software", re.I)
    picked = [ln for ln in lines if wanted.search(ln) and len(ln) < 160]
    return picked[:40] or lines[:40]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--browser", default="google-chrome")
    parser.add_argument("--timeout", type=float, default=90.0, help="seconds to wait for each pass's report")
    args = parser.parse_args()

    probe = Probe()
    result: dict = {}
    try:
        for name, flags in PASSES.items():
            try:
                done, _, stderr = run_browser(args.browser, flags, probe.url(name), probe.events[name], args.timeout)
            except OSError as e:
                print(json.dumps({"error": f"cannot start {args.browser}: {e}"}))
                return 2
            if not done:
                tail = "\n".join(stderr.strip().splitlines()[-15:])
                print(json.dumps({"error": f"{name} pass: no report from the page within {args.timeout:.0f}s"}))
                print(f"browser stderr (last lines):\n{tail}", file=sys.stderr)
                return 2
            result[name] = probe.reports[name]
    finally:
        probe.server.shutdown()

    webgl, webgpu = result["webgl"], result["webgpu"]
    summary = {
        "webgl2": webgl["webgl2"],
        "webgl_renderer": webgl.get("webgl_renderer"),
        "webgpu": webgpu["webgpu"],
        "adapter": webgpu.get("adapter"),
    }
    passed = bool(webgl["webgl2"]) and webgpu["webgpu"] == "adapter"
    if not passed:
        summary["details"] = result
        if not webgl["webgl2"]:
            summary["gpu_status_webgl_pass"] = gpu_status(args.browser, PASSES["webgl"])
        if webgpu["webgpu"] != "adapter":
            summary["gpu_status_webgpu_pass"] = gpu_status(args.browser, PASSES["webgpu"])
    print(json.dumps(summary))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
