"""The 3D gate: a built studio served on 127.0.0.1, driven in headless Chromium.

The claim is one thing, and it is about the served app: a 3D layout is drawn, and a drag
turns it. The layout is picked from the console by typing its id, because that is the path a
user takes and the layout picker lists every registry id; the drag is real CDP mouse input,
and the node positions it claims about are read back off the view's own projection.

Usage: three.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: software raster in a container, and the numbers are the projection's own floats,
not the rasteriser's — so a red row here is about the camera, not about the load. The one
thing that cannot be measured here is how the drawing looks, and no row pretends to.
"""

import argparse
import functools
import json
import subprocess
import sys
import tempfile
import threading
from http.server import ThreadingHTTPServer
from pathlib import Path

# The CDP client is the perf gate's, not a second copy of it: one WebSocket implementation
# in the repository, and this gate drives the same browser the same way.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "perf"))
# The shared HTTP handler lives in deploy/, the directory above this one.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import cdp
import threerows as judge
from serve import QuietHandler
from threepage import SPACE_LAYOUT, Space

DEBUG_PORT = 9224


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
        f"--user-data-dir={profile}", f"--window-size=1280,800",
        "about:blank",
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def measure(dist, out, commit, broken):
    """The whole run in one function, so every exit path closes the browser and the server."""
    server = serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = launch_browser(profile)
        try:
            page = cdp.Page(DEBUG_PORT)
            served = f"http://127.0.0.1:{server.server_address[1]}/"
            studio = Space(page, served, frozen=broken)
            studio.open()
            shots = out / "shots"
            shots.mkdir(parents=True, exist_ok=True)
            return {
                "label": out.name, "commit": commit, "break": broken,
                "browser": page.call("Browser.getVersion").get("product"),
                "layout": SPACE_LAYOUT, "rows": judge.run_rows(studio, shots),
            }
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def table(report):
    head = [f"# studio-3d — {report['label']}", "",
            f"commit `{report['commit']}` · {report['browser']} · layout `{report['layout']}` · "
            f"real CDP input, software raster", "",
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
                        help="the negative control: the camera is frozen, so no drag can "
                             "change a projected position and the turn row must fail")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit, args.broken)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-3d: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
