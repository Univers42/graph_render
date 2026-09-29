"""Studio local-graph and settings gate: a built studio served on 127.0.0.1, driven in headless Chromium.

Usage: local.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

The server, the browser launch, the CDP client and the input library are the navigation
gate's (deploy/nav/) and the perf gate's; this file adds only its rows.

Ponytail: software raster in a container; "inside the viewport" is read from the count of
nodes the renderer painted, which skips nodes off the canvas, so a node drawn one pixel
inside the edge counts as inside. A row CDP cannot drive is NOT-RUN, never PASS.
"""
import argparse
import json
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "nav"))
sys.path.insert(0, str(HERE.parent / "perf"))

import cdp
import localrows
import nav
import settingsrows
from drive import VIEWPORT
from localdrive import LocalStudio

# The navigation gate owns 9223; a distinct port lets the two run in one afternoon's sequence
# without waiting for a browser to release its socket.
nav.DEBUG_PORT = 9224


def measure(dist, out, commit, broken):
    server = nav.serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = cdp.Page(nav.DEBUG_PORT)
            studio = LocalStudio(page, f"http://127.0.0.1:{server.server_address[1]}/")
            studio.open()
            return {
                "label": out.name, "commit": commit, "break": broken,
                "browser": page.call("Browser.getVersion").get("product"), "viewport": VIEWPORT,
                "rows": [*localrows.run_rows(studio, broken), *settingsrows.run_rows(studio)],
            }
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--commit", default="unknown")
    parser.add_argument("--break", action="store_true", dest="broken",
                        help="the negative control: the depth-1 row expects one node the walk does not reach")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit, args.broken)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-local: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = nav.table(report).replace("studio-nav", "studio-local", 1)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
