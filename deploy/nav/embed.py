"""Studio embed gate: `app/embed.html` drives `<graph-studio>` through the host API alone.

Usage: embed.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  gate mode    0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run
       --break      1 every targeted row FAIL (the controls bite) · 0 a targeted row did not
                    fail, named on stderr · 2 the harness could not run

The page (`app/src/embed.ts`) has no React of its own. It puts the element in a shadow root of its
own, sets `resolve` before the element is defined, loads a fixture with `loadGraph` and keeps every
event its `document` listener hears on `window.__embed`. The rows are the conditions of
`docs/contract/host-api.md` that only a browser can show: the load as the smoke gate judges it
(store error, banner, node count, the browser's error channels, a screenshot), the events crossing
two shadow roots, a pre-upgrade `resolve`, the three ways to open a node, two loads raced, a
refused load, storage across a reload, and the CSP a host is asked for (verdict 13).

Three gate runs, each in its own browser: `plain` without COOP/COEP (verdict 13 says they are
optional), `isolated` with them, and `csp` under HOST_CSP below with COOP/COEP off.

The negative control is one run per break, each in its own browser, and each reports only the rows
its fault targets. The faults are scripts run before the page's own, over CDP
(`embedpage.FAULTS`), a wasm that exports nothing but memory (`smokecdp.MEMORY_ONLY_WASM`) and a
CSP without `'wasm-unsafe-eval'`: nothing in `app/` or `packages/` carries a switch for the gate.
Caveat: a break proves its row can go red for that one fault, not for every fault that would
break the same condition in the wild.
"""
import argparse
import json
import shutil
import sys
import tempfile
from pathlib import Path

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import embedpage
import smokecdp
from drive import Studio
from embedgestures import step_dblclick, step_enter, step_open, step_overlap, step_refused
from embedrows import (step_channels, step_composed, step_load, step_pick, step_resolve, step_select,
                       step_storage)

# Verdict 13: what a host serving the studio must allow, and nothing more.
HOST_CSP = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; img-src 'self' data:"
NO_WASM_CSP = HOST_CSP.replace(" 'wasm-unsafe-eval'", "")
FIXTURE = "fixtures/force/clustered.json"
WASM = "graph_wasm.wasm"

STEPS = {"load": step_load, "pick": step_pick, "select": step_select, "resolve": step_resolve,
         "dblclick": step_dblclick, "enter": step_enter, "open": step_open, "overlap": step_overlap,
         "refused": step_refused, "channels": step_channels, "composed": step_composed,
         "storage": step_storage}
# The order matters: `pick` again once the inspector is open, the error channels after the refused
# load, and `composed` (which reads the whole page's history) before `storage` reloads the page.
FULL = ("load", "pick", "select", "resolve", "pick", "dblclick", "enter", "open", "overlap", "refused",
        "channels", "composed", "storage")
HEALTH_ROWS = ("embed-no-store-error", "embed-no-overlay", "embed-drew-nodes", "embed-host-load",
               "embed-no-exception", "embed-no-console-error")


def run(name, steps, isolated=False, csp=None, faults=(), keep=None, **more):
    """One browser session: its serving, its faults, its steps and the rows a break reports."""
    return {"name": name, "steps": steps, "isolated": isolated, "csp": csp, "faults": faults,
            "keep": keep, "broken_wasm": more.get("broken_wasm", False),
            "overlap_awaits": more.get("overlap_awaits", False)}


GATE = (run("plain", FULL), run("isolated", FULL, isolated=True),
        run("csp", ("load", "channels"), csp=HOST_CSP))

BREAKS = (
    run("break-health", ("load", "channels"), faults=(smokecdp.INJECTED_THROW,), keep=HEALTH_ROWS,
        broken_wasm=True),
    run("break-remember", ("load", "storage"), faults=("remember",), keep=("embed-storage",)),
    run("break-overlap", ("load", "overlap"), keep=("embed-overlap",), overlap_awaits=True),
    run("break-composed", ("load", "pick", "select", "pick", "dblclick", "refused", "composed"),
        faults=("composed",), keep=("embed-composed", "embed-load-refused")),
    run("break-dblclick", ("load", "pick", "dblclick"), faults=("dblclick",), keep=("embed-dblclick",)),
    run("break-enter", ("load", "pick", "enter"), faults=("enter",), keep=("embed-enter",)),
    run("break-open", ("load", "pick", "open"), faults=("open",), keep=("embed-open",)),
    run("break-upgrade", ("load", "pick", "select", "resolve"), faults=("upgrade",),
        keep=("embed-resolve-upgrade",)),
    run("break-csp", ("load", "channels"), csp=NO_WASM_CSP,
        keep=("embed-no-console-error", "embed-drew-nodes", "embed-host-load")),
)


def fixture_counts(served):
    doc = json.loads((served / FIXTURE).read_text())
    return {"nodes": len(doc["nodes"]), "edges": len(doc["edges"])}


def served_copy(dist, broken_wasm):
    """`dist` itself, or a scratch copy whose wasm exports memory only; (served, scratch)."""
    if not broken_wasm:
        return dist, None
    scratch = Path(tempfile.mkdtemp())
    shutil.copytree(dist, scratch / "dist")
    (scratch / "dist" / WASM).write_bytes(smokecdp.MEMORY_ONLY_WASM)
    return scratch / "dist", scratch


def drive(page, spec, ctx):
    """Every step of `spec` in order, then only the rows it keeps."""
    for fault in spec["faults"]:
        page.throw_on_load(embedpage.FAULTS.get(fault, fault))
    rows = [row for step in spec["steps"] for row in STEPS[step](page, ctx)]
    kept = rows if spec["keep"] is None else [row for row in rows if row["row"] in spec["keep"]]
    return [dict(row, run=spec["name"]) for row in kept]


def measure_run(spec, args):
    """One run in its own server and browser; every exit path closes both and the copy."""
    served, scratch = served_copy(args.dist, spec["broken_wasm"])
    server = nav.serve(served, isolated=spec["isolated"], csp=spec["csp"])
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = smokecdp.Watcher(nav.DEBUG_PORT)
            page.start_watching()
            url = f"http://127.0.0.1:{server.server_address[1]}/embed.html"
            ctx = {"url": url, "out": args.out, "run": spec["name"], "hand": Studio(page, url), "point": None,
                   "fixture": fixture_counts(served), "overlap_awaits": spec["overlap_awaits"]}
            return drive(page, spec, ctx), page.call("Browser.getVersion").get("product")
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
            if scratch is not None:
                shutil.rmtree(scratch, ignore_errors=True)


def measure(args):
    rows, browser = [], None
    for spec in BREAKS if args.broken else GATE:
        measured, browser = measure_run(spec, args)
        rows.extend(measured)
    return {"label": args.out.name, "commit": args.commit, "break": args.broken, "browser": browser,
            "runs": [spec["name"] for spec in (BREAKS if args.broken else GATE)], "rows": rows}


def table(report):
    mode = "negative control" if report["break"] else "gate"
    head = [f"# studio-embed — {report['label']} ({mode})", "",
            f"commit `{report['commit']}` · {report['browser']} · runs {', '.join(report['runs'])} · "
            "screenshots `studio-embed-<run>.png`", "",
            "| run | row | expectation | measured | verdict |", "|---|---|---|---|---|"]
    body = [f"| {r['run']} | `{r['row']}` | {r['expectation']} | {r['measured'].replace('|', '/')} | {r['verdict']} |"
            for r in report["rows"]]
    notes = [f"{r['run']} `{r['row']}` not run: {r['why']}" for r in report["rows"] if r["verdict"] == "NOT-RUN"]
    return "\n".join([*head, *body, "", *(notes or ["no row was left unrun"]), ""])


def exit_code(report):
    """Gate: 0 only when every row passed. Break: 1 only when every targeted row failed."""
    rows = report["rows"]
    if not report["break"]:
        return 0 if rows and all(r["verdict"] == "PASS" for r in rows) else 1
    held = [f"{r['run']}/{r['row']} ({r['verdict']})" for r in rows if r["verdict"] != "FAIL"]
    if rows and not held:
        return 1
    print(f"studio-embed: the negative control did not bite: {', '.join(held) or 'no row reported'}",
          file=sys.stderr)
    return 0


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--commit", default="unknown")
    parser.add_argument("--break", action="store_true", dest="broken",
                        help="the negative control: one run per fault, each reporting the rows it targets")
    return parser.parse_args()


def main():
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-embed: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return exit_code(report)


if __name__ == "__main__":
    sys.exit(main())
