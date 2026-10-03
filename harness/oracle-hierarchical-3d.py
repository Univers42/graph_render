"""Differential of layout.hierarchical3d against SciGraphs' own _hierarchical_layout_3d
(SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:113-147), run in the
ge-python-oracle image with the SciGraphs/ submodule mounted so the arm is the reference
function itself and not a restatement of it:

  graph-cli emit-hierarchical-3d-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-hierarchical-3d.py target/hierarchical-3d-fixtures
  graph-cli oracle-hierarchical-3d

Each seed's graph is rebuilt as an nx.Graph over nodes 0..n-1 in order (self-loops dropped,
parallel edges merged: the port runs on the same undirected simple graph) and handed to the
reference at the fixture's own `scale`, which is SciGraphs' dispatcher default of 5.0. Node
insertion order and adjacency order therefore agree with the port's dense index and its CSR
rows, which is what makes the disk-by-disk comparison node by node.

**Its own arm file, not one shared with the three graph-free placements.** It is the only
one of the five 3D layouts that reads a graph, so it is compared over SHAPES — the BFS
levels, the per-level disk radius, and the ring split inside `_disk_positions` — rather than
over node counts. Folding it into `oracle-basic-3d.py` would put a graph-reading layout and
two graph-free ones behind one selector whose inputs are not the same.

Metric: the largest absolute coordinate difference over all three axes. The layout is a
closed form, so agreement is a tolerance and nothing weaker; the result holds the worst case
and also how many seeds matched bit for bit after the snapshot's f32 narrowing, and graph-cli
holds the ceiling.

Two things the arm compares that a coordinate gap alone would not catch, because they are
the reference's own behaviours rather than the port's arithmetic:

  - `_disk_positions` rounds `take` with **ties to even** (hierarchical.py:97, :99). At a
    level whose share lands on an exact half — `count = 10` over 2 rings is
    `np.round([2.5, 7.5]) = [2, 8]` — `f64::round` would give `[3, 8]`, the reference's own
    `while take.sum() > count` loop would then move a unit, and the drawing would come out
    `[3, 7]`: a different PICTURE, not a different rounding. So the ring split is reported
    as its own number beside the coordinate gap.
  - the reference's docstring is stale: it says "BFS depth sets Y, each level fills a disk in
    XZ" while the code puts the level on z (:140, :144) and the disk in x/y. This arm is the
    reference, so it settles that question in the motor's favour without a word of prose.

Ponytail: the arm is handed an nx.Graph, never an nx.DiGraph. The port reads a directed
input undirected, because the motor's Topology carries directedness per edge and has no
whole-graph flag, so SciGraphs' in-degree-0 roots branch (hierarchical.py:127-128) is not
ported and is not compared — the same departure `oracle-circular-hierarchy.py` states. The
gate model is one connected random graph per seed, so a single node, an empty graph and a
disconnected one rest on graph-core's own tests. The reference prints two progress lines per
call, so it is silenced rather than left to interleave with the result.
"""
import contextlib
import io
import json
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import networkx as nx  # noqa: E402
import numpy as np  # noqa: E402

from oracle_common import (  # noqa: E402
    finite,
    read_manifest,
    require_cases,
    require_seeds,
)

sys.path.insert(0, os.path.join("SciGraphs", "core"))
from scigraphs_core.mesh.layouts.hierarchical import (  # noqa: E402
    _hierarchical_layout_3d,
)

directory = sys.argv[1]
name = "hierarchical-3d"
manifest, digest = read_manifest(directory, name)
with open(os.path.join(directory, f"{name}.jsonl")) as handle:
    lines = handle.readlines()
require_seeds(manifest, lines, name)


def theirs_of(case):
    """The reference's own answer over the fixture's graph, at the fixture's own scale."""
    graph = nx.Graph()
    graph.add_nodes_from(range(case["n"]))
    graph.add_edges_from(
        (s, t) for s, t in zip(case["source"], case["target"]) if s != t
    )
    with contextlib.redirect_stdout(io.StringIO()):
        positions = _hierarchical_layout_3d(graph, case["scale"])
    return np.asarray(positions, dtype=float)


def ring_split(positions):
    """The ring split of each level's disk, read back off the drawing by counting the
    distinct distances from each level's own axis.

    A count rather than a recomputation: the reference's `take` is not observable from
    outside `_disk_positions`, but the drawing it produces IS — every point of a ring sits
    at exactly `radius * (k + 0.5) / rings`, so the number of distinct radii is the number
    of non-empty rings. Comparing that is what catches a ties-to-even disagreement, which
    moves every node after the tie rather than one coordinate.
    """
    splits = {}
    for level in np.unique(positions[:, 2]):
        on_level = positions[positions[:, 2] == level][:, :2]
        if len(on_level) <= 1:
            splits[float(level)] = 0
            continue
        radii = np.hypot(on_level[:, 0], on_level[:, 1])
        # f32 vs f64: the drawing arrives narrowed, so radii within a rounding step of each
        # other are one ring, not several.
        distinct = 0
        last = None
        for value in sorted(radii):
            if last is None or value - last > 1e-3:
                distinct += 1
                last = value
        splits[float(level)] = distinct
    return splits


cases = len(lines)
worst = 0.0
worst_seed = None
exact = 0
ring_disagreements = 0

for text in lines:
    case = json.loads(text)
    ours = np.column_stack(
        [case[name]["x"], case[name]["y"], case[name]["z"]]
    ).astype(float)
    theirs = theirs_of(case)
    if ours.shape != theirs.shape:
        sys.exit(f"seed {case['seed']}: shape {ours.shape} vs {theirs.shape}")
    gap = float(np.abs(ours - theirs).max())
    # `>` is false for a NaN, so the guard is the metric and not the comparison: without it a
    # NaN coordinate leaves `worst` at its 0.0 initialiser and the case reports as a match.
    if finite(gap, "gap") > worst:
        worst, worst_seed = gap, case["seed"]
    if np.array_equal(ours.astype(np.float32), theirs.astype(np.float32)):
        exact += 1
    # The ring split is compared as a SHAPE and not as coordinates, and a disagreement is
    # counted rather than gated: it can only arise from the ties-to-even rule, which the arm
    # and the port share by construction. A count is here so that a future port that loses
    # it shows up as a number, not as a coordinate gap nobody reads.
    if ring_split(ours) != ring_split(theirs):
        ring_disagreements += 1

layouts = {
    name: {
        "cases": cases,
        "worst": worst,
        "worst_seed": worst_seed,
        "f32_bit_exact": exact,
        "ring_split_disagreements": ring_disagreements,
    }
}
require_cases(layouts, (name,), name)
result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"SciGraphs _hierarchical_layout_3d + _disk_positions on networkx "
    f"{nx.__version__}, numpy {np.__version__}",
    "layouts": layouts,
}
with open(os.path.join(directory, f"{name}-result.json"), "w") as out:
    json.dump(result, out, indent=1)
print(json.dumps(layouts))