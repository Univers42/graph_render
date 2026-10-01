"""Studio navigation gate: a built studio served on 127.0.0.1, driven in headless Chromium.

Every row is a claim about the served app, read from the camera the app is drawing with. The
input goes in as real input — CDP mouse, wheel and key events — so nothing passes here that a
user's hand could not do; nothing is dispatched through the studio's own API to satisfy a row.

Usage: nav.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: software raster in a container, and the camera is a float the app hands the probe
through the element's own `view`. A row CDP cannot drive is reported NOT-RUN, never PASS.
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

# The CDP client is the perf gate's, not a second copy of it: one WebSocket implementation
# in the repository, and this gate drives the same browser the same way.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "perf"))

import cdp
import navrows as judge
from drive import VIEWPORT, Studio

DEBUG_PORT = 9223


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


def measure(dist, out, commit, broken):
    """The whole run in one function, so every exit path closes the browser and the server."""
    server = serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = launch_browser(profile)
        try:
            page = cdp.Page(DEBUG_PORT)
            served = f"http://127.0.0.1:{server.server_address[1]}/"
            studio = Studio(page, served, expect_drag=201 if broken else None, expect_gradient=not broken)
            studio.open()
            return {
                "label": out.name, "commit": commit, "break": broken,
                "browser": page.call("Browser.getVersion").get("product"), "viewport": VIEWPORT,
                "rows": judge.run_rows(studio),
            }
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def table(report):
    head = [f"# studio-nav — {report['label']}", "",
            f"commit `{report['commit']}` · {report['browser']} · viewport "
            f"{report['viewport'][0]}x{report['viewport'][1]} · real CDP input, software raster", "",
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
                        help="the negative control: the drag row expects a move that is not made, "
             "and the edge gradient row never turns the gradient on")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit, args.broken)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-nav: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
