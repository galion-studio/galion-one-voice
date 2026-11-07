#!/usr/bin/env python3
"""Static demo server. Pretty paths map onto the .html files."""
import functools
import http.server
import sys
from urllib.parse import urlsplit, urlunsplit

PRETTY = {
    "/samantha": "/samantha.html",
    "/jarvis": "/jarvis.html",
    "/spectrum": "/spectrum.html",
}


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
    }

    def _map_path(self, path):
        if path == "/nebula":
            rest = "/"
            prefixed = True
        elif path.startswith("/nebula/"):
            rest = path[len("/nebula"):]
            prefixed = True
        else:
            rest = path
            prefixed = False
        if rest != "/" and rest.endswith("/"):
            trimmed = rest.rstrip("/") or "/"
            if trimmed in PRETTY:
                dest = ("/nebula" + trimmed) if prefixed else trimmed
                return ("redirect", dest)
        origin = PRETTY.get(rest, rest)
        return ("ok", origin)

    def _prepare(self):
        parts = urlsplit(self.path)
        kind, value = self._map_path(parts.path)
        if kind == "redirect":
            return "redirect", urlunsplit(parts._replace(path=value))
        self.path = urlunsplit(parts._replace(path=value))
        return "ok", None

    def do_GET(self):
        kind, loc = self._prepare()
        if kind == "redirect":
            self.send_response(301)
            self.send_header("Location", loc)
            self.end_headers()
            return
        return super().do_GET()

    def do_HEAD(self):
        kind, loc = self._prepare()
        if kind == "redirect":
            self.send_response(301)
            self.send_header("Location", loc)
            self.end_headers()
            return
        return super().do_HEAD()


if __name__ == "__main__":
    d = sys.argv[1]
    p = int(sys.argv[2])
    http.server.ThreadingHTTPServer(
        ("127.0.0.1", p),
        functools.partial(Handler, directory=d),
    ).serve_forever()
