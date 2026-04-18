import json
import os
import ssl
import threading
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse


OUTPUT_DIR = Path(os.environ["OUTPUT_DIR"])
PORT = int(os.environ.get("PORT", "8443"))
CERT_FILE = os.environ["CERT_FILE"]
KEY_FILE = os.environ["KEY_FILE"]

OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

_COUNTER = 0
_LOCK = threading.Lock()


def next_sequence() -> int:
    global _COUNTER
    with _LOCK:
        _COUNTER += 1
        return _COUNTER


def sanitize_path(path: str) -> str:
    stripped = path.strip("/") or "root"
    return stripped.replace("/", "__")


class Handler(BaseHTTPRequestHandler):
    server_version = "cdc-receiver/1.0"

    def log_message(self, fmt: str, *args) -> None:
        print(
            f"{self.log_date_time_string()} {self.address_string()} "
            f"{fmt % args}",
            flush=True,
        )

    def do_GET(self) -> None:
        if self.path != "/healthz":
            self.send_response(404)
            self.end_headers()
            return

        body = json.dumps({"ok": True}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self) -> None:
        length = int(self.headers.get("Content-Length", "0"))
        body_bytes = self.rfile.read(length)
        body_text = body_bytes.decode("utf-8", errors="replace")

        try:
            body_json = json.loads(body_text)
        except json.JSONDecodeError:
            body_json = None

        parsed_path = urlparse(self.path)
        sequence = next_sequence()
        safe_path = sanitize_path(parsed_path.path)
        output_path = OUTPUT_DIR / f"{sequence:04d}-{safe_path}.json"

        entry = {
            "sequence": sequence,
            "received_at": datetime.now(timezone.utc).isoformat(),
            "method": self.command,
            "path": parsed_path.path,
            "query": parsed_path.query,
            "headers": dict(self.headers.items()),
            "body_text": body_text,
            "body_json": body_json,
        }

        if isinstance(body_json, dict):
            payload = body_json.get("payload")
            if isinstance(payload, list):
                entry["payload_length"] = len(payload)

        output_path.write_text(json.dumps(entry, indent=2, sort_keys=True) + "\n")
        print(
            json.dumps(
                {
                    "sequence": sequence,
                    "path": parsed_path.path,
                    "payload_length": entry.get("payload_length"),
                    "file": str(output_path),
                }
            ),
            flush=True,
        )

        response = json.dumps({"ok": True, "sequence": sequence}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)


def main() -> None:
    server = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(certfile=CERT_FILE, keyfile=KEY_FILE)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    print(
        json.dumps(
            {
                "listening_on": PORT,
                "output_dir": str(OUTPUT_DIR),
                "cert_file": CERT_FILE,
            }
        ),
        flush=True,
    )
    server.serve_forever()


if __name__ == "__main__":
    main()
