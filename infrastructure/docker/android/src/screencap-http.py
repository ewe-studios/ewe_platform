"""Emulator visual access: ADB screencap to PNG, served via HTTP. Auto-refresh 2s."""
import http.server, subprocess, sys

HTML = """<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Android Emulator</title>
<meta http-equiv="refresh" content="2">
<style>*{margin:0;padding:0;box-sizing:border-box}
body{background:#000;display:flex;align-items:center;justify-content:center;min-height:100vh}
img{max-width:100vw;max-height:100vh;object-fit:contain}
.info{position:fixed;bottom:8px;right:12px;color:#555;font:12px monospace}
.tools{position:fixed;top:8px;left:8px;display:flex;gap:6px}
.tools button{background:#333;color:#ccc;border:1px solid #555;padding:4px 10px;border-radius:4px;cursor:pointer;font:11px monospace}
.tools button:hover{background:#555}
</style></head><body>
<img src="/screen.png" id="s"><div class="info">ADB screencap 2s</div></body></html>""".encode()

class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/" or self.path == "/vnc.html":
            self.send_response(200)
            self.send_header("Content-type","text/html;charset=utf-8")
            self.end_headers()
            self.wfile.write(HTML)
        elif self.path.startswith("/screen.png"):
            try:
                r = subprocess.run(["adb","-e","exec-out","screencap","-p"],capture_output=True,timeout=5)
                self.send_response(200)
                self.send_header("Content-type","image/png")
                self.send_header("Cache-Control","no-cache")
                self.end_headers()
                self.wfile.write(r.stdout)
            except: self.send_error(500)
        else: self.send_error(404)

http.server.HTTPServer(("0.0.0.0",int(sys.argv[1])if len(sys.argv)>1 else 6080),H).serve_forever()
