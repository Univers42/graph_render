"""The SciGraphs parity gate: the built studio's parity page, driven in headless Chromium.

Usage: run.py --dist DIR --out DIR [--break] [--commit ID]
Exit:  0 every gating row PASS · 1 a gating row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: software raster in a container on a shared host. The gate judges bytes, not
speed, so nothing here depends on how fast the machine was.
"""

import argparse
import functools
import json
import socket
import subprocess
import sys
import tempfile
import threading
import time
from http.server import ThreadingHTTPServer
from pathlib import Path

# The gate drives the browser with the perf gate's own CDP client, not a second one. It is
# appended, not prepended, so deploy/perf/rows.py cannot answer for this gate's rows.
sys.path.append(str(Path(__file__).resolve().parent.parent / "perf"))
# The shared HTTP handler lives in deploy/, the directory above this one.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import cdp  # noqa: E402
import png  # noqa: E402
import rows as judge  # noqa: E402
import spec  # noqa: E402
from serve import QuietHandler  # noqa: E402

# The fig1 frame the fixture was projected into (05-reproducible-pipeline.qmd:660-664).
VIEWPORT = (1920, 1080)
PARITY_PAGE = "parity.html"
FIXTURE = "fixtures/scigraphs/lesmis.json"
REFERENCE = "/refs/matplotlib-3.10.0/_cm_listed.py"
# The page's own state, or the error it reported, or null while it has neither.
STATE = ("JSON.stringify(window.__parity"
         " ?? (window.__parityError ? {error: window.__parityError} : null))")
# The page fetches the fixture and paints a frame; a slow host gets longer, not a verdict.
STATE_SECONDS = 30


def serve(dist):
    handler = functools.partial(QuietHandler, directory=str(dist))
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def free_port():
    """A port nothing is listening on, so two gates on one host cannot collide.

    Ponytail: the perf gate pins 9222 and a second run on the same host can drive the first
    run's browser, which is a NOT-RUN rather than a wrong answer. Binding port 0 and taking
    the number narrows that to the microseconds between the close and the browser's bind.
    """
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def launch_browser(profile, port):
    # --no-sandbox: the container has no user namespace to build the sandbox from, and the
    # only page ever loaded is this repository's own build, served from 127.0.0.1.
    return subprocess.Popen([
        "chromium", "--headless=new", "--no-sandbox", "--disable-gpu",
        "--disable-dev-shm-usage", f"--remote-debugging-port={port}",
        f"--user-data-dir={profile}", f"--window-size={VIEWPORT[0]},{VIEWPORT[1]}",
        "about:blank",
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def page_state(page, seconds):
    """The page's own state, once it has one; a page that never gets one is a null."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = page.evaluate(STATE)
        if value is not None:
            return json.loads(value)
        time.sleep(0.1)
    return None


def measure(args):
    """The page as it drew it: its own state, its own screenshot, and the fixture beside it."""
    server = serve(args.dist)
    port = free_port()
    with tempfile.TemporaryDirectory() as profile:
        browser = launch_browser(profile, port)
        try:
            page = cdp.Page(port)
            page.set_viewport(VIEWPORT[0], VIEWPORT[1], 1)
            page.navigate(f"http://127.0.0.1:{server.server_address[1]}/{PARITY_PAGE}")
            state = page_state(page, STATE_SECONDS)
            shot = args.out / "parity.png"
            page.screenshot(str(shot))
            return {
                "label": args.out.name, "commit": args.commit, "viewport": list(VIEWPORT),
                "state": state, "fixture": json.loads((args.root / FIXTURE).read_text()),
                "png": str(shot), "breaks": args.broken,
            }
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--root", type=Path, default=Path("/w"))
    parser.add_argument("--reference", default=REFERENCE)
    parser.add_argument("--break", dest="broken", action="store_true", help="the negative control")
    parser.add_argument("--commit", default="unknown")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args)
        report["rows"] = judge.judge(report, spec.load(args.reference))
        (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
        table = judge.table(report)
        (args.out / "report.txt").write_text(table)
        print(table)
    except (cdp.CdpError, OSError, png.PngError, ValueError) as failure:
        print(f"studio-parity: could not run: {failure}", file=sys.stderr)
        return 2
    gating = [row for row in report["rows"] if row["gating"]]
    return 0 if gating and all(row["verdict"] == "PASS" for row in gating) else 1


sys.exit(main())
