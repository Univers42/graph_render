"""Studio filtering gate: a built studio served on 127.0.0.1, driven in headless Chromium.

Every row is a claim about the served app, read from the studio's own registry and the view's
own state. The input goes in through the studio's own registry (`dispatch(id, raw)` and the
console line `run(line)`), because a filter is not a gesture: it is what a dock control and a
typed line both do.

Usage: filters.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: the probe re-implements the query grammar and the colormap in Python so a row is
an oracle and not a second copy of the product agreeing with itself — but a disagreement
about the grammar's spelling (is `path:` a prefix?) is then a red row about the probe.
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

# The CDP client is the perf gate's, not a second copy of it: one WebSocket implementation
# in the repository, and this gate drives the same browser the same way.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "perf"))

import cdp
import filtersrows as judge

DEBUG_PORT = 9224


class QuietHandler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}

    def log_message(self, *_):
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
        f"--user-data-dir={profile}", "--window-size=1400,900",
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
            studio = Studio(page, served)
            studio.open()
            return {
                "label": out.name, "commit": commit, "break": broken,
                "browser": page.call("Browser.getVersion").get("product"), "viewport": (1400, 900),
                "rows": judge.run_rows(studio, broken),
            }
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def table(report):
    head = [f"# studio-filters — {report['label']}", "",
            f"commit `{report['commit']}` · {report['browser']} · viewport "
            f"{report['viewport'][0]}x{report['viewport'][1]} · registry + view state", "",
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
                        help="the negative control: the probe's evaluator drops NOT")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit, args.broken)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-filters: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


class Studio:
    """The page, and the studio's own registry and view state."""

    def __init__(self, page, url):
        self.page = page
        self.url = url

    def open(self):
        self.page.set_viewport(1400, 900, 1)
        self.page.navigate("about:blank")
        self.page.navigate(self.url)
        self.page.evaluate("customElements.whenDefined('graph-studio')")
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            state = self.page.evaluate("""
            (() => {
              const s = document.querySelector('graph-studio').studio;
              if (s === null) return null;
              const at = s.store.get();
              return { done: at.busy.length === 0 && at.meta !== null, error: at.error };
            })()
            """)
            if state is None:
                time.sleep(0.2)
                continue
            if state["error"] is not None:
                raise cdp.CdpError(f"the studio failed to open: {state['error']}")
            if state["done"]:
                # The nodes travel to where the layout put them; a camera read mid-flight
                # would be a camera read of a drawing that is still arriving.
                time.sleep(1.5)
                return
            time.sleep(0.2)
        raise cdp.CdpError("the studio drew no graph in 90s")

    def read(self):
        report = self.page.evaluate("""
        (() => {
          const host = document.querySelector('graph-studio');
          const studio = host === null ? null : host.studio;
          const view = host === null ? null : host.view;
          const canvas = host === null ? null : host.shadowRoot.querySelector('canvas');
          if (studio === null || view === null || canvas === null) return null;
          const box = canvas.getBoundingClientRect();
          return {
            camera: view.camera(), limits: view.limits(), selected: studio.store.get().selected,
            box: [box.left, box.top, box.width, box.height], dpr: devicePixelRatio,
            nodes: view.stats().nodes,
          };
        })()
        """)
        if report is None:
            raise cdp.CdpError("the page has no graph-studio with a studio and a view on it")
        return report


if __name__ == "__main__":
    sys.exit(main())