"""The two HTTP servers of the service-image gate (scripts/service-image.sh), both on 127.0.0.1.

The host: an origin of its own with three pages, everything else passed through to the service.
That pass-through is the supported embedding (docs/contract/host-api.md condition 13): the motor's
Worker must be same-origin with the page, so a host reverse-proxies the service's `/embed/`.
`/isolated/` and `/plain/` load the bundle that way, with and without COOP/COEP; `/direct/` loads
it from the service's own origin, the embedding that is documented to fail.

The negative control: the same pass-through in front of the service, minus the headers that
isolate the page and admit the bundle, so every later row sees a service that does not send them.
"""
import functools
import threading
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ISOLATION = {"Cross-Origin-Opener-Policy": "same-origin", "Cross-Origin-Embedder-Policy": "require-corp"}
# The least the docs ask of a host page's CSP (docs/deploy/service.md); the gate serves exactly it,
# so a bundle that needs more fails here first. The direct page's admits the service's origin too,
# so what refuses it is the browser's same-origin rule for a Worker, not this policy.
CSP = "script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'"
DIRECT_CSP = "script-src 'self' {service} 'wasm-unsafe-eval'; worker-src 'self' {service}"
STRIPPED = {"cross-origin-opener-policy", "cross-origin-embedder-policy", "cross-origin-resource-policy"}
# Hop-by-hop, or recomputed for the body as forwarded.
UNFORWARDED = {"connection", "keep-alive", "transfer-encoding", "content-length", "host"}

PAGE = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>service-image host</title>
<link rel="icon" href="data:,">
<style>html, body {{ margin: 0; height: 100%; }} graph-studio {{ width: 100vw; height: 100vh; }}</style>
</head><body>
<graph-studio wasm="{base}graph_wasm.wasm"></graph-studio>
<script type="module" src="{base}graph-studio.js"></script>
</body></html>
"""


def host_pages(version, service):
    """Path -> (html, isolated, csp) for the three pages the host serves."""
    proxied = PAGE.format(base=f"/embed/{version}/")
    direct = PAGE.format(base=f"{service}/embed/{version}/")
    return {"/isolated/": (proxied, True, CSP), "/plain/": (proxied, False, CSP),
            "/direct/": (direct, False, DIRECT_CSP.format(service=service))}


class PassThrough(BaseHTTPRequestHandler):
    """A page of its own where `pages` has one, else the upstream's reply, minus `strip`."""

    def __init__(self, *args, upstream, pages, strip, requested, **kwargs):
        self.upstream, self.pages, self.strip, self.requested = upstream, pages, strip, requested
        super().__init__(*args, **kwargs)

    def do_GET(self):  # noqa: N802 - the base class dispatches on this name
        if self.path in self.pages:
            self.send_page(*self.pages[self.path])
        else:
            self.forward()

    def send_page(self, html, isolated, csp):
        body = html.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Security-Policy", csp)
        for name, value in (ISOLATION.items() if isolated else ()):
            self.send_header(name, value)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def forward(self):
        self.requested.append(self.path)
        headers = {k: v for k, v in self.headers.items() if k.lower() not in UNFORWARDED}
        request = urllib.request.Request(self.upstream + self.path, headers=headers)
        try:
            reply = urllib.request.urlopen(request, timeout=30)
        except urllib.error.HTTPError as refused:
            reply = refused
        with reply:
            body, status, received = reply.read(), reply.status, reply.headers.items()
        self.send_response(status)
        for name, value in received:
            if name.lower() not in UNFORWARDED and not (self.strip and name.lower() in STRIPPED):
                self.send_header(name, value)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):  # noqa: A002 - the base class names it
        pass


def start(upstream, pages=None, strip=False):
    """A pass-through on a free port; returns the server and the list of paths it forwarded."""
    requested = []
    handler = functools.partial(PassThrough, upstream=upstream, pages=pages or {}, strip=strip,
                                requested=requested)
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, requested


def origin(server):
    return f"http://127.0.0.1:{server.server_address[1]}"
