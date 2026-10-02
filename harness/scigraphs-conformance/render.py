#!/usr/bin/env python3
"""Render the SVGs the metrics arm wrote into PNGs, with the repo's `gm-chromium` image.

Run inside the chromium image, with the repository at /w and the fixture directory mounted:

    docker run --rm -v "$PWD:/w" -w /w gm-chromium \
        python3 harness/scigraphs-conformance/render.py target/scigraphs-conformance/shapes \
            --out target/scigraphs-conformance/png

Exit: 0 every PNG written · 1 one could not be · 2 the browser could not be driven.

**Software raster in a container, driven over CDP rather than by a screenshot flag**, because
that is what `deploy/parity/run.py` already does and a second browser driver would be a second
thing to keep working. The CDP client is `deploy/perf/cdp.py` — the perf gate's — imported
rather than copied, so a change to how this repository drives Chromium changes this gate too.

**No new dependency.** The shapes are SVG (text) and the page that shows them is HTML, so the
whole renderer is a page load and a screenshot: `chromium`, the Python standard library and
the CDP client that was already in the tree. Nothing here reaches for cairosvg, rsvg or a
headless-shell binary.

The contact sheet is a single wide page holding all 32 lesmis panels, screenshotted once, so
"all 32 at a glance" is one file rather than 32 reads.
"""

import argparse
import functools
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

# `deploy/perf/cdp.py` — the gate's own browser client (`deploy/parity/run.py:24-25`).
sys.path.append(str(Path(__file__).resolve().parent.parent.parent / "deploy" / "perf"))

import cdp  # noqa: E402

#: The viewport. Wide enough for the contact sheet's six columns of three-panel SVGs.
VIEWPORT = (1800, 1400)

#: How long a page gets to paint before its screenshot is taken. A slow host gets longer, not
#: a verdict: nothing here measures time.
PAINT_SECONDS = 2.0


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, format, *args):  # noqa: A002 - the base class names it
        pass


def free_port():
    """A port nothing is listening on, so two renders on one host cannot collide."""
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def launch(profile, port):
    """Headless Chromium. `--no-sandbox` because the container has no user namespace to build
    a sandbox from, and the only pages ever loaded are this repository's own SVG files, served
    from 127.0.0.1."""
    return subprocess.Popen(
        [
            "chromium", "--headless=new", "--no-sandbox", "--disable-gpu",
            "--disable-dev-shm-usage", f"--remote-debugging-port={port}",
            f"--user-data-dir={profile}", f"--window-size={VIEWPORT[0]},{VIEWPORT[1]}",
            "about:blank",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def page_for(name, root, http_port, devtools_port, shot):
    """One page: the SVG or the sheet, a moment for it to paint, then the screenshot.

    The URL is `/<name>` and the existence check is `<root>/<name>`, because the HTTP root is
    the shapes directory: serving it is what lets one `sheet.html` load all 32 SVGs with
    relative paths and no second copy of anything.

    A missing file is a **failure, not a blank PNG**: a white rectangle in a directory of
    evidence is worse than a name in the log, because it looks like a shape.
    """
    if not os.path.exists(os.path.join(root, name)):
        return False, "%s: not written" % name
    page = cdp.Page(devtools_port)
    page.set_viewport(VIEWPORT[0], VIEWPORT[1], 1)
    page.navigate("http://127.0.0.1:%d/%s" % (http_port, name))
    time.sleep(PAINT_SECONDS)
    page.screenshot(str(shot))
    return True, None


def render(shapes, out, report):
    """Every SVG and the sheet, in one browser and one server.

    **The shapes directory is what is served**, not the fixture directory: `sheet.html`
    references its SVGs relatively, so serving it is what lets one page load all 32 without a
    second copy of anything.
    """
    os.makedirs(out, exist_ok=True)
    targets = sorted(name for name in os.listdir(shapes) if name.endswith(".svg"))
    targets += sorted(
        name for name in os.listdir(shapes)
        if name.endswith(".html") and name.startswith("sheet")
    )
    handler = functools.partial(QuietHandler, directory=shapes)
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    failures = []
    with tempfile.TemporaryDirectory() as profile:
        devtools = free_port()
        browser = launch(profile, devtools)
        try:
            for name in targets:
                png = os.path.join(out, name.replace(".svg", ".png").replace(".html", ".png"))
                ok, why = page_for(
                    name, shapes, server.server_address[1], devtools, png,
                )
                report.append({"svg": name, "png": os.path.basename(png), "ok": ok, "why": why})
                if not ok:
                    failures.append(why)
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
    return failures


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("shapes", help="the directory holding the SVGs and sheet.html")
    parser.add_argument("--out", default="target/scigraphs-conformance/png")
    return parser.parse_args(argv[1:])


def main(argv):
    args = parse_args(argv)
    shapes = Path(args.shapes)
    if not shapes.is_dir():
        print("scigraphs-render: %s is not a directory" % shapes, file=sys.stderr)
        return 2
    report = []
    try:
        failures = render(str(shapes), args.out, report)
    except (cdp.CdpError, OSError) as failure:
        print("scigraphs-render: could not run: %s" % failure, file=sys.stderr)
        return 2
    index = os.path.join(args.out, "render.json")
    with open(index, "w") as handle:
        json.dump({"viewport": list(VIEWPORT), "pngs": report}, handle, indent=1, sort_keys=True)
        handle.write("\n")
    print("scigraphs-render: %d PNGs in %s" % (len(report) - len(failures), args.out))
    for why in failures:
        print("  FAIL %s" % why, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
