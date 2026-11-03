#!/usr/bin/env python3
"""Serve the number of entries in /proc/net/tcp as a Prometheus gauge.

Standard library only.
"""
import os
import sys
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer

DEFAULT_PORT = 8080
INTERVAL = 10  # seconds
PATH = os.environ.get("PROC_NET_TCP", "/proc/net/tcp")

# A single int assignment is atomic under the GIL, so no lock is needed.
count = 0


def count_entries(path):
    """Count lines minus the header, reading in fixed-size chunks."""
    lines = 0
    with open(path, "rb", buffering=0) as f:
        while chunk := f.read(8192):
            lines += chunk.count(b"\n")
    return max(lines - 1, 0)


def sample():
    global count
    try:
        count = count_entries(PATH)
    except OSError as e:
        print(f"failed to read {PATH}: {e}", file=sys.stderr)


def sampler(stop):
    while not stop.wait(INTERVAL):
        sample()


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/metrics":
            self.send_error(404)
            return
        body = (
            "# HELP open_tcp_connections Number of entries in /proc/net/tcp.\n"
            "# TYPE open_tcp_connections gauge\n"
            f"open_tcp_connections {count}\n"
        ).encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; version=0.0.4")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass  # no per-request logging, matching the other implementations


def main():
    # Port: first CLI argument, then $PORT, then the default.
    raw = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("PORT")
    try:
        port = int(raw) if raw else DEFAULT_PORT
        if not 0 <= port <= 65535:
            raise ValueError
    except ValueError:
        print(f"invalid port: {raw}", file=sys.stderr)
        sys.exit(2)

    # Take one sample up front so the first scrape isn't a misleading 0.
    sample()
    threading.Thread(target=sampler, args=(threading.Event(),), daemon=True).start()

    # Single-threaded server: requests are handled one at a time.
    server = HTTPServer(("0.0.0.0", port), Handler)
    print(f"listening on 0.0.0.0:{port}, serving /metrics", file=sys.stderr)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
