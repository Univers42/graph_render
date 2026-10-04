"""One HTTP handler for every browser gate: quiet logging and cross-origin isolation.

Plan P3's threads build (model (a) of docs/decisions/browser-threads.md) needs a SharedArrayBuffer,
which a browser only hands to a cross-origin isolated page: COOP `same-origin` and COEP
`require-corp` on every response, the wasm and the fixtures included. The five gates used to
each carry a copy of this handler; they import it from here.

`csp`, when given, is sent as `Content-Security-Policy` on every response, the worker script
included, which is how a worker gets its own policy: the embed gate serves the CSP a host is asked
for (docs/contract/host-api.md, verdict 13) and, as its negative control, one that refuses wasm.
"""
from http.server import SimpleHTTPRequestHandler


class QuietHandler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}

    def __init__(self, *args, isolated=True, csp=None, **kwargs):
        self.isolated = isolated
        self.csp = csp
        super().__init__(*args, **kwargs)

    def end_headers(self):
        # The two headers that isolate the page; skipped when isolated is False, which is how
        # the smoke gate's STUDIO_COI_BREAK negative control serves without them.
        if self.isolated:
            self.send_header("Cross-Origin-Opener-Policy", "same-origin")
            self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        if self.csp is not None:
            self.send_header("Content-Security-Policy", self.csp)
        super().end_headers()

    def log_message(self, format, *args):  # noqa: A002 - the base class names it
        pass
