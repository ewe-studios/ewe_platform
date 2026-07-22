"""Emulator display: ADB screencap to PNG, served via HTTP. Auto-refresh 2s."""
import http.server, subprocess, sys

HTML = """<!DOCTYPE html><html><head><meta charset="utf-8"><title>Android</title>
<meta http-equiv="refresh" content="2"><style>*{margin:0;padding:0}body{background:#000;display:flex;align-items:center;justify-content:center;min-height:100vh}img{max-width:100vw;max-height:100vh;object-fit:contain}</style></head><body><img src="/screen.png"></body></html>""".encode()

class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path in ("/","/vnc.html"):
            self.send_response(200); self.send_header("Content-type","text/html;charset=utf-8"); self.end_headers(); self.wfile.write(HTML)
        elif self.path.startswith("/screen.png"):
            r = subprocess.run(["adb","-e","exec-out","screencap","-p"],capture_output=True,timeout=5)
            self.send_response(200); self.send_header("Content-type","image/png"); self.send_header("Cache-Control","no-cache"); self.end_headers(); self.wfile.write(r.stdout)
        else: self.send_error(404)
http.server.HTTPServer(("0.0.0.0",int(sys.argv[1])if len(sys.argv)>1 else 6080),H).serve_forever()
