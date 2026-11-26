#!/usr/bin/env python3
"""A local fake model server for the Cloudflare end-to-end tests.

It answers every POST with one fixed text in the OpenRouter shape. It needs no
model key and no network. This is the only local form of the model service.
"""

import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

TEXT = "hello from the fake model server"


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        request = json.loads(self.rfile.read(length) or b"{}")
        if request.get("stream"):
            # The OpenRouter server-sent event shape: one delta and [DONE].
            body = (
                "data: "
                + json.dumps({"choices": [{"delta": {"content": TEXT}}]})
                + "\n\ndata: [DONE]\n\n"
            ).encode("utf-8")
            content_type = "text/event-stream"
        else:
            body = json.dumps(
                {"choices": [{"message": {"content": TEXT}}]}
            ).encode("utf-8")
            content_type = "application/json"
        self.send_response(200)
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=8799)
    args = parser.parse_args()
    ThreadingHTTPServer(("127.0.0.1", args.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
