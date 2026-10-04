"""Differential of layout.circular.ring, layout.spiral, layout.bipartite, layout.random.3d,
layout.basic3d.spiral and layout.bipartite_3d against networkx 3.6's circular_layout,
spiral_layout and bipartite_layout plus SciGraphs' _spiral_layout_3d and
_bipartite_layout_3d, run in the ge-python-oracle image:

  graph-cli emit-closed-form-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-closed-form.py target/closed-form-fixtures
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-closed-form.py target/closed-form-fixtures /w/SciGraphs/core
  graph-cli oracle-closed-form

The second argv is the SciGraphs core directory the two closed-form 3-D references live
in; omitted it defaults to <repo>/SciGraphs/core, which is where the submodule lands.

Metric per 2-D layout: the largest absolute coordinate difference (both arms are rescaled
to unit scale). The result holds the worst value per layout; graph-cli holds the ceiling.
The bipartite arm passes networkx our own first column as `nodes=` (see the graph-cli
module's Ponytail) and compares x per node and y as sorted values per column.

Metric per 3-D layout:
  layout.random.3d      distribution only: the worst per-axis error in the sample mean
                        against 0.5 and in the sample variance against 1/12. A coordinate
                        comparison is impossible (our stream is Mulberry32, the
                        reference's is numpy's Mersenne Twister), so this arm is a
                        distribution metric, not a coordinate metric.
  layout.basic3d.spiral largest absolute coordinate difference against
                        `_spiral_layout_3d(n, 5.0)`; 5.0 is `basic_3d::SCALE`, the
                        dispatcher's `scale = 5.0` and what layout.basic3d.spiral draws.
  layout.bipartite_3d   largest absolute coordinate difference against
                        `_bipartite_layout_3d(G, 5.0)`, comparing each of the two rings
                        as a SET of (x, y, z) points.

Ponytail: the bipartite comparison checks geometry given a partition, not the partition
rule, and not the order of nodes inside a column. The gate model is one connected graph
per seed, so an empty graph, one node and a disconnected graph are not exercised here.
Ponytail (3-D): the bipartite_3d comparison inherits that caveat and adds the node order
inside a ring, which is the partition's own; a partition that is correct but rotated
within its plane passes. _bipartite_layout_3d prints progress on stdout and falls back to
a greedy maximum cut on a non-bipartite graph, so the print (not the gate) is the only
warning that the partition under test was not the two-colouring.
Ponytail (3-D random): the distribution metric cannot see a wrong but unbiased stream --
a mis-scaled axis, a dropped draw's successor or a stream that is uniform in the wrong
sub-range can all pass while every coordinate is wrong. The escape hatch is the gate's
ceiling on the mean/variance error, not a coordinate comparison.
"""
import json
import os
import sys

import numpy as np
import networkx as nx

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oracle_common import (  # noqa: E402
    finite,
    networkx_version,
    read_manifest,
    require_cases,
    require_seeds,
)

if len(sys.argv) not in (2, 3):
    sys.exit(
        "usage: oracle-closed-form.py <fixtures-dir> [SciGraphs-core-dir]"
    )
fixtures_dir = sys.argv[1]
core = sys.argv[2] if len(sys.argv) == 3 else os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "SciGraphs", "core"
)
# sys.path silently ignores a directory that is not there, so the two closed-form 3-D
# references would surface as an ImportError with no hint about which path was wrong.
if not os.path.isdir(core):
    sys.exit(f"oracle-closed-form.py: SciGraphs core directory not found: {core}")
sys.path.insert(0, core)
from scigraphs_core.mesh.layouts import basic as ref_basic  # noqa: E402
from scigraphs_core.mesh.layouts import hierarchical as ref_hier  # noqa: E402

# The only scale the 3-D dispatcher hands these two layouts (dispatcher.py:14).
SCALE_3D = 5.0

networkx_version(nx)
manifest, digest = read_manifest(fixtures_dir, "closed-form")
# One open, one pass: the digest and the measurement read the same handle's bytes. Two
# independent opens left a window in which an emit that ran while this arm was reading
# produced a result whose `sha256` covered fewer bytes than it measured.
with open(os.path.join(fixtures_dir, "closed-form.jsonl")) as handle:
    lines = handle.readlines()
require_seeds(manifest, lines, "closed-form")


def block(case, key):
    """A case's columns as an (n, d) float array: xy, or xyz when the layout is 3D."""
    ours = case[key]
    columns = [ours["x"], ours["y"]]
    if "z" in ours:
        columns.append(ours["z"])
    return np.column_stack(columns).astype(float)


def gap(ours, theirs):
    return float(np.abs(ours - theirs).max())


def bipartite_gap(graph, ours):
    xs = np.unique(np.round(ours[:, 0], 9))
    first = [i for i in range(len(ours)) if ours[i, 0] <= xs[0] + 1e-9]
    theirs = nx.bipartite_layout(graph, nodes=set(first))
    theirs = np.array([theirs[i] for i in range(len(ours))])
    worst = finite(float(np.abs(ours[:, 0] - theirs[:, 0]).max()), "bipartite x gap")
    for side in (first, [i for i in range(len(ours)) if i not in set(first)]):
        worst = max(worst, gap(np.sort(ours[side, 1]), np.sort(theirs[side, 1])) if side else 0.0)
    return worst


def reference_points(graph, n):
    """Both networkx layouts, once per case and each exactly once.

    They used to be called inside the per-node comprehension that indexed their results, so
    `circular_layout` and `spiral_layout` were each rebuilt `n` times per case — 601 calls x
    1000 seeds per arm, on a metric that does not depend on the node.
    """
    return {
        "ring": np.array([nx.circular_layout(graph)[i] for i in range(n)]),
        "spiral": np.array([nx.spiral_layout(graph)[i] for i in range(n)]),
    }


def random_3d_gap(case, key):
    """Worst per-axis deviation of our draw from the uniform the contract names: the sample
    mean against 0.5 and the sample variance against 1/12, six numbers, worst wins.

    This is a DISTRIBUTION metric and not a coordinate metric, and no coordinate comparison
    is possible: our stream is Mulberry32 and the reference's is numpy's Mersenne Twister,
    so the two draw different values at the same index however correct either is.

    **The two assertions below are the load-bearing part, and they are exact.** The mean and
    variance terms cannot detect a broken stream: for ANY data in [0, 1) the mean lies in
    [0, 1) so `|mean - 0.5| < 0.5`, and the variance lies in [0, 1/4] so `|var - 1/12| <=
    0.167`. Their worst is therefore bounded by 0.5 for every possible input, which is
    exactly `CEILING_3D_RANDOM`, so on their own they can never turn the row red — measured,
    not argued. An all-zeros z column scores `|0 - 0.5| = 0.5` and PASSES. That is the same
    shape of defect this job found in the LOBPCG start block, where one column of the block
    was left all zeros, so it is the one that had to be caught here.

    A uniform draw of `n >= 2` points spans its axis and does not repeat another axis, both
    with probability zero under `rand(n, 3)`. Those two are properties of the reference
    itself, not tolerances, so they are asserted rather than scored.

    Ponytail: what is left uncaught is a stream that is uniform but WRONG in a way that keeps
    its range and its independence — uniform on a sub-interval, or drawn in the wrong order.
    Failing input: a stream uniform on [0, 0.5) scores a mean gap of 0.25 and passes.
    Direction: always optimistic. Escape hatch: compare against the reference's OWN draw at
    the same `n` rather than against the contract's constants, which turns the metric into a
    two-sample test and costs the independence this arm relies on.
    """
    ours = block(case, key)
    worst = 0.0
    for axis in range(3):
        column = ours[:, axis]
        assert column.max() > column.min(), (
            f"random_3d axis {axis} is constant at {column[0]}: {case['n']} draws off a "
            "uniform stream do not repeat"
        )
        worst = max(
            worst,
            finite(abs(float(column.mean()) - 0.5), f"random_3d mean gap axis {axis}"),
            finite(abs(float(column.var()) - 1.0 / 12.0), f"random_3d var gap axis {axis}"),
        )
    for a, b in ((0, 1), (0, 2), (1, 2)):
        assert not np.array_equal(ours[:, a], ours[:, b]), (
            f"random_3d axes {a} and {b} are the same draws: the stream reuses a column "
            "rather than drawing three"
        )
    return float(worst)


def spiral_3d_gap(case, n):
    """Worst coordinate gap against SciGraphs' conical 3-D spiral at the dispatched scale.

    `SCALE_3D` (5.0) is `basic_3d::SCALE`, the `scale = 5.0` the 3-D dispatcher passes and
    the value `layout.basic3d.spiral` draws at; passing 1.0 instead would compare a unit
    cone against a five-unit one. `_spiral_layout_3d` prints nothing.
    """
    theirs = np.asarray(ref_basic._spiral_layout_3d(n, SCALE_3D), dtype=float)
    return gap(block(case, "spiral_3d"), theirs)


def bipartite_3d_gap(graph, ours):
    """Worst coordinate gap against SciGraphs' two 3-D rings, each side compared as a SET.

    The reference puts each node set on its own plane at z = +-{scale*0.5} and walks the
    ring in the partition's own node order, so a node order that differs inside a ring is
    not a geometry error and is not compared: x and y are compared as sorted values per
    side, and z against the plane. `_bipartite_layout_3d` prints progress and can fall back
    to a greedy maximum cut on a non-bipartite graph; a wrong partition shows up as the
    count assertion below rather than as a coordinate gap.

    Ponytail: x and y are compared as SORTED values per side, so anything that permutes one
    axis independently of the other is invisible — a correct partition rotated within its
    plane, an x/y axis swap, and any independent reordering of one axis all score 0.0. Only
    z, which has one value per plane, is compared per node. A partition that is valid but not
    the one the reference's two-colouring picks is also uncaught, except through the side
    counts. Direction: optimistic. Escape hatch: compare x and y per node in the reference's
    own node order once the partition is asserted, which is what the conformance gate already
    does for this id byte for byte.
    """
    theirs = np.asarray(ref_hier._bipartite_layout_3d(graph, SCALE_3D), dtype=float)
    worst = 0.0
    for plane in (-0.5 * SCALE_3D, 0.5 * SCALE_3D):
        ours_side = ours[np.isclose(ours[:, 2], plane, atol=1e-6, rtol=0.0)]
        theirs_side = theirs[np.isclose(theirs[:, 2], plane, atol=1e-9, rtol=0.0)]
        # An empty side is a FAILURE, not a skip. A `continue` here was measured to read as a
        # perfect match: an empty `worst` is 0.0, so a layout that put every node on one plane,
        # or at the wrong scale, would score 0.0 on both rings and the row would go green on
        # a layout that draws no second ring at all. A partition that puts every node on one
        # side is a real answer too, so the reference's side may legitimately be empty -- but
        # only when OURS is empty as well, and then the two counts still have to agree.
        assert len(ours_side) == len(theirs_side), (
            f"plane {plane}: {len(ours_side)} of our nodes vs {len(theirs_side)} of the "
            "reference's -- the partition disagrees, not the geometry, or a ring is missing "
            f"from our drawing (our z runs {ours[:, 2].min()}..{ours[:, 2].max()} against the "
            f"reference's planes at +-{0.5 * SCALE_3D})"
        )
        if not len(ours_side):
            continue
        # Our z is written to f32, the reference's is exact, so compare z to f32 too.
        worst = max(
            worst,
            finite(float(np.abs(ours_side[:, 2] - plane).max()), "bipartite_3d z gap"),
        )
        for axis in (0, 1):
            worst = max(
                worst,
                gap(np.sort(ours_side[:, axis]), np.sort(theirs_side[:, axis])),
            )
    return worst


KEYS = ("ring", "spiral", "bipartite", "random_3d", "spiral_3d", "bipartite_3d")
layouts = {key: {"cases": 0, "worst": 0.0} for key in KEYS}
for text in lines:
    case = json.loads(text)
    graph = nx.Graph()
    graph.add_nodes_from(range(case["n"]))
    graph.add_edges_from((s, t) for s, t in zip(case["source"], case["target"]) if s != t)
    theirs = reference_points(graph, case["n"])
    metrics = {
        "ring": gap(block(case, "ring"), theirs["ring"]),
        "spiral": gap(block(case, "spiral"), theirs["spiral"]),
        "bipartite": bipartite_gap(graph, block(case, "bipartite")),
        "random_3d": random_3d_gap(case, "random_3d"),
        "spiral_3d": spiral_3d_gap(case, case["n"]),
        "bipartite_3d": bipartite_3d_gap(graph, block(case, "bipartite_3d")),
    }
    for key, metric in metrics.items():
        layouts[key]["cases"] += 1
        # `finite` before the accumulator: `max` is false for a NaN, so a NaN gap would leave
        # `worst` at its 0.0 initialiser and the case would report as a perfect match.
        layouts[key]["worst"] = max(
            layouts[key]["worst"], finite(metric, f"{key} gap")
        )

require_cases(layouts, KEYS, "closed-form")

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": (
        f"networkx {nx.__version__}, numpy {np.__version__}, "
        "SciGraphs basic/hierarchical"
    ),
    "layouts": layouts,
}
with open(os.path.join(sys.argv[1], "closed-form-result.json"), "w") as out:
    json.dump(result, out, indent=1)
print(json.dumps(layouts))
