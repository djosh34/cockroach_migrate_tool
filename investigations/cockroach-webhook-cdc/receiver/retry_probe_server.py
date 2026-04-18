import hashlib
import json
import os
import ssl
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


PORT = int(os.environ.get("PORT", "9443"))
CERT_FILE = os.environ["CERT_FILE"]
KEY_FILE = os.environ["KEY_FILE"]
LOG_PATH = Path(os.environ["LOG_PATH"])

LOG_PATH.parent.mkdir(parents=True, exist_ok=True)

_seen = {}


def append_log(entry: dict) -> None:
    with LOG_PATH.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(entry, sort_keys=True) + "\n")


class Handler(BaseHTTPRequestHandler):
    server_version = "retry-probe/1.0"

    def log_message(self, fmt: str, *args) -> None:
        print(fmt % args, flush=True)

    def do_GET(self) -> None:
        if self.path != "/healthz":
            self.send_response(404)
            self.end_headers()
            return

        body = b'{"ok":true}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self) -> None:
        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        body_hash = hashlib.sha256(body).hexdigest()
        attempt = _seen.get(body_hash, 0) + 1
        _seen[body_hash] = attempt

        entry = {
            "attempt": attempt,
            "body_hash": body_hash,
            "body_text": body.decode("utf-8", errors="replace"),
            "headers": dict(self.headers.items()),
            "path": self.path,
        }

        if self.path == "/fail-once" and attempt == 1:
            entry["response_code"] = 500
            append_log(entry)
            body = b'{"ok":false,"attempt":1}'
            self.send_response(500)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        entry["response_code"] = 200
        append_log(entry)
        body = json.dumps({"ok": True, "attempt": attempt}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def main() -> None:
    server = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(certfile=CERT_FILE, keyfile=KEY_FILE)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    print(json.dumps({"port": PORT, "log_path": str(LOG_PATH)}), flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
