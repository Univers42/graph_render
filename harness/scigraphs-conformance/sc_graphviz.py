"""The Graphviz arm: the nine names SciGraphs reaches through `scigraphs_utils`, taken from
the engine itself.

**The engine's points, in points, with no SciGraphs layer on top** — because that layer is
`scigraphs_utils.graphviz_layout`, absent from both oracle images, and inventing a stand-in
for it would be measuring our guess rather than the engine. So this arm is the engine, and
every row that uses it carries a convention gap saying the `scale = 5.0` multiply and the
spectral z SciGraphs adds are not in these numbers.

The DOT is written undirected for all nine, which is right for eight of them and wrong for
`dot`: SciGraphs builds `dot` directed (`yifan_hu.py:302`). That is recorded as
`GRAPHVIZ_DOT`'s gap rather than fixed here, because a second DOT writer in this arm is a
second thing that can disagree with `gv_plain`.
"""

import json
import os
import sys
import tempfile

# `harness/` is inside `FINGERPRINTED` (`crates/graph-cli/src/fingerprint.rs:21`), so
# importing a module by name would write `harness/__pycache__/*.pyc` and move the fingerprint
# for as long as it exists — which is exactly the disagreement `oracle-graphviz.py:57-62`
# sets the flag for. Set before the child modules are imported.
sys.dont_write_bytecode = True

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from gv_plain import engine_points  # noqa: E402

from sc_fixture import FixtureError, write_f64  # noqa: E402
from sc_names import GRAPHVIZ_ROWS, LAYOUT_SEED, graphviz_version  # noqa: E402


def run(directory, fixtures):
    """Every Graphviz row, writing `ref/<NAME>.f64` and `ref/<NAME>.json` beside the others."""
    out = os.path.join(directory, "ref")
    os.makedirs(out, exist_ok=True)
    for name, engine in GRAPHVIZ_ROWS:
        values, report = run_name(out, name, engine, fixtures)
        write_f64(os.path.join(out, "%s.f64" % name), values)
        _record(out, name, engine, report)
        print("  %-20s %s" % (name, _summary(report)), flush=True)
    return len(GRAPHVIZ_ROWS)


def run_name(out, name, engine, fixtures):
    """One engine over every fixture, at `LAYOUT_SEED`.

    The seed is the layout seed, not `gv_plain`'s own default of 1, because SciGraphs hands
    the engine `start = get_layout_seed()` (`yifan_hu.py:229`, `:248`) and a row that compared
    the engine at 1 against the motor at the layout seed would be measuring two seeds.
    """
    values = []
    report = []
    with tempfile.TemporaryDirectory() as scratch:
        for fixture in fixtures:
            try:
                points = engine_points(
                    engine, scratch, "%s-%s" % (name, fixture.name),
                    fixture.n, fixture.edges(), start=LAYOUT_SEED,
                )
            except SystemExit as failure:
                report.append({
                    "fixture": fixture.name,
                    "status": "not run",
                    "detail": "engine refused: %s" % (failure,),
                })
                continue
            report.append({
                "fixture": fixture.name,
                "status": "ok",
                "layout_substituted": None,
                "detail": "the engine's own -Tplain points, in points, z = 0",
            })
            for x, y in points:
                values.extend([float(x), float(y), 0.0])
    return values, report


def _summary(report):
    ok = sum(1 for entry in report if entry["status"] == "ok")
    return "%d/%d fixtures, -Gstart=%d" % (ok, len(report), LAYOUT_SEED)


def _record(out, name, engine, report):
    """`ref/<NAME>.json`, in the same shape the SciGraphs arm writes."""
    body = {
        "name": name,
        "arm": "ge-graphviz-oracle:%s" % engine,
        "engine": engine,
        "engine_version": graphviz_version(engine),
        "start_seed": LAYOUT_SEED,
        "libraries": {"graphviz": graphviz_version(engine)},
        "layout_substituted": None,
        "layout_substitutions": [],
        "status": "ok" if all(entry["status"] == "ok" for entry in report) else "partial",
        "fixtures": report,
    }
    with open(os.path.join(out, "%s.json" % name), "w") as handle:
        json.dump(body, handle, indent=1, sort_keys=True)
        handle.write("\n")


def check_fixtures(directory, fixtures):
    """The engine has to be on the path, or the nine rows are nine `not run` cells.

    Failing here is better than writing nine empty files: a silent nine-row hole reads like a
    measurement, and this arm's whole job is the opposite.
    """
    for _, engine in GRAPHVIZ_ROWS:
        try:
            graphviz_version(engine)
        except FileNotFoundError as missing:
            raise FixtureError(
                "%s is not installed; the Graphviz arm cannot run in this image (%s)"
                % (engine, missing)
            ) from missing
    return os.path.isdir(directory)
