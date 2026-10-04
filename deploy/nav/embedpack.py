"""The embed gate served over a built pack: the host page imports the pack, not the sources.

Usage: embed.py --dist PAGE_DIST --pack PACK --out DIR [--break]   (embed.py calls `run_pack`)

The three runs, the steps and the rows are `embed.py`'s, unchanged: `app/embed.html` is the same
host page, built with its one element import aliased to the pack's `graph-studio.js`
(app/vite.pack.config.ts, PACK_MODE=host), and served beside the pack's own files. So every row
that measures the studio is measuring the artifact a host would download. The host directory is
assembled here: the page's build, the pack's files at the top level (which is where the `wasm`
attribute and the worker's `new URL("graph_wasm_threads.wasm", wasmUrl)` look for them), and the
fixtures the host serves itself — the pack ships no fixture.

Two rows per run are added, `pack-wasm-<run>`: which wasm the motor worker fetched. `plain` and
`csp` must fetch `graph_wasm.wasm`, `isolated` must fetch `graph_wasm_threads.wasm`
(`packages/graph-studio/src/motor/worker.ts`: the threads artifact is loaded only when the page is
cross-origin isolated, and it is a sibling of the serial one, never a second request for it).

Where the fetches are read from: the requests the handler was given, recorded from the first byte
of every run. Not the CDP `Network` domain: the motor worker auto-attaches with
`waitForDebuggerOnStart: false` (`smokecdp.start_watching`), so `Network.enable` on its session only
lands at the next 200 ms poll of `embedpage.wait_loaded` — after a fetch that a local server
answers in a few ms. The page's own `Network.enable` is still called, and what it saw is reported
next to the served log, so a run that did fetch nothing on the page says so.
"""
import functools
import shutil
import tempfile
import threading
from http.server import ThreadingHTTPServer
from pathlib import Path

import embed
import embedpage
import nav
import smokecdp
import verdict
from drive import Studio
from serve import QuietHandler

# The fixtures are the host's own (the element's `fixtures` attribute), never the pack's.
FIXTURES = Path(__file__).resolve().parents[2] / "fixtures"
# What the worker must fetch in each run: the serial module unless the page is isolated.
EXPECTED_WASM = {"plain": "graph_wasm.wasm", "csp": "graph_wasm.wasm",
                 "isolated": "graph_wasm_threads.wasm"}


class Recording(QuietHandler):
    """The gate's handler, plus every path it was asked for: the log the wasm rows read."""

    def __init__(self, *args, seen, **kwargs):
        self.seen = seen
        super().__init__(*args, **kwargs)

    def do_GET(self):  # noqa: N802 - the base class names it
        self.seen.append(self.path.split("?")[0])
        super().do_GET()


def serve(served, isolated, csp, seen):
    """`nav.serve` over the recording handler, so the requests of one run are its own."""
    handler = functools.partial(Recording, directory=str(served), isolated=isolated, csp=csp, seen=seen)
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def assemble(page_dist, pack):
    """The host directory: the page's build, the pack's files beside it, the fixtures under it."""
    host = Path(tempfile.mkdtemp()) / "host"
    shutil.copytree(page_dist, host)
    for item in sorted(pack.iterdir()):
        if item.is_file():
            shutil.copy2(item, host / item.name)
    shutil.copytree(FIXTURES, host / "fixtures")
    return host


def wasm_names(paths):
    """The wasm files among `paths`, each once, in byte order."""
    return sorted({Path(path).name for path in paths if path.endswith(".wasm")})


def cdp_names(page):
    """What the page target's own Network domain saw, page and workers alike."""
    names = set()
    for event in page.events:
        params = event.get("params", {})
        if event.get("method") == "Network.requestWillBeSent":
            names |= wasm_names([params.get("request", {}).get("url", "")])
    return sorted(names)


def wasm_row(spec, seen, cdp):
    """`pack-wasm-<run>`: the worker fetched the module this run's isolation allows, and no other."""
    want = EXPECTED_WASM[spec.name]
    got = wasm_names(seen)
    measured = (f"the worker fetched {', '.join(got) or 'no wasm at all'}; "
                f"the page's Network domain saw {', '.join(cdp) or 'nothing'}; "
                f"{spec.name} expects {want}")
    return verdict.row(f"pack-wasm-{spec.name}", f"fetched {want} and nothing else", measured,
                       got == [want])


def pack_run(spec, args, host):
    """One run over the host directory: `embed.drive`'s rows, then this module's own wasm row."""
    served, scratch = embed.served_copy(host, spec.broken_wasm)
    seen = []
    server = serve(served, spec.isolated, spec.csp, seen)
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = smokecdp.Watcher(nav.DEBUG_PORT)
            page.start_watching()
            page.call("Network.enable")
            url = f"http://127.0.0.1:{server.server_address[1]}/embed.html"
            ctx = {"url": url, "out": args.out, "run": spec.name, "hand": Studio(page, url),
                   "point": None, "fixture": embed.fixture_counts(served),
                   "overlap_awaits": spec.overlap_awaits}
            rows = embed.drive(page, spec, ctx)
            if not args.broken and spec.name in EXPECTED_WASM:
                rows = [*rows, dict(wasm_row(spec, seen, cdp_names(page)), run=spec.name)]
            return rows, page.call("Browser.getVersion").get("product")
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
            if scratch is not None:
                shutil.rmtree(scratch, ignore_errors=True)


def run_pack(args):
    """Every run of `embed.py`'s gate or its negative control, over one assembled host directory."""
    host = assemble(args.dist, args.pack)
    specs = embed.BREAKS if args.broken else embed.GATE
    rows, browser = [], None
    try:
        for spec in specs:
            measured, browser = pack_run(spec, args, host)
            rows.extend(measured)
    finally:
        shutil.rmtree(host.parent, ignore_errors=True)
    return {"label": args.out.name, "commit": args.commit, "break": args.broken, "browser": browser,
            "pack": args.pack.name, "runs": [spec.name for spec in specs], "rows": rows}