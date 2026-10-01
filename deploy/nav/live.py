"""Studio live-force gate: a built studio settling on screen, driven with real CDP input.

Usage: live.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: the negative control takes the motor away from the page and removes the paint hook,
so no force request is sent and no frame reaches the canvas: all three rows go red. A control
that left one row green would prove nothing about it.
"""
import json
import sys
import tempfile

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import liverows as judge
from drive import VIEWPORT, Studio


def measure(dist, out, commit, broken):
    server = nav.serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = cdp.Page(nav.DEBUG_PORT)
            studio = Studio(page, f"http://127.0.0.1:{server.server_address[1]}/", wait_for_settle=False)
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
        print(f"studio-live: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = nav.table(report).replace("studio-nav", "studio-live", 1)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())