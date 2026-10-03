"""Studio load-smoke gate: does the built studio come up at all, and say so if it does not.

Usage: smoke.py --dist DIR --out DIR [--break] [--break-coi] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

The 2026-10-01 failure: `scripts/studio.sh` served whatever `target/` held, so the page loaded a
wasm older than the SDK, the studio died with `exports.gm_dim is not a function`, and every
browser gate passed anyway — each one reads its own rows and none of them looked at the load. These
rows are the ones that look: the browser's error channels, the studio's own store, the banner in
the shadow root, and the node count on the canvas.

Ponytail: the negative control is two faults, one per cause, and both go red for the same reason
they would in the wild. (1) A valid wasm that exports `memory` and nothing else is served where
`graph_wasm.wasm` was: the loader refuses it by name, the SDK logs the refusal inside the motor
worker and latches the failure, and the studio comes up degraded — store error, banner, no nodes,
one console error. Nothing pokes the page to force that; the fault is in the bytes served. (2) A
script that throws before any studio code runs, injected over CDP, because the 2026-10-01 incident's
`TypeError` no longer escapes anywhere: the SDK latches the loader's refusal, so the stale wasm of
fault (1) reaches the page as state and never as an exception. See `smokecdp.INJECTED_THROW`.
Third control: `--break-coi` serves without COOP/COEP, so `smoke-cross-origin-isolated` fails.
"""
import argparse
import json
import shutil
import sys
import tempfile
import time
from pathlib import Path

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import smokecdp
import smokerows as judge
from drive import VIEWPORT, Studio

# The file the element resolves from app/index.html's `wasm="graph_wasm.wasm"`.
WASM = "graph_wasm.wasm"
# How long the studio has to come up and hand back a graph. It builds the fixture, opens a motor
# session and lays it out, all on load.
LOAD_CAP_S = 90.0


def settle(page, url):
    """Wait for the studio to come up, then for the drawing to stop moving.

    Unlike the other gates this does not treat a store error as a harness failure: a studio that
    failed to load is the thing being measured, and its rows have to run to say so.
    """
    page.set_viewport(VIEWPORT[0], VIEWPORT[1], 1)
    page.navigate("about:blank")
    page.navigate(url)
    page.evaluate("customElements.whenDefined('graph-studio')")
    deadline = time.monotonic() + LOAD_CAP_S
    while time.monotonic() < deadline:
        page.watch_workers()
        at = page.evaluate(judge.PROBE)
        if at is not None and (at["error"] is not None or (at["meta"] is not None and at["busy"] == 0)):
            return settle_drawing(page, url, at)
        time.sleep(0.2)
    raise cdp.CdpError(f"the studio drew no graph in {LOAD_CAP_S:.0f}s")


def settle_drawing(page, url, at):
    """Let the nodes travel to where the layout put them, then re-read the page once they stop."""
    if at["nodes"] > 0:
        Studio(page, url).settle_drawing()
    return page.evaluate(judge.PROBE)


def measure(args):
    """The whole run in one function, so every exit path closes the browser, the server and the copy.

    The arguments come as one namespace (the parity gate's shape): `--break` and `--break-coi` are
    two faults in one run, and five positional parameters would cross the house limit of four.
    """
    served, scratch = args.dist, None
    if args.broken:
        scratch = Path(tempfile.mkdtemp())
        served = scratch / "dist"
        shutil.copytree(args.dist, served)
        (served / WASM).write_bytes(smokecdp.MEMORY_ONLY_WASM)
    server = nav.serve(served, isolated=not args.break_coi)
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = smokecdp.Watcher(nav.DEBUG_PORT)
            page.start_watching()
            if args.broken:
                page.throw_on_load(smokecdp.INJECTED_THROW)
            url = f"http://127.0.0.1:{server.server_address[1]}/"
            at = settle(page, url)
            page.call("Runtime.evaluate", {"expression": "1"})  # drain what is still in the socket
            page.watch_workers()
            page.screenshot(args.out / "studio-smoke.png")
            return {"label": args.out.name, "commit": args.commit, "break": args.broken, "url": url,
                    "browser": page.call("Browser.getVersion").get("product"),
                    "viewport": VIEWPORT, "wasm_bytes": (served / WASM).stat().st_size,
                    "rows": judge.run_rows(page, at)}
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
            if scratch is not None:
                shutil.rmtree(scratch, ignore_errors=True)


def table(report):
    """The nav table with this gate's name, so one renderer serves all the browser gates."""
    head = [f"# studio-smoke — {report['label']}", "",
            f"commit `{report['commit']}` · {report['browser']} · viewport "
            f"{report['viewport'][0]}x{report['viewport'][1]} · graph_wasm.wasm "
            f"{report['wasm_bytes']} bytes served · screenshot `studio-smoke.png`", "",
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
                        help="the negative control: serve a valid wasm that exports memory only, "
                             "and throw before the page's own scripts")
    parser.add_argument("--break-coi", action="store_true", dest="break_coi",
                        help="the negative control: serve without the COOP/COEP headers, so "
                             "neither the page nor the motor worker is isolated and the row must fail")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-smoke: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
