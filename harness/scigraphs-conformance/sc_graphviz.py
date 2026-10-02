"""The Graphviz arm: the nine names SciGraphs reaches through `scigraphs_utils`, taken from
the engine itself.

**The engine runs, and then SciGraphs' own layer runs on top of what it produced.**
`_scigraphs_utils_graphviz_layout` (`yifan_hu.py:278-337`) never returns the engine's points:
lines `:318-325` centre them on their mean, divide by their largest extent and multiply by
`scale`. That layer lives inside `scigraphs_utils.graphviz_layout`, a C++ extension absent from
both oracle images, so this arm applies those five lines itself and the motor arm applies the
same five in Rust (`motor/gv_post.rs`). Both arms then emit what SciGraphs would, and what the
matrix compares is the layout rather than the convention.

**What is still not SciGraphs' answer is the engine's own text.** `gv_plain.parse_plain` reads
`-Tplain`, which writes inches at five decimals, and multiplies by 72 (`gv_plain.py:24`,
`:95-96`): so every reference coordinate is a multiple of `7.2e-4` points. That grid is this
arm's own doing and not SciGraphs' — `graphviz_layout(num_nodes, edges, engine=..., ...)`
(`yifan_hu.py:298-307`) is handed a node count and an edge list and returns an array, so it is
a layout call rather than a rendering, and a rendering is what rounds. Hence
`GRAPHVIZ_TWOPI`'s `max_gap` floors at 7.5e-5 rather than at zero, and that row's gap is about
the arm rather than about the scale.

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
from sc_names import GRAPHVIZ_ROWS, LAYOUT_SEED, SCALE, graphviz_version  # noqa: E402

#: `dims = min(3, raw.shape[1])` (`yifan_hu.py:315`) over an engine that writes two columns,
#: with `graphviz_dim` at its default `"2"` (`yifan_hu.py:314`). `YIFAN_HU` asks for `"2Z"`
#: (`yifan_hu.py:357`), which changes the z column at `:327-334` and not this number.
GRAPHVIZ_DIMS = 2


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
                "detail": "the engine's -Tplain points, then yifan_hu.py:318-325",
            })
            for point in zip(*_scigraphs_columns(points)):
                values.extend([float(point[0]), float(point[1]), 0.0])
    return values, report


def _scigraphs_columns(points):
    """`yifan_hu.py:318-325`, axis by axis, over the engine's points.

    **These five lines are the row, and they are transcribed rather than executed.** The
    oracle image this arm runs in is `debian:trixie-slim` plus a Graphviz build and no numpy
    at all (`docker/graphviz-oracle.Dockerfile`), so this is `numpy` written out by hand. The
    Rust arm (`crates/graph-cli/src/oracle_python/conformance/motor/gv_post.rs`) is the same
    function in another language and its test `the_sum_here_is_numpys_own_order` holds it to
    numpy 2.3.3's own mean at six lengths covering every branch of the reduction, so the two
    arms of this row are held to the same summation order by a number, not by agreement.

    The extent is taken **after** the centring, which is `raw_range[:dims].max()` at `:320`
    and not the range of the raw column: the two are equal in exact arithmetic and differ in
    `f64`, and only the first is what SciGraphs computes. An axis past `GRAPHVIZ_DIMS` is not
    written at all, which is `positions = np.zeros((num_nodes, 3))` at `:323` and is why a 2D
    engine's z comes out `0.0`.
    """
    columns = [list(axis) for axis in zip(*points)][:GRAPHVIZ_DIMS]
    centred = []
    for column in columns:
        mean = _numpy_pairwise_sum(column) / len(column)
        centred.append([value - mean for value in column])
    extent = max(max(column) - min(column) for column in centred)
    divisor = extent if extent > 0 else 1.0
    return [[value / divisor * SCALE for value in column] for column in centred]


def _numpy_pairwise_sum(values):
    """numpy's own summation order for `float64`, in plain Python.

    **A left-to-right `sum()` is a different reduction, and here it is worth about `4e-14`.** The
    mean moves by several ULPs, the centring moves with it, and after the rescale that lands
    around `4e-14` on a 77-node layout — twelve orders of magnitude under the `7.2e-4` grid the
    `-Tplain` text puts this arm's own coordinates on. It is written this way because a
    convention that depends on a summation order is not a convention, and because the Rust arm
    is held to numpy at six lengths: the two transcriptions have to be the same reduction for
    that test to mean anything about this one.

    numpy's order: a plain sum below eight elements, eight accumulators up to
    `PW_BLOCKSIZE = 128`, and above that a split in two at an eight-aligned midpoint, with the
    eight combined as `((r0+r1)+(r2+r3)) + ((r4+r5)+(r6+r7))`.
    """
    block = 128
    count = len(values)
    if count < 8:
        total = 0.0
        for value in values:
            total += value
        return total
    if count > block:
        split = count // 2 // 8 * 8
        return _numpy_pairwise_sum(values[:split]) + _numpy_pairwise_sum(values[split:])
    acc = list(values[:8])
    index = 8
    while index < count - count % 8:
        for offset in range(8):
            acc[offset] += values[index + offset]
        index += 8
    folded = ((acc[0] + acc[1]) + (acc[2] + acc[3])) + ((acc[4] + acc[5]) + (acc[6] + acc[7]))
    for value in values[index:]:
        folded += value
    return folded


def _summary(report):
    ok = sum(1 for entry in report if entry["status"] == "ok")
    return "%d/%d fixtures, -Gstart=%d, SciGraphs' convention" % (ok, len(report),
                                                                  LAYOUT_SEED)


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
