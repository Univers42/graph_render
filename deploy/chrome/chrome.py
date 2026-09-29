"""Studio chrome gate: a built studio served on 127.0.0.1, sized in a real browser.

Every row is a claim about the right edge and the bottom edge of the served app, read from
the page's own layout numbers and from the pixels the compositor drew. There is no golden
image: a band of the composite counts as the canvas when it is the theme's background or a
colour the drawing carries just inside the edge (see chromerows.py for why the panels' box
shadow makes that the honest test), and a band that is a flat colour of its own — a page
background, a scrollbar track, a gap the host left — is not the canvas and fails.

The device pixel ratio is the other half. It is read once, in
`packages/graph-render/src/canvas2d/controller.ts`, and a window that moves to a screen of
another density changes it without changing a single CSS pixel, so a view that re-measures
only on a resize keeps the backing store it had.

Usage: chrome.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: overlay scrollbars on Linux mean the page never needs one, so most of this
matrix is measured on a browser that would not show the bug on a machine with classic
scrollbars. The classic run is a second launch with the overlay ones turned off, and the
honest answer for the `100vw` rows is written down in the report rather than dressed up.
"""
import argparse
import functools
import json
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

# The CDP client is the perf gate's, not a second copy of it: one WebSocket implementation
# in the repository, and this gate drives the same browser the same way.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "perf"))

import cdp
import chromerows as judge
from chromedrive import Chrome

MAIN_PORT = 9225
CLASSIC_PORT = 9226
BREAK_PORT = 9227
WINDOW = (1440, 900)

# The negative control's page: the app/index.html this job replaced, rule for rule. The
# rewrite is of the served build, not of the repository, so the repository keeps the fix.
PRE_FIX_CSS = """
      html, body { margin: 0; height: 100%; background: #1b1b1f; }
      graph-studio { width: 100vw; height: 100vh; }
"""
# What the fixed page is expected to have said instead, and what the break run reads back
# out of the served document to show the rewrite took.
FIXED_HOST_CSS = "position: fixed"


class QuietHandler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}

    def log_message(self, format, *args):  # noqa: A002 - the base class names it
        pass


def serve(dist):
    handler = functools.partial(QuietHandler, directory=str(dist))
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def launch_browser(profile, port, flags=()):
    # --no-sandbox: the container has no user namespace to build the sandbox from, and the
    # only page ever loaded is this repository's own build, served from 127.0.0.1.
    return subprocess.Popen([
        "chromium", "--headless=new", "--no-sandbox", "--disable-gpu",
        "--disable-dev-shm-usage", f"--remote-debugging-port={port}",
        f"--user-data-dir={profile}", f"--window-size={WINDOW[0]},{WINDOW[1]}",
        *flags, "about:blank",
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def with_page(dist, port, flags, work, seen):
    """A server, a browser and a page, and every one of them closed again whatever happens.

    `seen` is filled with the browser's own name, so the report says what it was measured
    on without a launch of its own.
    """
    server = serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = launch_browser(profile, port, flags)
        try:
            page = cdp.Page(port)
            seen.setdefault("browser", page.call("Browser.getVersion").get("product", "unknown"))
            studio = Chrome(page, f"http://127.0.0.1:{server.server_address[1]}/")
            return work(studio)
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


# ---------------------------------------------------------------------- the negative control


def pre_fix_copy(dist):
    """A copy of the build with its page put back to the rule it had before the fix."""
    copy = Path(tempfile.mkdtemp(prefix="studio-chrome-break-"))
    shutil.copytree(dist, copy / "dist")
    page = copy / "dist" / "index.html"
    text = page.read_text()
    head, _, rest = text.partition("<style>")
    _, _, tail = rest.partition("</style>")
    if not head or not tail:
        raise cdp.CdpError("the built index.html has no <style> block to put back")
    page.write_text(f"{head}<style>{PRE_FIX_CSS}</style>{tail}")
    # The other half of the old page: :host { position: relative; ... } lives in the built
    # bundle as a template literal. The break serves the old host rule too, or the fixed
    # `inset: 0` would keep holding and the control would measure a fixed page with an old
    # width. Ponytail: a plain string replace on minified output — if the build ever splits
    # or re-escapes that literal, the replace silently matches nothing, so the served row
    # (`chrome-break-served`) reads the computed style back and reports the rewrite.
    old_host = "display: block; position: fixed; inset: 0;"
    for js in (copy / "dist").rglob("*.js"):
        text = js.read_text()
        if old_host in text:
            js.write_text(text.replace(old_host, "display: block; position: relative;", 1))
    return copy / "dist"


def run_break(studio):
    """The pre-fix page, and an account of what it did to the right edge.

    The control has two halves. The first is a check on the check: the served page is read
    back to show the rewrite really happened, so a control that passed by serving the fixed
    build could not pass. The second is the strip itself — and there the honest answer is
    whatever the pixels say. A pre-fix page that leaves an edge behind fails; a pre-fix page
    that does not is reported NOT-RUN with the numbers, because a control that cannot be
    told apart from the fixed build is not a passing control and is not a failing one either.
    """
    studio.page.navigate(studio.url)
    studio.open(*WINDOW, 1)
    css = studio.page.evaluate("""
    (() => {
      const host = document.querySelector('graph-studio');
      const style = getComputedStyle(host);
      return { position: style.position, width: style.width, height: style.height,
               inset: [style.top, style.right, style.bottom, style.left].join(' ') };
    })()
    """)
    rewritten = css["position"] == "static"
    rows = [judge.row("chrome-break-served", "the negative control serves the pre-fix page: "
                      "the host is laid out by width and height, not by inset",
                      f"position {css['position']}, {css['width']} x {css['height']}, "
                      f"inset {css['inset']}", rewritten)]

    judged = judge.check(studio, "chrome-break-strip")
    if judged["verdict"] == "PASS":
        rows.append(judge.row(
            "chrome-break-strip", "the pre-fix page leaves a strip on the right or bottom edge",
            judged["measured"], False,
            "the pre-fix page produced numbers the fixed page also produces, so the strip did "
            "not reproduce here: " + judged["measured"]))
    else:
        rows.append(judged)
    return rows


# ------------------------------------------------------------------------------- the whole run


def measure(dist, out, commit, broken):
    """Every launch, and every row, in one place so every exit path cleans up."""
    seen = {}
    rows = []

    def launch(dist_root, port, flags, work):
        def run(studio):
            studio.open(*WINDOW, 1)
            return work(studio)
        return with_page(dist_root, port, flags, run, seen)

    rows += launch(dist, MAIN_PORT, (), lambda studio: (
        judge.run_sizes(studio)
        + judge.run_themes(studio)
        + judge.run_panels(studio)
        + judge.run_sweep(studio)
        + judge.run_dpr_change(studio)
        + judge.run_console(studio)))

    # The overlay scrollbars Linux ships are why the classic run is a second launch: the
    # flag is read once, at startup, and cannot be turned on inside a running browser.
    def classic(studio):
        rows = []
        for width, height, dpr in ((1440, 900, 1), (320, 700, 1), (2560, 1400, 2)):
            rows.append(judge.check_after(studio, width, height, dpr))
            rows[-1]["row"] = f"chrome-classic-{rows[-1]['row']}"
        return rows

    rows += launch(dist, CLASSIC_PORT, ("--disable-features=OverlayScrollbar",), classic)

    if broken:
        rows += with_page(pre_fix_copy(dist), BREAK_PORT, (), run_break, seen)

    return {
        "label": out.name, "commit": commit, "break": broken, "browser": seen.get("browser", "unknown"),
        "window": list(WINDOW), "widths": list(judge.WIDTHS), "dprs": list(judge.DPRS),
        "scrollbars": "overlay (default) and a second launch with --disable-features=OverlayScrollbar",
        "rows": rows,
    }


def table(report):
    head = [f"# studio-chrome — {report['label']}", "",
            f"commit `{report['commit']}` · {report['browser']} · window "
            f"{report['window'][0]}x{report['window'][1]} · real CDP input, software raster · "
            f"scrollbars: {report['scrollbars']}", "",
            "| row | expectation | measured | verdict |", "|---|---|---|---|"]
    body = [f"| `{r['row']}` | {r['expectation']} | {r['measured']} | {r['verdict']} |"
            for r in report["rows"]]
    counts = {}
    for r in report["rows"]:
        counts[r["verdict"]] = counts.get(r["verdict"], 0) + 1
    notes = [f"`{r['row']}` not run: {r['why']}" for r in report["rows"] if r["verdict"] == "NOT-RUN"]
    summary = [f"{counts.get(v, 0)} {v} of {len(report['rows'])} rows" for v in ("PASS", "FAIL", "NOT-RUN")]
    return "\n".join([*head, *body, "", f"**{' · '.join(summary)}**", "",
                      *(notes or ["no row was left unrun"]), ""])


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--commit", default="unknown")
    parser.add_argument("--break", action="store_true", dest="broken",
                        help="the negative control: serve the page as it was before the fix")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit, args.broken)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-chrome: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
