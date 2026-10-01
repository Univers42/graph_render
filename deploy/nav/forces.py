"""Studio forces gate: the Forces panel in a built studio, driven with real CDP input.

Usage: forces.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: the negative control removes the panel from the page before the rows read it, so it
proves the rows notice a missing panel, not that a missing registration would do so.
"""
import json
import sys
import tempfile

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import forcesrows as judge
from drive import VIEWPORT, Studio


def measure(dist, out, commit, broken):
    server = nav.serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = cdp.Page(nav.DEBUG_PORT)
            studio = Studio(page, f"http://127.0.0.1:{server.server_address[1]}/")
            studio.open()
            return {"label": out.name, "commit": commit, "break": broken,
                    "browser": page.call("Browser.getVersion").get("product"), "viewport": VIEWPORT,
                    "rows": judge.run_rows(studio, broken)}
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def main():
    args = nav.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit, args.broken)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-forces: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = nav.table(report).replace("studio-nav", "studio-forces", 1)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
