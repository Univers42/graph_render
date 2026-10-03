#!/usr/bin/env python3
"""The `layout.twopi` differential: our closed-form radial layout against Graphviz's own.

Run in the `ge-graphviz-oracle` image (Graphviz 16.1.0, no network):

  graph-cli emit-twopi-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
      python3 harness/oracle-twopi.py target/twopi-fixtures target/gv-twopi
  graph-cli oracle-twopi

One file, two arms: for each fixture it writes a DOT graph, runs
`twopi -Tplain -Gstart=<GM_GV_START or 1>`, and compares Graphviz's node positions with ours.
The DOT writer
and the `-Tplain` reader are **imported from `harness/gv_plain.py`**, the one module every
Graphviz arm shares, not copied, so the determinism evidence the ADR records (`cmp` over two
runs of that plumbing) stays attached to the code that runs here. The comparison is against
Graphviz, never against a second run of ours. Graphviz's own answer per seed is written to
`target/gv-twopi/graphviz-twopi.jsonl`, which is the file the ADR's determinism check
`cmp`s. The result's `oracle` string is interpolated from the Graphviz version `dot -V`
reports and the `-Gstart` the engine was actually run at, so it names the command that ran.

**The metric** is the largest absolute node-coordinate difference in points, after both arms
are rescaled onto the same bounding box: for each seed, Graphviz's own node-centre bounding
box is the target and both arms are mapped onto it with one uniform scale.

Why rescale at all: Graphviz translates its drawing so the bounding box's lower-left corner
is the origin and our layout puts its own centre there, so the arms differ by a translation
and a scale that say nothing about the layout. Mapping both onto one box removes exactly
those two and nothing else — ring radii, angles and relative positions all survive.

Why one uniform scale and not one per axis: a per-axis map would let an aspect-ratio error
and a shape error both arrive as an unscaled shape difference, indistinguishable. A single
`max` scale keeps the aspect honest, so a drawing that is right but stretched is a *larger*
gap. It also survives the degenerate axes a radial layout produces — a 2-node graph is a
straight line, so one of its two spans is zero — where a per-axis divide would not.

**The closed cases are compared too, and exactly.** Six graphs whose layout has an
analytically determined answer — one node, two nodes, a 3-path, a 4-cycle, a 5-star and a
6-branch — have their layout derived from Graphviz's `lib/twopigen/circle.c` and restated in
`crates/graph-core/src/layout/radial/twopi/tests.rs`, where **our** arm is pinned node by
node against them. This file pins **Graphviz's** arm on the same closed answers, so neither
arm is graded against a tolerance the other is exempt from.

The comparison is **byte for byte**, on the strings `-Tplain` itself prints. Graphviz
translates its drawing so the *node boxes* start at the origin, so the exact offset from the
closed answer's frame to Graphviz's is half a default node outside the node-centre bounding
box. Adding that to the closed answer's exact coordinates and printing at five significant
digits reproduces the oracle's own node lines when the two agree, so the comparison is exact
and not a tolerance. It also settles the precision the rescale cannot: five digits is the
resolution the oracle carries, and a structural error (the wrong centre, a ring one step
out, a subtree share off by one leaf) moves a digit rather than a fraction of one.

Ponytail: the Graphviz plumbing used to be imported from `harness/oracle-graphviz.py` by
path, with `importlib`, because that filename carries a hyphen and is not importable by name.
It now comes from `harness/gv_plain.py`, the one module every Graphviz arm shares, which is
where those functions actually live — so the path import is gone, along with the
`sys.dont_write_bytecode` dance it needed to keep a `__pycache__` out of the fingerprinted
`harness/` tree (`crates/graph-cli/src/fingerprint.rs:21,37-47`). The one name still read by
path is `CLOSED_CASES`, which `gv_frames.py` reaches here for; a copy of those six graphs
would be a second definition of what the closed comparison is about.
"""

import json
import os
import sys
import tempfile

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oracle_common import finite, read_manifest, require_cases, require_seeds

# The six closed cases and their answers are tables, and they were half this file by line
# count, so they moved to `harness/twopi_closed.py`. The four names below are re-exported
# here because `gv_frames.py` reads `CLOSED_CASES` from **this** file by path — its
# filename is not importable by name — and a second definition of those graphs would be a
# second thing the closed comparison could disagree with.
from gv_plain import START_SEED, graphviz_version, parse_plain, run_engine, write_dot

from twopi_closed import (  # noqa: F401  (CLOSED_CASES is read by gv_frames, by path)
    CLOSED_ANSWERS,
    CLOSED_CASES,
    DIGITS,
    NODE_SIZE_INCH,
    POINTS_PER_INCH,
)


def engine_version():
    """The Graphviz the engine actually ran, asked of the binary rather than asserted."""
    return graphviz_version()


def engine_start():
    """The `-Gstart` the engine ran at: the value `run_engine` used, not a literal.

    `gv_plain.START_SEED` is `GM_GV_START`'s default and `run_engine` resolves it inside its
    body, so this is the seed in every command below — `twopi` is measured INERT for it
    (`docs/measurements/p13-gv1.patchwork.md`), which is exactly why the string has to be
    interpolated: a hard-coded `-Gstart=1` beside a `GM_GV_START=7` run recorded a command
    that was not the one executed.
    """
    return START_SEED


def engine_points(tmp, name, count, edges):
    """`twopi -Tplain -Gstart=<seed>` over one DOT graph, as dense-indexed points.

    The DOT writer and the `-Tplain` reader are imported from `gv_plain` — the module that
    owns them, shared with every other Graphviz arm — rather than reached through the driver
    by path. The path import of `oracle-graphviz.py` is kept for the closed-case answers and
    the version/seed the driver resolves, and `gv_plain` is where the plumbing actually
    lives, so a caller that needs the plumbing reads it there.
    """
    source = [a for a, _ in edges]
    target = [b for _, b in edges]
    dot = os.path.join(tmp, f"{name}.dot")
    write_dot(dot, count, source, target)
    _, nodes = parse_plain(run_engine("twopi", dot), count)
    return [tuple(nodes[f"n{i}"]) for i in range(count)]


def ours_of(record):
    """The fixture's own coordinates, as points in dense-index order."""
    column = record["twopi"]
    return [(column["x"][i], column["y"][i]) for i in range(record["n"])]


def edges_of(record):
    """The fixture's dense edge endpoints, in its own column order."""
    return list(zip(record["source"], record["target"]))


def bbox_of(points):
    """A point cloud's bounding box: `((min_x, min_y), (width, height))`."""
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    return (min(xs), min(ys)), (max(xs) - min(xs), max(ys) - min(ys))


def rescale(points, box):
    """`points` onto `box`, one uniform scale from the larger axis's span."""
    (min_x, min_y), (width, height) = box
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    span = max(max(xs) - min(xs), max(ys) - min(ys))
    if span <= 0.0:
        return [(p[0] - min_x, p[1] - min_y) for p in points]
    scale = max(width, height) / span
    low_x, low_y = min(xs), min(ys)
    return [((x - low_x) * scale, (y - low_y) * scale) for x, y in zip(xs, ys)]


def gap(ours, theirs):
    """The largest absolute coordinate difference in points, on one shared box."""
    box = bbox_of(theirs)
    mine, other = rescale(ours, box), rescale(theirs, box)
    return max(max(abs(a[0] - b[0]), abs(a[1] - b[1])) for a, b in zip(mine, other))


def rendered(points):
    """The closed answer as the node lines `-Tplain` would print for it.

    Graphviz translates its drawing so the *node boxes* start at the origin, so half a node
    outside the node-centre bounding box is the whole of the offset — exact, and derived
    from the answer rather than from the oracle, which is what keeps this a comparison
    against the closed answer rather than against the oracle's rounded text.
    """
    (low_x, low_y), _ = bbox_of(points)
    off_x = NODE_SIZE_INCH[0] / 2.0 - low_x / POINTS_PER_INCH
    off_y = NODE_SIZE_INCH[1] / 2.0 - low_y / POINTS_PER_INCH
    return " ".join(
        f"{x / POINTS_PER_INCH + off_x:.{DIGITS}g} {y / POINTS_PER_INCH + off_y:.{DIGITS}g}"
        for x, y in points
    )


def printed_nodes(tmp, name, count, edges):
    """Graphviz's own node coordinates, as the two strings `-Tplain` printed for each.

    The text, not the parsed float: the comparison is byte for byte, and re-printing a
    parsed value would grade our `f64` against the oracle's rounding instead of against the
    oracle's arithmetic.
    """
    dot = os.path.join(tmp, f"{name}.dot")
    write_dot(dot, count, [a for a, _ in edges], [b for _, b in edges])
    rows = {}
    for line in run_engine("twopi", dot).splitlines():
        parts = line.split()
        if len(parts) >= 4 and parts[0] == "node":
            rows[parts[1]] = (parts[2], parts[3])
    return [rows[f"n{i}"] for i in range(count)]


def closed_case(tmp, name):
    """One closed case: Graphviz's arm rendered against the closed answer, exactly."""
    edges = CLOSED_CASES[name]
    count = 1 + max((max(edge) for edge in edges), default=0)
    theirs = printed_nodes(tmp, f"closed-{name}", count, edges)
    want = rendered(CLOSED_ANSWERS[name])
    got = " ".join(f"{x} {y}" for x, y in theirs)
    return {"nodes": count, "exact": want == got, "want": want, "got": got}


def sweep(tmp, fixtures):
    """Every fixture through the engine: the worst gap in points, and the raw output.

    The raw output is written to `<graphviz-out-dir>/graphviz-twopi.jsonl` so the ADR's
    determinism evidence is reproducible by running this twice and `cmp`-ing that file,
    and so a reviewer can read what the oracle actually said per seed.
    """
    worst = 0.0
    theirs = []
    for record in fixtures:
        points = engine_points(tmp, f"g{record['seed']}", record["n"], edges_of(record))
        worst = max(worst, finite(gap(ours_of(record), points), "twopi gap"))
        theirs.append({"seed": record["seed"], "n": record["n"], "points": points})
    return worst, theirs


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: oracle-twopi.py <fixtures-dir> <graphviz-out-dir>")
    fixtures_dir, out_dir = sys.argv[1], sys.argv[2]
    manifest, digest = read_manifest(fixtures_dir, "twopi")
    fixtures = read_lines(os.path.join(fixtures_dir, "twopi.jsonl"))
    require_seeds(manifest, fixtures, "twopi")
    with tempfile.TemporaryDirectory() as tmp:
        worst, theirs = sweep(tmp, fixtures)
        closed = {name: closed_case(tmp, name) for name in CLOSED_CASES}
    require_cases({"twopi": {"cases": len(theirs)}}, ("twopi",), "twopi")
    os.makedirs(out_dir, exist_ok=True)
    write_lines(os.path.join(out_dir, "graphviz-twopi.jsonl"), theirs)
    exact = all(row["exact"] for row in closed.values())
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": digest,
        "oracle": (
            f"Graphviz {engine_version()} twopi -Tplain "
            f"-Gstart={engine_start()}"
        ),
        "layouts": {"twopi": {"cases": len(theirs), "worst": worst}},
        "closed": closed,
        "closed_exact": exact,
    }
    with open(os.path.join(fixtures_dir, "twopi-result.json"), "w") as out:
        json.dump(result, out, indent=1)
    print(
        f"twopi: {len(theirs)} seeds, worst {worst:.3e} points; "
        f"closed {len(closed)} exact: {exact}"
    )
    return 0 if exact else 1


def read_lines(path):
    with open(path) as handle:
        return [json.loads(line) for line in handle]


def write_lines(path, rows):
    with open(path, "w") as handle:
        for row in rows:
            handle.write(json.dumps(row) + "\n")


if __name__ == "__main__":
    sys.exit(main())
