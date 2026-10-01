#!/usr/bin/env python3
"""The `layout.twopi` differential: our closed-form radial layout against Graphviz's own.

Run in the `ge-graphviz-oracle` image (Graphviz 16.1.0, no network):

  graph-cli emit-twopi-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
      python3 harness/oracle-twopi.py target/twopi-fixtures target/gv-twopi
  graph-cli oracle-twopi

One file, two arms: for each fixture it writes a DOT graph, runs
`twopi -Tplain -Gstart=1`, and compares Graphviz's node positions with ours. The DOT writer
and the `-Tplain` reader are **imported from `harness/oracle-graphviz.py`**, not copied, so
the determinism evidence the ADR records (`cmp` over two runs of that plumbing) stays
attached to the code that runs here. The comparison is against Graphviz, never against a
second run of ours. Graphviz's own answer per seed is written to
`target/gv-twopi/graphviz-twopi.jsonl`, which is the file the ADR's determinism check
`cmp`s.

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

Ponytail: `harness/oracle-graphviz.py` is imported by path with `importlib` because its
filename carries a hyphen and is not importable by name. That is uglier than a relative
`from oracle_graphviz import ...`, and it is the price of not copying the DOT writer: a copy
would be a second definition of how the oracle is invoked, and the `cmp` in the ADR proves
determinism of *that* one. The import runs with `sys.dont_write_bytecode` set, so it leaves no
`__pycache__` in the fingerprinted `harness/` tree (see `load_oracle`).
"""

import importlib.util
import json
import math
import os
import sys
import tempfile

# The plain format reports inches; both of this file's arms are in points.
POINTS_PER_INCH = 72.0

# The plain format prints five significant digits, so the closed-case rendering does too:
# that is the resolution the oracle carries, and comparing at any finer one would grade our
# `f64` against its rounded text.
DIGITS = 5

# Graphviz's default node size in inches, which is what makes the bounding box's lower-left
# corner half a node outside the node-centre bounding box. The fixtures set no `width`,
# `height` or `fixedsize`, so this is the value `-Tplain` used.
NODE_SIZE_INCH = (0.75, 0.5)


def star_angles():
    """The four leaf angles of a star on Graphviz's defaults: 45 + k*90 degrees."""
    return [math.pi / 4 + k * math.pi / 2 for k in range(4)]


# The closed cases, in the shape graph-core's `probe::graph` builds them: an edge list in
# creation order, which is the order the sibling angle sweep depends on.
CLOSED_CASES = {
    "one-node": [],
    "two-nodes": [(0, 1)],
    "three-path": [(0, 1), (1, 2)],
    "four-cycle": [(0, 1), (1, 2), (2, 3), (3, 0)],
    "five-star": [(0, 1), (0, 2), (0, 3), (0, 4)],
    "six-branch": [(0, 1), (0, 2), (0, 3), (2, 4), (4, 5)],
}

# The closed answers, in points, node by node, derived from `lib/twopigen/circle.c`: the
# centre at the origin, ring r at radius 72*r, and each subtree's share of 2*PI in
# proportion to the leaves below it. The translation onto the drawing's lower-left corner is
# the one degree of freedom `-Tplain` cannot show, so every case is compared after each arm
# has had it applied — see `rendered`.
CLOSED_ANSWERS = {
    "one-node": [(0.0, 0.0)],
    "two-nodes": [(0.0, 0.0), (-72.0, 0.0)],
    "three-path": [(0.0, -72.0), (0.0, 0.0), (0.0, 72.0)],
    "four-cycle": [(0.0, 0.0), (0.0, 72.0), (0.0, 144.0), (0.0, -72.0)],
    "five-star": [(0.0, 0.0)]
    + [(72.0 * math.cos(a), 72.0 * math.sin(a)) for a in star_angles()],
    "six-branch": [
        (-36.0, -36.0 * math.sqrt(3.0)),
        (-144.0, 0.0),
        (0.0, 0.0),
        (72.0, -72.0 * math.sqrt(3.0)),
        (36.0, 36.0 * math.sqrt(3.0)),
        (72.0, 72.0 * math.sqrt(3.0)),
    ],
}


def load_oracle(here):
    """`harness/oracle-graphviz.py`, by path: its filename is not importable by name."""
    path = os.path.join(here, "oracle-graphviz.py")
    # `harness/` is inside `FINGERPRINTED` (`crates/graph-cli/src/fingerprint.rs:21`), and
    # importing a module by path makes CPython write `harness/__pycache__/*.pyc` — a
    # transient file inside a fingerprinted tree, which moves the fingerprint for as long
    # as it exists. That would make `emit` (fingerprint without the `.pyc`) and `oracle-twopi`
    # (fingerprint with it) disagree on a clean checkout, and the check would refuse a run
    # that was in fact the right one. `fingerprint.rs:37-47` requires every transient path
    # to lie outside the set, so the bytecode is never written.
    previous = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec = importlib.util.spec_from_file_location("oracle_graphviz", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


def engine_points(oracle, tmp, name, count, edges):
    """`twopi -Tplain -Gstart=1` over one DOT graph, as dense-indexed points."""
    source = [a for a, _ in edges]
    target = [b for _, b in edges]
    dot = os.path.join(tmp, f"{name}.dot")
    oracle.write_dot(dot, count, source, target)
    _, nodes = oracle.parse_plain(oracle.run_engine("twopi", dot), count)
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


def printed_nodes(oracle, tmp, name, count, edges):
    """Graphviz's own node coordinates, as the two strings `-Tplain` printed for each.

    The text, not the parsed float: the comparison is byte for byte, and re-printing a
    parsed value would grade our `f64` against the oracle's rounding instead of against the
    oracle's arithmetic.
    """
    dot = os.path.join(tmp, f"{name}.dot")
    oracle.write_dot(dot, count, [a for a, _ in edges], [b for _, b in edges])
    rows = {}
    for line in oracle.run_engine("twopi", dot).splitlines():
        parts = line.split()
        if len(parts) >= 4 and parts[0] == "node":
            rows[parts[1]] = (parts[2], parts[3])
    return [rows[f"n{i}"] for i in range(count)]


def closed_case(oracle, tmp, name):
    """One closed case: Graphviz's arm rendered against the closed answer, exactly."""
    edges = CLOSED_CASES[name]
    count = 1 + max((max(edge) for edge in edges), default=0)
    theirs = printed_nodes(oracle, tmp, f"closed-{name}", count, edges)
    want = rendered(CLOSED_ANSWERS[name])
    got = " ".join(f"{x} {y}" for x, y in theirs)
    return {"nodes": count, "exact": want == got, "want": want, "got": got}


def sweep(oracle, tmp, fixtures):
    """Every fixture through the engine: the worst gap in points, and the raw output.

    The raw output is written to `<graphviz-out-dir>/graphviz-twopi.jsonl` so the ADR's
    determinism evidence is reproducible by running this twice and `cmp`-ing that file,
    and so a reviewer can read what the oracle actually said per seed.
    """
    worst = 0.0
    theirs = []
    for record in fixtures:
        points = engine_points(
            oracle, tmp, f"g{record['seed']}", record["n"], edges_of(record)
        )
        worst = max(worst, gap(ours_of(record), points))
        theirs.append({"seed": record["seed"], "n": record["n"], "points": points})
    return worst, theirs


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: oracle-twopi.py <fixtures-dir> <graphviz-out-dir>")
    fixtures_dir, out_dir = sys.argv[1], sys.argv[2]
    oracle = load_oracle(os.path.dirname(os.path.abspath(__file__)))
    manifest = read(os.path.join(fixtures_dir, "twopi-manifest.json"))
    fixtures = read_lines(os.path.join(fixtures_dir, "twopi.jsonl"))
    with tempfile.TemporaryDirectory() as tmp:
        worst, theirs = sweep(oracle, tmp, fixtures)
        closed = {name: closed_case(oracle, tmp, name) for name in CLOSED_CASES}
    os.makedirs(out_dir, exist_ok=True)
    write_lines(os.path.join(out_dir, "graphviz-twopi.jsonl"), theirs)
    exact = all(row["exact"] for row in closed.values())
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": manifest["sha256"]["twopi.jsonl"],
        "oracle": "Graphviz 16.1.0 twopi -Tplain -Gstart=1",
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


def read(path):
    with open(path) as handle:
        return json.load(handle)


def read_lines(path):
    with open(path) as handle:
        return [json.loads(line) for line in handle]


def write_lines(path, rows):
    with open(path, "w") as handle:
        for row in rows:
            handle.write(json.dumps(row) + "\n")


if __name__ == "__main__":
    sys.exit(main())
