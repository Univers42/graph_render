"""Studio layout-parameter gate: the dock's Layout settings panel driven in headless Chromium.

Every row is a claim about the served app: the controls the motor's own schema asked for are on
screen, a slider moved by hand redraws and costs one run, and the console words reach the same
place. The input goes in as a real mouse gesture on the control, or as a line through the
console's own parser — never as a dispatch of the studio's API for a row that is about a control.

Usage: params.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Reuses nav.py's server, browser launch and argument shape, and drive.py's page.
"""
import argparse
import json
import sys
import tempfile
from pathlib import Path

# nav puts the perf gate's cdp.py on the path, so it is imported first.
from nav import DEBUG_PORT, launch_browser, serve

import cdp
import paramspage
import paramsrows as judge
from drive import VIEWPORT, Studio


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--commit", default="unknown")
    parser.add_argument("--break", action="store_true", dest="broken",
                        help="the negative control: the drag walks the slider to where it already is")
    return parser.parse_args()


def measure(dist, out, commit, broken):
    """The whole run in one function, so every exit path closes the browser and the server."""
    server = serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = launch_browser(profile)
        try:
            page = cdp.Page(DEBUG_PORT)
            studio = Studio(page, f"http://127.0.0.1:{server.server_address[1]}/")
            studio.open()
            paramspage.install(studio)
            return {"label": out.name, "commit": commit, "break": broken,
                    "browser": page.call("Browser.getVersion").get("product"), "viewport": VIEWPORT,
                    "rows": judge.run_rows(studio, broken)}
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def table(report):
    head = [f"# studio-params — {report['label']}", "", f"commit `{report['commit']}` · {report['browser']} · "
            f"viewport {report['viewport'][0]}x{report['viewport'][1]} · real mouse on the dock's own controls", "",
            "| row | expectation | measured | verdict |", "|---|---|---|---|"]
    body = [f"| `{r['row']}` | {r['expectation']} | {r['measured']} | {r['verdict']} |" for r in report["rows"]]
    notes = [f"`{r['row']}` not run: {r['why']}" for r in report["rows"] if r["verdict"] == "NOT-RUN"]
    return "\n".join([*head, *body, "", *(notes or ["no row was left unrun"]), ""])


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit, args.broken)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-params: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
