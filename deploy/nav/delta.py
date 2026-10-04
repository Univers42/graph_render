"""Studio delta gate: a live graph that grows while the force layout keeps running.

Ten batches of 100 nodes go into `<graph-studio>.applyDeltas` with the forces animating, and the
rows ask what a host would ask afterwards: did the drawing reach the size it was promised, is it
still moving, did nothing throw, and does a batch naming an id the graph already holds get refused
whole with nothing drawn. app/dist is served on 127.0.0.1 and driven in headless Chromium, the
shape `live.py` has; the rows themselves are in `deltarows.py`.

Usage: delta.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: the negative control is one fault in the page and none in this file. `--break` puts
`?break-deltas=1` on the URL, the studio hands it to the worker as `open.breakDeltas`
(motor/protocol.ts, motor/worker.ts) and the worker drops the `grow` that follows an extend. The
probe reads the same numbers with and without the flag, and what it measured is this: the batches
still go in and the structure snapshot still carries them, so `deltas-drawn` stays PASS (1400
drawn of 1400), and `deltas-moving` is the row that goes red — largest travel 0.000 world units,
because nothing steps the nodes the extends added. So the control is what the worker's `grow`
exists for, and the gate still exits 1.
"""
import json
import sys
import tempfile

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import smokecdp  # a page that keeps the event frames the shared client drops
import deltarows as judge  # the four rows, and the burst they measure
from drive import VIEWPORT, Studio


# ------------------------------------------------------------------ the run


def measure(args):
    """The whole run in one function, so every exit path closes the browser and the server."""
    server = nav.serve(args.dist)
    url = f"http://127.0.0.1:{server.server_address[1]}/"
    if args.broken:
        url += "?break-deltas=1"
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = smokecdp.Watcher(nav.DEBUG_PORT)
            page.start_watching()  # before the first navigation: the error domains, and the worker
            studio = Studio(page, url, wait_for_settle=False)
            studio.open()
            page.watch_workers()  # the worker's own error domains, once it has attached
            driven = judge.drive(studio)
            return {"label": args.out.name, "commit": args.commit, "break": args.broken,
                    "url": url, "browser": page.call("Browser.getVersion").get("product"),
                    "viewport": VIEWPORT, "deltas": driven,
                    "rows": judge.run_rows(page, studio, driven)}
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def one_line(rows):
    """The one line the log and the rows file read: the count when all pass, else what did not."""
    if all(one["verdict"] == "PASS" for one in rows):
        return f"studio-delta: PASS {len(rows)} rows"
    bad = [f"{one['row']} {one['verdict']}" for one in rows if one["verdict"] != "PASS"]
    return "studio-delta: FAIL " + ", ".join(bad)


def main():
    args = nav.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-delta: FAIL could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = nav.table(report).replace("studio-nav", "studio-delta", 1)
    (args.out / "table.md").write_text(printed)
    print(printed)
    print(one_line(report["rows"]))
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())