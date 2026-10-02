"""The Graphviz arm: the nine names SciGraphs reaches through `scigraphs_utils`, taken from
the engine itself.

**The engine runs, and then SciGraphs' own layer runs on top of what it produced.**
`_scigraphs_utils_graphviz_layout` (`yifan_hu.py:278-337`) never returns the engine's points:
lines `:318-325` centre them on their mean, divide by their largest extent and multiply by
`scale`. That layer lives inside `scigraphs_utils.graphviz_layout`, a C++ extension absent from
both oracle images, so this arm applies those five lines itself and the motor arm applies the
same five in Rust (`motor/gv_post.rs`). Both arms then emit what SciGraphs would, and what the
matrix compares is the layout rather than the convention.

**The engine's own text is not what this arm reads.** `-Tplain` prints coordinates in inches
through `printdouble`, which is `agxbprint(&buf, "%.5g", v)` (`lib/common/output.c:66-71`,
`printpoint` at `:76-79`): five *significant* digits, so a coordinate in [1, 10) in sits on a
step of `1e-4` in = `7.2e-3` points and one above 10 in on `1e-3` in. Reading that text puts
the reference on the graph's own rounding grid. So the points come from `gv_exact`
(`gv_exact.c`/`gv_exact.py`), which links libgvc in this image, runs `gvLayout` and prints
`ND_coord(n)` with `%a` — the same translated points `-Tplain` rounds, unrounded.
`GRAPHVIZ_TWOPI`'s `max_gap` was floored at 7.5e-5 by that grid; this arm's own doctest
(`gv_exact.py`) measures the residue it removes at up to 3.6e-2 pt for a 77-node ring, and
`2e-4` pt for twopi, which is the same layout with the digits it started with.

**What is still not SciGraphs' answer is `scigraphs_utils` itself.**
`graphviz_layout(num_nodes, edges, engine=..., ...)` (`yifan_hu.py:298-307`) is handed a node
count and an edge list and returns an array, so on this reading it is a layout call rather than
a rendering, and a rendering is what rounds. **That is an inference, not a measurement:** the
extension's source is not on disk, only the `scigraphs-utils==0.2.0` pin
(`SciGraphs/constraints/linux-x64.txt:21`), so what it does to the coordinates between
`gvLayout` and the array is unverified — as is the seed, which this arm supplies as the graph
attribute `start` the way `-Gstart` does (`lib/common/input.c:281-286`, `:178-192`).

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

from gv_exact import exact_points  # noqa: E402

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
                points = exact_points(
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
                "detail": "ND_coord(n) via gv_exact.c, then yifan_hu.py:318-325",
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
    function in another language and its test `the_mean_of_an_n_by_2_array_is_the_left_to_right_sum`
    holds it to numpy's own `mean(axis=0)` on `(n, 2)` C-contiguous arrays at fourteen lengths,
    so the two arms of this row are held to the same summation order by a number, not by
    agreement.

    The extent is taken **after** the centring, which is `raw_range[:dims].max()` at `:320`
    and not the range of the raw column: the two are equal in exact arithmetic and differ in
    `f64`, and only the first is what SciGraphs computes. An axis past `GRAPHVIZ_DIMS` is not
    written at all, which is `positions = np.zeros((num_nodes, 3))` at `:323` and is why a 2D
    engine's z comes out `0.0`.
    """
    columns = [list(axis) for axis in zip(*points)][:GRAPHVIZ_DIMS]
    centred = []
    for column in columns:
        mean = _numpy_mean(column)
        centred.append([value - mean for value in column])
    extent = max(max(column) - min(column) for column in centred)
    divisor = extent if extent > 0 else 1.0
    return [[value / divisor * SCALE for value in column] for column in centred]


def _numpy_mean(values):
    """`values.mean()` for one column of an `(n, 2)` C-contiguous array: a plain left-to-right
    sum and then one division by the length.

    Ponytail: **left to right, not numpy's pairwise sum**, and the reason is that numpy's
    pairwise reduction only runs along the contiguous axis. `raw` is `(n, 3)` C-contiguous, so
    `raw.mean(axis=0)` (`yifan_hu.py:318`) walks axis 0 — the strided one — as a flat sequence,
    and the eight accumulators and the split above `PW_BLOCKSIZE` never run. Measured in
    `ge-python-oracle` on `(n, 2)` C-contiguous arrays at n = 1, 2, 3, 5, 8, 9, 16, 17, 33, 64,
    127, 128, 129, 300: left to right equals `mean(axis=0)` at every length and on both columns,
    while the pairwise sum of the same values differs at every n >= 8 on column 0. The Rust arm
    is the same function in another language and
    `the_mean_of_an_n_by_2_array_is_the_left_to_right_sum` pins it to those hex values. What it
    gets wrong: a Fortran-ordered `raw` would restore the pairwise sum, and
    `scigraphs_utils.graphviz_layout` is a C++ extension with no source on disk to read its
    allocation from — the escape hatch is to take this from the array's flags rather than its
    shape, which nothing here can do.
    """
    total = 0.0
    for value in values:
        total += value
    return total / len(values)


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
