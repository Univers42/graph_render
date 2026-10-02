"""Studio perf gate: serve a built studio, drive it in headless Chromium, judge the rows.

Usage: run.py --dist DIR --out DIR --driver NAME [--edge-colour flat|gradient] [--baseline FILE]
              [--record-baseline FILE] [--commit ID] [--cases N,N,...] [--layout ID] [--backend NAME]
Exit:  0 every gating row PASS · 1 a gating row FAIL or NOT-RUN · 2 the harness could not run

--cases replaces the frame cases with these node counts at DPR 1 and skips the block and idle
runs: a scale measurement, not a gate, so its gating rows read NOT-RUN and it exits 1.
--layout lays every --cases graph out with ID instead (a cheap one keeps a 1M-node case inside
the 180 s open timeout). --backend opens the page at ?backend=NAME and, for any choice but
canvas2d, lets Chromium draw WebGL2 on SwiftShader; each case records the backend that drew.

GM_GPU=1 measures the same cases on the host's GPU instead (deploy/nav/gpu.py): the flags, the
device and the check are one knob, the report names the renderer it drew on, and a browser that
fell back to software exits 2 rather than reporting a CPU rasteriser as a GPU.

Ponytail: software raster in a container on a shared host. Numbers compare run to run on
one machine; they are not the frame rate a user's browser reaches (read the studio HUD).
"""

import argparse
import functools
import json
import subprocess
import sys
import tempfile
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import cdp
import rows as judge

HERE = Path(__file__).resolve().parent
# The GL backend knob and the renderer check live with the browser launcher (deploy/nav/gpu.py).
sys.path.insert(0, str(HERE.parent / "nav"))

import gpu
# A full-HD window: at DPR 2 the graph canvas is about 3000x2000, the size a desktop user has.
VIEWPORT = (1920, 1080)
DEBUG_PORT = 9222
# The studio's own default, Barnes-Hut (`docs/reports/perf-studio-fa2bh.md`): O(n log n), so
# the cases under LARGE_FROM are laid out by it rather than by the exact dense sum.
FORCE_LAYOUT = "layout.forceatlas2.barnes_hut"
# Measured in the motor alone (2026-09-29): the exact forceatlas2 takes 66 s at 20 000 nodes,
# pivot MDS 1.3 s. The frame rows time the drawing, not the layout, so the large case is laid
# out by the one that finishes; each case records which.
LARGE_LAYOUT = "layout.mds.pivot"
LARGE_FROM = 5000
FRAME_CASES = [(120, 1), (120, 2), (2000, 1), (2000, 2), (10000, 1), (10000, 2)]
# The counter rows read the view at these sizes, at DPR 1 (a counter does not depend on DPR).
STATS_CASES = [(2000, 1), (10000, 1)]
BLOCK_NODES = [120, 500]
PROFILED_CASE = (2000, 2)
# The backend the view drew the case with, and why WebGL2 was refused if it was.
DREW_WITH = "(({backend, backendFailure}) => ({backend, backendFailure}))(window.__perf.view().stats())"


class QuietHandler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}

    def log_message(self, format, *args):  # noqa: A002 - the base class names it
        pass


def serve(dist):
    handler = functools.partial(QuietHandler, directory=str(dist))
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def launch_browser(profile, backend):
    # --no-sandbox: the container has no user namespace to build the sandbox from, and the
    # only page ever loaded is this repository's own build, served from 127.0.0.1. The same
    # reason holds for --enable-unsafe-swiftshader, the only WebGL2 a GPU-less container has.
    # The gate itself runs without it, so its rows keep measuring the Canvas2D painter. Under
    # GM_GPU=1 that flag is replaced by the host's GL backend and the device behind it
    # (deploy/nav/gpu.py), which makes this a measurement of another arm, not a gating row.
    webgl = gpu.chrome_flags(draws_webgl=backend not in (None, "canvas2d"))
    return subprocess.Popen([
        "chromium", "--headless=new", "--no-sandbox", *webgl,
        "--disable-dev-shm-usage", f"--remote-debugging-port={DEBUG_PORT}",
        f"--user-data-dir={profile}", f"--window-size={VIEWPORT[0]},{VIEWPORT[1]}",
        "about:blank",
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


class Studio:
    """The page, plus the driver that knows how to operate this studio."""

    def __init__(self, page, url, driver, edge_colour="flat"):
        self.page = page
        self.url = url
        self.driver = (HERE / "drivers" / f"{driver}.js").read_text()
        self.edge_colour = edge_colour
        self.open_ms = None
        # Without Page.enable the script is registered (an identifier comes back) and never run.
        page.call("Page.enable")
        page.call("Page.addScriptToEvaluateOnNewDocument", {"source": (HERE / "react-hook.js").read_text()})

    def open(self, nodes, dpr, layout=FORCE_LAYOUT):
        self.page.set_viewport(VIEWPORT[0], VIEWPORT[1], dpr)
        self.page.navigate("about:blank")
        self.page.navigate(self.url)
        self.page.evaluate(self.driver)
        limit = self.page.evaluate("window.__perf.maxNodes")
        if nodes > limit:
            return f"driver caps at {limit} nodes"
        started = time.monotonic()
        self.page.evaluate(f"window.__perf.open({nodes}, {json.dumps(layout)})")
        self.open_ms = round(1000 * (time.monotonic() - started))
        # The edge colour mode is a display setting, so it is asked for once the graph is
        # drawn: a driver with no hook for it is measured in whatever mode it opened in.
        if self.page.evaluate("typeof window.__perf.edgeColour === 'function'"):
            self.page.evaluate(f"window.__perf.edgeColour({json.dumps(self.edge_colour)})")
        return None

    def probe(self, name, args):
        source = (HERE / "probes" / f"{name}.js").read_text()
        return self.page.evaluate(f"({source})({json.dumps(args)})")


def measure_frames(studio, out, frame_cases, layout_override=None):
    cases = []
    for at, (nodes, dpr) in enumerate(frame_cases):
        layout = layout_override or (LARGE_LAYOUT if nodes >= LARGE_FROM else FORCE_LAYOUT)
        try:
            skipped = studio.open(nodes, dpr, layout)
        except TimeoutError:
            # The page is still busy with this graph and its CDP reply is pending, so no later
            # case can be measured on it: the cases already measured are kept.
            cases.append({"nodes": nodes, "dpr": dpr, "notRun": "did not open within 180 s (cdp.evaluate)"})
            cases.extend({"nodes": n, "dpr": d, "notRun": "an earlier case timed out"} for n, d in frame_cases[at + 1:])
            break
        if skipped is not None:
            cases.append({"nodes": nodes, "dpr": dpr, "notRun": skipped})
            continue
        # From navigation to the graph drawn and its edge colour set: what loading it cost React.
        case = {"nodes": nodes, "dpr": dpr, "layout": layout, "openMs": studio.open_ms,
                "reactAtOpen": studio.page.evaluate("window.__reactCommits ?? null"),
                "drewWith": studio.page.evaluate(DREW_WITH)}
        if (nodes, dpr) in STATS_CASES:
            case["stats"] = studio.probe("stats", {"settleMs": 1500})
        case.update(studio.probe("frame", {"settleMs": 1500, "profile": False}))
        if (nodes, dpr) == PROFILED_CASE:
            case["profile"] = studio.probe("frame", {"settleMs": 200, "profile": True})["phases"]
        studio.page.screenshot(str(out / f"frame-{nodes}-dpr{dpr}.png"))
        cases.append(case)
    return cases


def measure_block(studio):
    cases = []
    for nodes in BLOCK_NODES:
        skipped = studio.open(nodes, 1)
        if skipped is not None:
            cases.append({"nodes": nodes, "notRun": skipped})
            continue
        cases.append({"nodes": nodes, "layouts": studio.probe("block", {})})
    return cases


def measure_idle(studio):
    skipped = studio.open(120, 1)
    if skipped is not None:
        return {"notRun": skipped}
    return studio.probe("idle", {"settleMs": 1500, "seconds": 4})


def measure(args, out):
    server = serve(args.dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = launch_browser(profile, args.backend)
        try:
            page = cdp.Page(DEBUG_PORT)
            query = f"?backend={args.backend}" if args.backend else ""
            url = f"http://127.0.0.1:{server.server_address[1]}/{query}"
            # What the browser's WebGL2 backend is, before a case is opened: under GM_GPU=1 a
            # software rasteriser here is a refusal to measure, not a slower number. Read on a
            # loaded document — about:blank hands out no context and names no backend.
            page.navigate(url)
            name = gpu.check(page)
            studio = Studio(page, url, args.driver, args.edge_colour)
            version = page.call("Browser.getVersion").get("product")
            report = {
                "label": out.name, "driver": args.driver, "commit": args.commit,
                "edgeColour": args.edge_colour, "backend": args.backend,
                "browser": version, "viewport": VIEWPORT, "renderer": name,
            }
            if args.cases:
                report["frames"] = measure_frames(studio, out, [(nodes, 1) for nodes in args.cases], args.layout)
                return report | {"block": [{"nodes": 0, "notRun": "--cases"}], "idle": {"notRun": "--cases"}}
            return report | {
                "frames": measure_frames(studio, out, FRAME_CASES),
                "block": measure_block(studio),
                "idle": measure_idle(studio),
            }
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--driver", required=True)
    parser.add_argument("--edge-colour", default="flat", choices=["flat", "gradient"],
                        help="the appearance.edgecolour mode the drawing is measured in")
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--record-baseline", type=Path)
    parser.add_argument("--commit", default="unknown")
    parser.add_argument("--cases", type=lambda text: [int(n) for n in text.split(",")],
                        help="node counts measured at DPR 1 instead of the gate's frame cases")
    parser.add_argument("--layout", help="with --cases: the layout every case is laid out with")
    parser.add_argument("--backend", choices=["auto", "canvas2d", "webgl2"],
                        help="the ?backend= the page opens with (default: none, the studio's own choice)")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args, args.out)
    except gpu.SoftwareRasteriser as failure:
        print(f"studio-perf: {failure}", file=sys.stderr)
        return 2
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-perf: could not run: {failure}", file=sys.stderr)
        return 2
    baseline = json.loads(args.baseline.read_text()) if args.baseline else None
    report["rows"] = judge.judge(report, baseline)
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    table = judge.table(report)
    (args.out / "table.md").write_text(table)
    print(table)
    if args.record_baseline:
        args.record_baseline.write_text(json.dumps(judge.baseline_of(report), indent=1) + "\n")
    return 0 if all(row["verdict"] == "PASS" for row in report["rows"] if row["gating"]) else 1


sys.exit(main())
