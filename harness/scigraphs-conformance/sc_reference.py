"""The reference arm: `apply_graph_layout` itself, for all 23 names the oracle image hosts.

**This is the reference function, not a restatement of it.** `SciGraphs/` is on the path and
`apply_graph_layout` is imported and called with the dispatcher's own defaults
(`iterations=50`, `scale=5.0`), so a row's reference is SciGraphs' answer and nothing else.
Whatever it writes into `obj["node_positions"]` — including a z column for a planar layout,
and including the fallback a missing library produces — is what the metrics get.

**A name that fails is a cell with a reason, never a skipped row.** `apply_graph_layout`
returns `False` and writes nothing on an exception (`dispatcher.py:167-175`), so a row whose
library is absent or whose graph it refuses carries `layout_substituted` or an error string
into `ref/<NAME>.json`, and the metrics turn that into `not run: <reason>`.
"""

import io
import json
import os
import sys
import traceback
from contextlib import redirect_stdout

from sc_fixture import FixtureError, write_f64
from sc_names import ITERATIONS, SCALE, SCIGRAPHS_NAMES, libraries

# `SciGraphs/core` is the package root (`harness/oracle-basic-3d.py:62` does the same).
sys.path.insert(0, os.path.join("SciGraphs", "core"))


def _load_dispatcher():
    """`apply_graph_layout` itself, imported through the package so its own imports run."""
    from scigraphs_core.mesh.layouts.dispatcher import apply_graph_layout

    return apply_graph_layout


def run_name(directory, name, fixtures):
    """One name over every fixture: the coordinates, and what the dispatcher said about them.

    SciGraphs is a chatty library — it prints a progress line and a timing per call — so its
    output is captured rather than mixed into the arm's own stdout, which would otherwise
    interleave the timing of this run into the file a reader diffs.
    """
    apply_graph_layout = _load_dispatcher()
    values = []
    report = []
    for fixture in fixtures:
        obj = fixture.scigraphs_object()
        transcript = io.StringIO()
        error = None
        try:
            with redirect_stdout(transcript):
                ok = apply_graph_layout(
                    obj, name, iterations=ITERATIONS, scale=SCALE,
                    edge_pairs=fixture.edge_pairs(),
                )
        except Exception as failure:  # noqa: BLE001 - any failure is a cell, not a crash
            ok = False
            error = "%s: %s" % (type(failure).__name__, failure)
            traceback.print_exc(file=sys.stderr)
        report.append(_entry(fixture, obj, ok, error))
        if ok and "node_positions" in obj:
            positions = obj["node_positions"]
            if len(positions) != 3 * fixture.n:
                report[-1]["status"] = "error"
                report[-1]["detail"] = "%d positions for %d nodes" % (
                    len(positions), fixture.n,
                )
                continue
            values.extend(float(value) for value in positions)
        elif ok:
            report[-1]["status"] = "error"
            report[-1]["detail"] = "apply_graph_layout wrote no node_positions"
    return values, report


def _entry(fixture, obj, ok, error):
    """One fixture's outcome: reached, substituted, or not reached — and why."""
    entry = {"fixture": fixture.name, "layout_substituted": obj.get("layout_substituted")}
    if error:
        entry.update(status="error", detail=error)
    elif not ok:
        entry.update(status="not run", detail="apply_graph_layout returned False")
    elif "node_positions" not in obj:
        entry.update(status="not run", detail="no node_positions")
    else:
        entry["status"] = "ok"
    return entry


def run(directory, fixtures):
    """Every SciGraphs-hosted name, writing `ref/<NAME>.f64` and `ref/<NAME>.json`."""
    out = os.path.join(directory, "ref")
    os.makedirs(out, exist_ok=True)
    versions = libraries()
    reached = 0
    for name in SCIGRAPHS_NAMES:
        values, report = run_name(directory, name, fixtures)
        write_f64(os.path.join(out, "%s.f64" % name), values)
        _record(out, name, "ge-python-oracle:scigraphs", versions, report)
        reached += 1
        print("  %-20s %s" % (name, _summary(report)), flush=True)
    return reached


def _summary(report):
    ok = sum(1 for entry in report if entry["status"] == "ok")
    substituted = {
        entry["layout_substituted"] for entry in report
        if entry.get("layout_substituted")
    }
    tail = ", substituted: %s" % ", ".join(sorted(substituted)) if substituted else ""
    return "%d/%d fixtures%s" % (ok, len(report), tail)


def _record(out, name, arm, versions, report):
    """`ref/<NAME>.json`: which arm, which libraries, and every fixture's outcome."""
    substituted = sorted({
        entry["layout_substituted"] for entry in report
        if entry.get("layout_substituted")
    })
    body = {
        "name": name,
        "arm": arm,
        "libraries": versions,
        "layout_substituted": substituted[0] if len(substituted) == 1 else (
            substituted or None
        ),
        "layout_substitutions": substituted,
        "status": "ok" if all(e["status"] == "ok" for e in report) else "partial",
        "fixtures": report,
    }
    with open(os.path.join(out, "%s.json" % name), "w") as handle:
        json.dump(body, handle, indent=1, sort_keys=True)
        handle.write("\n")


def check_names(motor_rows):
    """This arm's 32 names and the motor arm's 32 names are the same 32.

    The two lists are written on two sides of a boundary neither can see, so they are checked
    against each other rather than trusted: a name added to one and not the other would give
    the matrix a row with no reference or a reference with no row.
    """
    from sc_names import ALL_NAMES, GRAPHVIZ_ROWS, SCIGRAPHS_NAMES

    mine = sorted(SCIGRAPHS_NAMES + [name for name, _ in GRAPHVIZ_ROWS])
    if mine != sorted(ALL_NAMES):
        raise FixtureError(
            "sc_names: %d names in the two arms and %d in ALL_NAMES" % (len(mine), len(ALL_NAMES))
        )
    theirs = sorted(row["name"] for row in motor_rows)
    if theirs != sorted(ALL_NAMES):
        raise FixtureError(
            "motor.jsonl has %d rows and they are not this matrix's 32 names" % len(theirs)
        )
    return True
