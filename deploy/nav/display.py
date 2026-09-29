"""Studio display gate: a built studio served on 127.0.0.1, its display panel driven in headless Chromium.

Every row sets a control the way the dock does and reads the view's state back; a row that cannot
be read that way says so and is NOT-RUN. Reuses nav.py's server and browser launch and drive.py's page.

Usage: display.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: software raster in a container; pixels are read only for the theme corner and the glow
row, and glow's "around a node" is the whole canvas, so a glow that moved pixels elsewhere would pass.
"""
import json
import sys
import tempfile

# nav puts the perf gate's cdp.py on the path, so it is imported first.
from nav import DEBUG_PORT, launch_browser, parse_args, serve

import cdp
import displayrows as first
import displayrows2 as second
import displaylib
from drive import VIEWPORT, Studio

ROWS = (
    first.row_arrows, first.row_fade, first.row_fade_preset, first.row_size, first.row_size_degree,
    first.row_thickness, first.row_edge_style, second.row_themes, second.row_glow,
    second.row_console, second.row_animate, second.row_dock,
)


def measure(dist, out, commit, broken):
    server = serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = launch_browser(profile)
        try:
            page = cdp.Page(DEBUG_PORT)
            studio = Studio(page, f"http://127.0.0.1:{server.server_address[1]}/")
            studio.open()
            displaylib.install(studio)
            rows = []
            for make in ROWS:
                try:
                    rows.append(make(studio, broken))
                except cdp.CdpError as failure:
                    rows.append({"row": make.__name__, "expectation": "the row runs", "measured": str(failure),
                                 "verdict": "NOT-RUN", "why": f"the page refused: {failure}"})
            return {"label": out.name, "commit": commit, "break": broken,
                    "browser": page.call("Browser.getVersion").get("product"), "viewport": VIEWPORT, "rows": rows}
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def table(report):
    head = [f"# studio-display — {report['label']}", "", f"commit `{report['commit']}` · {report['browser']} · "
            f"viewport {report['viewport'][0]}x{report['viewport'][1]}", "",
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
        print(f"studio-display: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
