"""Studio backend gate: does the WebGL2 layer draw what Canvas2D draws, and is its giving up honest.

Usage: backend.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

The layer under test is `packages/graph-render/src/webgl2/`, which P5 added as a second painter
for one graph. Nothing else in the tree compares the two painters, so this gate is where a layer
that draws the wrong thing, or nothing, is caught. Four page loads of one seeded synthetic graph
and one camera: parity, `auto`, the fallback, and a context lost in the middle of a session.

Every load is a real page at `?backend=NAME` in one headless Chromium, with WebGL2 on
SwiftShader — the only WebGL2 a GPU-less container has — so `nav.launch_browser` takes the flag
as an argument rather than this file copying the launcher. The graph comes from the perf gate's
own driver (`deploy/perf/drivers/hook.js`), so there is one generator and one seeded source.

Caveat: the parity row counts the pixels on which the two painters differ by more than GAP, and
it cannot see anything else. Not a difference under GAP in every channel; not a difference of the
same size somewhere else; not a mark in the wrong place under another mark; not a layer that drew
the right picture slowly, which is the perf gate's row (deploy/perf). Nor can it see what the two
painters agree on wrongly, because "the same picture" is the claim neither is asked to make: GL
has no line width, no arrows and no impostor spheres (webgl2/layer.ts), so a tight ceiling would
be a ceiling on a defect, not on a difference.

Ponytail: the negative control is one fault, in the browser, injected over CDP before the page's
own scripts: the four draw calls a GL frame is made of become no-ops, so the layer still reports
`webgl2` and paints nothing. Nothing in app/ or packages/ is touched, so the control cannot mask
a real regression, and the parity row goes red on the pixels rather than on a flag.
"""
import argparse
import base64
import json
import sys
import tempfile
from pathlib import Path

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import imagery
import smokecdp
import backendrows as judge
from drive import CENTRE, VIEWPORT, Studio

HERE = Path(__file__).resolve().parent
# One driver for the whole tree: the perf gate's own `window.__perf.open(nodes, layout)`.
HOOK = (HERE.parent / "perf" / "drivers" / "hook.js").read_text()
# A dragged camera, so a frame is owed after the context is lost: a loss is noticed by a paint,
# and a page that is still paints nothing.
DRAG = (60, 40)

# The fallback row's fault, before any studio code: no WebGL2 context on an OffscreenCanvas. Every
# other kind is left alone, "2d" included — createBulk() reads a 2D context of its own to
# normalise colours (webgl2/layer.ts) and must still get one.
NO_WEBGL2 = """
(() => {
  const native = OffscreenCanvas.prototype.getContext;
  OffscreenCanvas.prototype.getContext = function (kind, ...rest) {
    return kind === "webgl2" ? null : native.call(this, kind, ...rest);
  };
})()
"""

# The context-lost row's fault: the same wrapper, but it keeps what the layer was given, so the
# gate can take that context away afterwards. A list, because the layer may ask for more than one.
KEEP_GL = """
(() => {
  window.__gmContexts = [];
  const native = OffscreenCanvas.prototype.getContext;
  OffscreenCanvas.prototype.getContext = function (kind, ...rest) {
    const context = native.call(this, kind, ...rest);
    if (kind === "webgl2" && context !== null) window.__gmContexts.push(context);
    return context;
  };
})()
"""

# What the gate calls on the context it was given: the extension every WebGL2 context has for
# exactly this, and the reason a page finds out that its context is gone.
LOSE_CONTEXT = """
(() => {
  const held = window.__gmContexts ?? [];
  if (held.length === 0) return 'the page never made a WebGL2 context';
  const extension = held[held.length - 1].getExtension('WEBGL_lose_context');
  if (extension === null) return 'this context has no WEBGL_lose_context';
  extension.loseContext();
  return `took the last of ${held.length} context(s) away`;
})()
"""

# The negative control: a layer that draws nothing and says nothing. These four calls are the only
# draws webgl2/draw.ts makes, and no-ops leave the OffscreenCanvas cleared, which the 2D canvas
# then blits over its ground: an empty frame that still reads as a healthy `backend`.
NO_DRAWS = """
(() => {
  if (typeof WebGL2RenderingContext === 'undefined') return;
  const nothing = () => {};
  for (const name of ['drawArrays', 'drawElements', 'drawArraysInstanced', 'drawElementsInstanced']) {
    WebGL2RenderingContext.prototype[name] = nothing;
  }
})()
"""

# The view's own counters, through the element the host page defines.
STATS = "document.querySelector('graph-studio').view.stats()"
# The canvas' own bitmap, as a data URL: no dock, no HUD, no legend drawn over the graph.
CANVAS_PNG = "document.querySelector('graph-studio').shadowRoot.querySelector('canvas').toDataURL('image/png')"


class Session:
    """One page target, and the one new-document script installed on it at a time."""

    def __init__(self, port, served):
        self.page = smokecdp.Watcher(port)
        self.page.start_watching()
        self.served = served
        self.script = None

    def install(self, source):
        """`source` runs before the page's own scripts on the next load; None removes the last one."""
        if self.script is not None:
            self.page.call("Page.removeScriptToEvaluateOnNewDocument", {"identifier": self.script})
            self.script = None
        if source is not None:
            added = self.page.call("Page.addScriptToEvaluateOnNewDocument", {"source": source})
            self.script = added["identifier"]

    def load(self, backend, inject=None, nodes=None, layout=None):
        """A settled page at `?backend=`, with the graph open and the perf driver in it."""
        self.install(inject)
        self.page.events.clear()
        studio = Studio(self.page, f"{self.served}?backend={backend}")
        studio.open()
        self.page.evaluate(HOOK)
        if nodes is not None:
            self.page.evaluate(f"window.__perf.open({nodes}, {json.dumps(layout)})")
            studio.settle_drawing()
        return studio

    def stats(self):
        return self.page.evaluate(STATS)

    def shot(self, name, out):
        """The canvas' own bitmap, as the page would export it.

        The canvas is the whole viewport and the dock, the legend and the HUD float over it, so a
        viewport screenshot is the studio's chrome as much as its graph — and a canvas that drew
        nothing would still be full of chrome. `toDataURL` is the canvas and nothing else, which is
        what both the parity and the blank-canvas rows are about.
        """
        data_url = self.page.evaluate(CANVAS_PNG)
        path = out / name
        path.write_bytes(base64.b64decode(data_url.split(",", 1)[1]))
        return path


def canvas(path):
    """`(meta, pixels)`: what the table prints about one screenshot, and its pixels.

    `offBackground` is the share of the pixels that are not the canvas' own background colour
    (imagery.py carries the caveat): the claim a blank canvas fails and a drawing passes.
    """
    width, height, pixels = imagery.read(path)
    ground = pixels[:3] * (width * height)
    return {"file": path.name, "size": [width, height],
            "offBackground": imagery.fraction(*imagery.differs(pixels, ground, 0))}, pixels


def parity(session, out, broken):
    """The same graph drawn by each painter, and the share of pixels on which they differ."""
    shots = {}
    for backend in ("canvas2d", "webgl2"):
        studio = session.load(backend, inject=NO_DRAWS if broken else None,
                              nodes=judge.PARITY_NODES, layout=judge.PARITY_LAYOUT)
        shots[backend] = {"path": session.shot(f"parity-{backend}.png", out),
                          "stats": session.stats(), "camera": studio.camera()}
    left, left_pixels = canvas(shots["canvas2d"]["path"])
    right, right_pixels = canvas(shots["webgl2"]["path"])
    over, total = imagery.differs(left_pixels, right_pixels, judge.GAP)
    return {"nodes": judge.PARITY_NODES, "layout": judge.PARITY_LAYOUT, "gap": judge.GAP,
            "share": imagery.fraction(over, total), "pixels": total,
            "cameras": [shots["canvas2d"]["camera"], shots["webgl2"]["camera"]],
            "canvas2d": left | {"stats": shots["canvas2d"]["stats"]},
            "webgl2": right | {"stats": shots["webgl2"]["stats"]}}


def auto_large(session):
    """A graph well over BULK_THRESHOLD at `?backend=auto`: the layer must be the one drawing."""
    session.load("auto", nodes=judge.LARGE_NODES, layout=judge.LARGE_LAYOUT)
    return {"nodes": judge.LARGE_NODES, "layout": judge.LARGE_LAYOUT, "stats": session.stats()}


def fallback(session, out):
    """`?backend=webgl2` on a browser with no WebGL2, and that page's own error channels."""
    session.load("webgl2", inject=NO_WEBGL2, nodes=judge.PARITY_NODES, layout=judge.PARITY_LAYOUT)
    meta, _ = canvas(session.shot("fallback.png", out))
    return {"stats": session.stats(), "shot": meta, "page": session.page}


def context_lost(session, out):
    """The layer's own context taken away in the session, then the camera moved one drag."""
    studio = session.load("webgl2", inject=KEEP_GL, nodes=judge.PARITY_NODES,
                          layout=judge.PARITY_LAYOUT)
    before = session.stats()
    lost = session.page.evaluate(LOSE_CONTEXT)
    studio.drag(CENTRE, *DRAG)
    studio.settle()
    meta, _ = canvas(session.shot("context-lost.png", out))
    return {"nodes": judge.PARITY_NODES, "layout": judge.PARITY_LAYOUT, "loseContext": lost,
            "before": before, "stats": session.stats(), "shot": meta, "camera": studio.camera()}


def measure(dist, out, commit, broken):
    """The whole run in one function, so every exit path closes the browser and the server."""
    server = nav.serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=["--enable-unsafe-swiftshader"])
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/"
            session = Session(nav.DEBUG_PORT, served)
            runs = {"parity": parity(session, out, broken), "auto": auto_large(session),
                    "fallback": fallback(session, out), "context-lost": context_lost(session, out)}
            rows = judge.run_rows(runs)
            # The fallback's page is not evidence and cannot be written: its row has read it.
            runs["fallback"].pop("page")
            return {"label": out.name, "commit": commit, "break": broken, "url": served,
                    "browser": session.page.call("Browser.getVersion").get("product"),
                    "viewport": list(VIEWPORT), "ceiling": judge.CEILING,
                    "blankFloor": judge.BLANK_FLOOR, "runs": runs, "rows": rows}
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def table(report):
    """The nav table with this gate's name, so one renderer serves all the browser gates."""
    parity_run = report["runs"]["parity"]
    head = [f"# studio-backend — {report['label']}", "",
            f"commit `{report['commit']}` · {report['browser']} · viewport "
            f"{report['viewport'][0]}x{report['viewport'][1]} · WebGL2 on SwiftShader · parity "
            f"ceiling {report['ceiling']:.0%} of pixels off by more than {parity_run['gap']}/255 · "
            f"blank floor {report['blankFloor']:.0%} · parity-canvas2d.png, parity-webgl2.png, "
            f"fallback.png, context-lost.png", "",
            "| row | expectation | measured | verdict |", "|---|---|---|---|"]
    body = [f"| `{r['row']}` | {r['expectation']} | {r['measured']} | {r['verdict']} |" for r in report["rows"]]
    notes = [f"`{r['row']}` not run: {r['why']}" for r in report["rows"] if r["verdict"] == "NOT-RUN"]
    return "\n".join([*head, *body, "", *(notes or ["no row was left unrun"]), ""])


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--commit", default="unknown")
    parser.add_argument("--break", action="store_true", dest="broken",
                        help="the negative control: the GL layer's four draw calls are no-ops, so "
                             "it paints nothing and still reports webgl2")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit, args.broken)
    except (cdp.CdpError, imagery.PngError, OSError) as failure:
        print(f"studio-backend: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())