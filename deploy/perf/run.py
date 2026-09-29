"""Studio perf gate: serve a built studio, drive it in headless Chromium, judge the rows.

Usage: run.py --dist DIR --out DIR --driver NAME [--baseline FILE] [--record-baseline FILE]
              [--commit ID]
Exit:  0 every gating row PASS · 1 a gating row FAIL or NOT-RUN · 2 the harness could not run

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
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import cdp
import rows as judge

HERE = Path(__file__).resolve().parent
# A full-HD window: at DPR 2 the graph canvas is about 3000x2000, the size a desktop user has.
VIEWPORT = (1920, 1080)
DEBUG_PORT = 9222
FORCE_LAYOUT = "layout.forceatlas2"
# Measured in the motor alone (2026-09-29): forceatlas2 takes 66 s at 20 000 nodes, pivot MDS
# 1.3 s. The frame rows time the drawing, not the layout, so the large case is laid out by
# the one that finishes; each case records which.
LARGE_LAYOUT = "layout.mds.pivot"
LARGE_FROM = 5000
FRAME_CASES = [(120, 1), (120, 2), (2000, 1), (2000, 2), (20000, 1), (20000, 2)]
BLOCK_NODES = [120, 500]
PROFILED_CASE = (2000, 2)


class QuietHandler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}

    def log_message(self, format, *args):  # noqa: A002 - the base class names it
        pass


def serve(dist):
    handler = functools.partial(QuietHandler, directory=str(dist))
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def launch_browser(profile):
    # --no-sandbox: the container has no user namespace to build the sandbox from, and the
    # only page ever loaded is this repository's own build, served from 127.0.0.1.
    return subprocess.Popen([
        "chromium", "--headless=new", "--no-sandbox", "--disable-gpu",
        "--disable-dev-shm-usage", f"--remote-debugging-port={DEBUG_PORT}",
        f"--user-data-dir={profile}", f"--window-size={VIEWPORT[0]},{VIEWPORT[1]}",
        "about:blank",
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


class Studio:
    """The page, plus the driver that knows how to operate this studio."""

    def __init__(self, page, url, driver):
        self.page = page
        self.url = url
        self.driver = (HERE / "drivers" / f"{driver}.js").read_text()

    def open(self, nodes, dpr, layout=FORCE_LAYOUT):
        self.page.set_viewport(VIEWPORT[0], VIEWPORT[1], dpr)
        self.page.navigate("about:blank")
        self.page.navigate(self.url)
        self.page.evaluate(self.driver)
        limit = self.page.evaluate("window.__perf.maxNodes")
        if nodes > limit:
            return f"driver caps at {limit} nodes"
        self.page.evaluate(f"window.__perf.open({nodes}, {json.dumps(layout)})")
        return None

    def probe(self, name, args):
        source = (HERE / "probes" / f"{name}.js").read_text()
        return self.page.evaluate(f"({source})({json.dumps(args)})")


def measure_frames(studio, out):
    cases = []
    for nodes, dpr in FRAME_CASES:
        layout = LARGE_LAYOUT if nodes >= LARGE_FROM else FORCE_LAYOUT
        skipped = studio.open(nodes, dpr, layout)
        if skipped is not None:
            cases.append({"nodes": nodes, "dpr": dpr, "notRun": skipped})
            continue
        case = {"nodes": nodes, "dpr": dpr, "layout": layout, **studio.probe("frame", {"settleMs": 1500, "profile": False})}
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
        browser = launch_browser(profile)
        try:
            page = cdp.Page(DEBUG_PORT)
            studio = Studio(page, f"http://127.0.0.1:{server.server_address[1]}/", args.driver)
            version = page.call("Browser.getVersion").get("product")
            return {
                "label": out.name, "driver": args.driver, "commit": args.commit,
                "browser": version, "viewport": VIEWPORT,
                "frames": measure_frames(studio, out),
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
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--record-baseline", type=Path)
    parser.add_argument("--commit", default="unknown")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args, args.out)
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
