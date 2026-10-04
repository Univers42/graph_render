"""Studio delta gate: a live graph that grows while the force layout keeps running.

Ten batches of 100 nodes go into `<graph-studio>.applyDeltas` with the forces animating, and the
rows ask what a host would ask afterwards: did the drawing reach the size it was promised, is it
still moving, did nothing throw, and does a batch naming an id the graph already holds get refused
whole with nothing drawn. app/dist is served on 127.0.0.1 and driven in headless Chromium, the
shape `live.py` has.

Usage: delta.py --dist DIR --out DIR [--break] [--commit ID]   (run from this directory)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

Ponytail: the negative control is one fault in the page and none in this file. `--break` puts
`?break-deltas=1` on the URL, the studio hands it to the worker as `open.breakDeltas`
(motor/protocol.ts, motor/worker.ts) and the worker drops the `grow` that follows an extend. The
probe reads the same numbers with and without the flag: the drawing never grows past its base, so
`deltas-drawn` and `deltas-moving` go red on the thresholds they pass otherwise.
"""
import json
import sys
import tempfile
import time

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import liverows  # the drawing's own readers: HOST/ROOT, SAMPLE_S, MOVING_PX, positions()
import smokecdp  # a page that keeps the event frames the shared client drops
import smokerows  # how a thrown or logged failure is described, and which ones are errors
import verdict  # one row's shape, the way every other nav gate's rows module builds it
from drive import VIEWPORT, Studio, apart

# The burst: how many calls, of how many nodes each. Ten by 100 is what a host streams, and ten
# is more than one frame, so the per-frame coalescing has something to coalesce.
CALLS, BATCH = 10, 100
# How long the drawing has to reach `base + CALLS * BATCH` after the last call's answer.
GROW_S = 2.0
# How often the drawn count is read while it grows, and where the motor worker's error domains
# get turned on (smokecdp.Watcher.watch_workers): an event sits in the socket until something
# reads, and this client only reads inside `call`.
POLL_S = 0.1
# The rows that drive the verb cannot be measured without it: host-api condition 8 has hosts
# feature-test `"applyDeltas" in el`, so its absence is NOT-RUN, never a silent pass.
MISSING = "`el.applyDeltas` is not on the element, so no batch was applied"


# ------------------------------------------------------------------ the batch, as the page reads


def record(node_id):
    """One ingest v1 node record (docs/contract/wasm-abi.md, Ingest — PROVISIONAL)."""
    return {"id": node_id, "kind": "record", "database_id": None, "source": "delta",
            "label": node_id, "group": None, "weight": 1, "version": 1,
            "has_note": False, "icon": None}


def batch_json(prefix, count, first_id=None):
    """One batch as the page's own JSON: `count` nodes named `prefix-<i>`, and no edges.

    `first_id` replaces the first node's id, which is how the refusal row gets an id the graph
    already holds into an otherwise valid batch. Ids are unique across the burst, because a
    repeated id is what refuses a batch.
    """
    ids = [f"{prefix}-{i}" for i in range(count)]
    if first_id is not None:
        ids[0] = first_id
    return json.dumps({"nodes": [record(node_id) for node_id in ids], "edges": []})


def push(studio):
    """`CALLS` batches through `el.applyDeltas`: one call per batch, one answer per call.

    Sequential, as a host does it — each call is awaited before the next is made — so a refusal in
    the middle is one answer among ten, not the end of the run. A rejection is caught here and
    answered as data: a refused batch is a row's expectation, not a page fault.
    """
    # Each batch is already JSON text and is spliced into the page's own source, so the list is
    # joined by hand: `json.dumps` over them would encode ten strings where ten objects belong.
    batches = "[" + ", ".join(batch_json(f"delta-{call}", BATCH) for call in range(CALLS)) + "]"
    return studio.page.evaluate(f"""
    (async () => {{
      const el = {liverows.HOST};
      if (typeof el.applyDeltas !== 'function') return {{ missing: true }};
      const calls = [];
      for (const batch of {batches}) {{
        try {{ calls.push({{ ok: true, answer: await el.applyDeltas(batch) }}); }}
        catch (failure) {{
          calls.push({{ ok: false, name: failure.name, detail: String(failure.message) }});
        }}
      }}
      return {{ missing: false, calls }};
    }})()
    """)


# ------------------------------------------------------------------ what the drawing says


def drawn(studio):
    """How many node rows the view draws now: the frame's own `nodeCount`, the reader
    `liverows.positions` walks, so "drawn" means one thing here and in the live gate."""
    return studio.page.evaluate(f"{liverows.HOST}.view.frame().nodeCount")


def wait_grown(studio, want, seconds=GROW_S):
    """Poll the drawn count until it reaches `want`: the count, and how long that took."""
    began = time.monotonic()
    count = drawn(studio)
    while count < want and time.monotonic() - began < seconds:
        studio.page.watch_workers()
        time.sleep(POLL_S)
        count = drawn(studio)
    return count, time.monotonic() - began


def drive(studio):
    """The forces on, then the burst, then the wait the size row is measured against."""
    host = liverows.HOST
    studio.page.evaluate(f"{host}.studio.dispatch('forces.animate', {{ on: true }})")
    base = studio.page.evaluate(f"{host}.studio.store.get().meta.nodeCount")
    pushed = push(studio)
    want = base + CALLS * BATCH
    count, took = wait_grown(studio, want)
    return {"base": base, "want": want, "pushed": pushed, "drawn": count, "waited": took}


# ------------------------------------------------------------------ the four rows


def not_run(name, expectation):
    """A row that drives the verb cannot be measured without it; that is not a pass."""
    return verdict.row(name, expectation, "nothing was applied", False, MISSING)


def row_drawn(driven):
    """The drawing reaches the size the burst promised: base plus every node of every batch."""
    expectation = (f"the drawn node count reaches {driven['want']} within {GROW_S:.0f}s of the "
                   f"last of {CALLS} batches of {BATCH} nodes")
    if driven["pushed"].get("missing"):
        return not_run("deltas-drawn", expectation)
    calls = driven["pushed"]["calls"]
    applied = sum(one["answer"]["applied"] for one in calls if one["ok"])
    refused = [f"{one['name']}: {one['detail']}" for one in calls if not one["ok"]]
    measured = (f"{driven['drawn']} drawn of {driven['want']} wanted, {applied} nodes applied"
                + (f", refused as {'; '.join(refused)}" if refused else "")
                + f", after {driven['waited']:.2f}s")
    return verdict.row("deltas-drawn", expectation, measured, driven["drawn"] >= driven["want"])


def row_moving(studio, driven):
    """The layout keeps moving, over the graph the batches grew.

    The travel is the largest single-node euclidean distance over `liverows.SAMPLE_S`, from the
    same reader and interval the live gate's settle row uses, but not its L1. The grown drawing is
    the precondition: a dropped `grow` leaves the old graph still twitching, and a row measuring
    that would pass on a graph the deltas never reached — which is the negative control.
    """
    expectation = (f"over {liverows.SAMPLE_S}s some drawn node travels more than "
                   f"{liverows.MOVING_PX} world unit, in the graph the batches grew")
    if driven["pushed"].get("missing"):
        return not_run("deltas-moving", expectation)
    if driven["drawn"] <= driven["base"]:
        return verdict.row("deltas-moving", expectation,
                           f"the drawing never grew past {driven['base']} nodes, so nothing is "
                           "running over the nodes the batches added", False)
    first = liverows.positions(studio)
    if not first:
        return verdict.row("deltas-moving", expectation, "the view reports an empty frame", False)
    time.sleep(liverows.SAMPLE_S)
    second = liverows.positions(studio)
    if len(first) != len(second):
        return verdict.row("deltas-moving", expectation,
                           f"the drawn count changed between the samples, {len(first)} then "
                           f"{len(second)}, and a prefix of it is not the whole drawing", False)
    travel = max(apart(one, other) for one, other in zip(first, second))
    measured = (f"largest travel {travel:.3f} world units over {len(first)} drawn nodes "
                f"(threshold {liverows.MOVING_PX})")
    return verdict.row("deltas-moving", expectation, measured, travel > liverows.MOVING_PX)


def row_clean(page):
    """Nothing threw, nothing logged an error, and no failure banner is up.

    The three channels are the load-smoke gate's, through its own readers: an uncaught exception
    from the page or the motor worker, both halves of the console (`console.error` and the
    browser's own `Log`), and the overlay out of the shadow root. The refusal row's rejection is
    caught in the page, so a refused batch is not a fault here: a fault is one the studio did not
    catch on the way past.
    """
    expectation = ("no page or motor-worker exception, no console or Log error, and no failure "
                   "banner while the batches are applied")
    page.call("Runtime.evaluate", {"expression": "1"})  # drain what is still in the socket
    thrown = smokerows.events(page, "Runtime.exceptionThrown")
    called = smokerows.faults(page, "Runtime.consoleAPICalled", ("type",), ("error", "assert"))
    logged = smokerows.faults(page, "Log.entryAdded", ("entry", "level"), ("error",))
    banner = page.evaluate(f"""
    (() => {{
      const alert = {liverows.ROOT}.querySelector('.gs-alert');
      return alert === null ? null : alert.textContent;
    }})()
    """)
    problems = [*thrown, *called, *logged]
    if banner is not None:
        problems.append(f"banner: {smokerows.short(banner)}")
    return verdict.row("deltas-clean", expectation,
                       "none" if not problems else "; ".join(problems), not problems)


def row_refused(studio):
    """A batch naming an id the graph already holds is refused whole, and draws nothing.

    The id comes out of the page's own `meta.ids`, so it is one the graph really has, and the
    count either side of the call is read inside the same evaluate: nothing else happens between.
    """
    expectation = ("a batch naming an id the graph already has rejects, and the drawn node count "
                   "does not grow across that one call")
    if studio.page.evaluate(f"typeof {liverows.HOST}.applyDeltas") != "function":
        return not_run("deltas-refused", expectation)
    held = studio.page.evaluate(f"{liverows.HOST}.studio.store.get().meta.ids[0]")
    batch = batch_json("delta-refused", 3, first_id=held)
    outcome = studio.page.evaluate(f"""
    (async () => {{
      const el = {liverows.HOST};
      const drawn = () => el.view.frame().nodeCount;
      const before = drawn();
      let answer = null, rejected = false, name = null, detail = null;
      try {{ answer = await el.applyDeltas({batch}); }}
      catch (failure) {{ rejected = true; name = failure.name; detail = String(failure.message); }}
      return {{ rejected, name, detail, before, after: drawn(), answer }};
    }})()
    """)
    how = (f"rejected as {outcome['name']}: {smokerows.short(outcome['detail'])}"
           if outcome["rejected"] else f"resolved with {outcome['answer']}")
    measured = (f"{held} was already in the graph; it {how}; {outcome['before']} nodes drawn "
                f"before that call, {outcome['after']} after")
    return verdict.row("deltas-refused", expectation, measured,
                       outcome["rejected"] and outcome["after"] == outcome["before"])


def run_rows(page, studio, driven):
    """The four rows, in the order a host meets them, plus this gate's `name`/`detail` spellings.

    `nav.table` renders `row` and `measured`; the rows file and a reader of report.json ask for
    `name` and `detail`. Both are set from one row here, so the two spellings cannot drift.
    """
    rows = [row_drawn(driven), row_moving(studio, driven), row_clean(page), row_refused(studio)]
    for one in rows:
        one["name"], one["detail"] = one["row"], one["measured"]
    return rows


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
            driven = drive(studio)
            return {"label": args.out.name, "commit": args.commit, "break": args.broken,
                    "url": url, "browser": page.call("Browser.getVersion").get("product"),
                    "viewport": VIEWPORT, "deltas": driven,
                    "rows": run_rows(page, studio, driven)}
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
