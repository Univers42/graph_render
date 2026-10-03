"""The 3D GL gate: the WebGL2 3D layer against the Canvas2D 3D painter, over app/dist.

Usage: gl.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Five page loads in one headless Chromium with WebGL2 on SwiftShader. Four are the studio's own
first graph laid out by `layout.basic3d.sphere` through the perf driver's `__perf.run` (the
console's `layout.run`) and turned by one real CDP drag: `?backend=canvas2d` and `?backend=webgl2`
for parity, the webgl2 page counting its GL calls through a second drag, a context taken away mid
session, and a browser with no WebGL2. The fifth is `?backend=auto` on a graph past the 3D layer's
threshold, which must stay on Canvas2D on this software rasteriser, named by deploy/nav/gpu.py. The page loads, faults and canvas
read-back are the backend gate's (deploy/nav/backend.py), imported rather than copied.

Caveat: the parity row counts pixels that differ by more than GAP and sees nothing else: not a
difference under GAP in every channel, not a node in the wrong place under another node, not a
layer that drew the right picture slowly (that is docs/measurements/perf-3d-gl.md). The uniform
row counts calls on the prototype, so a call made through a cached method reference would not
be counted; the layer makes none (`webgl2/sync3d.ts`).

Ponytail: the negative control is the backend gate's own fault, injected over CDP before the
page's scripts: the GL draw calls become no-ops, so the layer still reports `webgl2` and paints
nothing, and the parity and drawn rows go red on the pixels. Nothing in app/ or packages/ is
touched, so the control cannot mask a real regression.
"""
import argparse
import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav  # first: it puts the perf gate's CDP client on the path
import backend
import cdp
import glrows as judge
import gpu
import imagery
from drive import CENTRE, VIEWPORT

# The calls a drag may and may not make, counted on the prototype before the page's own scripts.
COUNT_GL = """
(() => {
  if (typeof WebGL2RenderingContext === 'undefined') return;
  window.__gmCalls = { bufferData: 0, texImage2D: 0, uniformMatrix4fv: 0, drawArraysInstanced: 0 };
  const proto = WebGL2RenderingContext.prototype;
  for (const name of Object.keys(window.__gmCalls)) {
    const native = proto[name];
    proto[name] = function (...rest) { window.__gmCalls[name] += 1; return native.apply(this, rest); };
  }
})()
"""
RESET_CALLS = "(() => { for (const name of Object.keys(window.__gmCalls)) window.__gmCalls[name] = 0; })()"
ORBIT = "document.querySelector('graph-studio').view.orbit()"
STORE_ERROR = "document.querySelector('graph-studio').studio.store.get().error"


def spaced(session, backend_name, inject=None):
    """A page at `?backend=`, the sphere laid out on its first graph, turned by one real drag."""
    studio = session.load(backend_name, inject=inject)
    session.page.evaluate(f"window.__perf.run({json.dumps(judge.LAYOUT)})")
    studio.settle_drawing()
    if session.page.evaluate(ORBIT) is None:
        raise cdp.CdpError(f"{judge.LAYOUT} drew no 3D frame: view.orbit() is null")
    studio.drag(CENTRE, *judge.DRAG)
    studio.settle()
    return studio


def shot(session, name, out):
    meta, pixels = backend.canvas(session.shot(name, out))
    return meta | {"stats": session.stats(), "orbit": session.page.evaluate(ORBIT)}, pixels


def parity(session, out, broken):
    """The same turned sphere drawn by each painter, and the share of pixels on which they differ."""
    spaced(session, "canvas2d")
    left, left_pixels = shot(session, "parity-canvas2d.png", out)
    # The `;` keeps two IIFEs two statements: without it the second is called on the first's result.
    faults = f"{backend.NO_DRAWS};{COUNT_GL}" if broken else COUNT_GL
    studio = spaced(session, "webgl2", inject=faults)
    right, right_pixels = shot(session, "parity-webgl2.png", out)
    over, total = imagery.differs(left_pixels, right_pixels, judge.GAP)
    return studio, {"gap": judge.GAP, "share": imagery.fraction(over, total), "pixels": total,
                    "canvas2d": left, "webgl2": right}


def drag_calls(session, studio):
    """The GL calls of one more drag on the page parity left, and that page's error channels."""
    session.page.evaluate(RESET_CALLS)
    studio.drag(CENTRE, *judge.DRAG)
    studio.settle()
    faults = judge.faults_of(session.page, session.page.evaluate(STORE_ERROR))
    return {"calls": session.page.evaluate("window.__gmCalls"), "faults": faults}


def context_lost(session, out):
    studio = spaced(session, "webgl2", inject=backend.KEEP_GL)
    before = session.stats()
    lost = session.page.evaluate(backend.LOSE_CONTEXT)
    studio.drag(CENTRE, *judge.DRAG)
    studio.settle()
    meta, _ = shot(session, "context-lost.png", out)
    return {"loseContext": lost, "before": before, "stats": meta.pop("stats"), "shot": meta}


def fallback(session, out):
    spaced(session, "webgl2", inject=backend.NO_WEBGL2)
    meta, _ = shot(session, "fallback.png", out)
    return {"stats": meta.pop("stats"), "shot": meta}


def auto_software(session):
    studio = session.load("auto", nodes=judge.AUTO_NODES, layout=judge.LAYOUT)
    orbit = session.page.evaluate(ORBIT)
    studio.drag(CENTRE, *judge.DRAG)
    studio.settle()
    return {"stats": session.stats(), "orbit": orbit, "renderer": session.page.evaluate(gpu.RENDERER_JS)}


def measure(dist, out, commit, broken):
    """The whole run in one function, so every exit path closes the browser and the server."""
    server = nav.serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=["--enable-unsafe-swiftshader"])
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/"
            session = backend.Session(nav.DEBUG_PORT, served)
            studio, parity_run = parity(session, out, broken)
            runs = {"parity": parity_run, "drag": drag_calls(session, studio)}
            runs |= {"context-lost": context_lost(session, out), "fallback": fallback(session, out),
                     "auto": auto_software(session)}
            rows = judge.run_rows(runs)
            return {"label": out.name, "commit": commit, "break": broken, "url": served,
                    "browser": session.page.call("Browser.getVersion").get("product"),
                    "viewport": list(VIEWPORT), "ceiling": judge.CEILING,
                    "blankFloor": judge.BLANK_FLOOR, "runs": runs, "rows": rows}
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def table(report):
    head = [f"# studio-3d-gl — {report['label']}", "",
            f"commit `{report['commit']}` · {report['browser']} · viewport "
            f"{report['viewport'][0]}x{report['viewport'][1]} · WebGL2 on SwiftShader · `{judge.LAYOUT}` · "
            f"parity ceiling {report['ceiling']:.1%} of pixels off by more than {judge.GAP}/255 · "
            f"blank floor {report['blankFloor']:.0%} · parity-canvas2d.png, parity-webgl2.png, "
            f"context-lost.png, fallback.png", "",
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
                        help="the negative control: the GL draw calls are no-ops, so the 3D layer "
                             "paints nothing and still reports webgl2")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit, args.broken)
    except (cdp.CdpError, imagery.PngError, OSError) as failure:
        print(f"studio-3d-gl: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
