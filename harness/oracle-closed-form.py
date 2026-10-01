"""Differential of layout.circular.ring, layout.spiral, layout.bipartite and the three
3D arms of p12-t4a, run in the ge-python-oracle image:

  graph-cli emit-closed-form-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-closed-form.py target/closed-form-fixtures
  graph-cli oracle-closed-form

Metric per layout: the largest absolute coordinate difference (both arms are rescaled to
unit scale). The result holds the worst value per layout; graph-cli holds the ceiling.
The bipartite arm passes networkx our own first column as `nodes=` (see the graph-cli
module's Ponytail) and compares x per node and y as sorted values per column.

The 3D arms, and WHY each is compared against what it is compared against
(`docs/measurements/p12-t4a.md` records this; it is not a preference):

  random_3d     networkx `random_layout(dim=3)`. networkx DOES take dim here.
  spiral_3d     SciGraphs `_spiral_layout_3d`. networkx `spiral_layout` REFUSES dim=3
                ("can only handle 2 dimensions", drawing/layout.py:1315) and the
                reference's curve is a different one anyway -- a cone climbing in z,
                not networkx's flat Archimedean spiral.
  bipartite_3d  SciGraphs `_bipartite_layout_3d`. networkx `bipartite_layout` has no
                `dim` parameter at all, and SciGraphs' 3D form puts each set on a RING
                on its own plane where the 2D form is two vertical COLUMNS.

Ponytail: the bipartite comparison checks geometry given a partition, not the partition
rule, and not the order of nodes inside a column. The gate model is one connected graph
per seed, so an empty graph, one node and a disconnected graph are not exercised here.
Ponytail (3D): random_3d is compared as a DISTRIBUTION, not coordinate-wise -- our
stream is Mulberry32 and the reference's is numpy's Mersenne Twister, so no coordinate
can ever match; the arm therefore checks the per-axis mean and variance the contract
decision names, and reports its worst deviation from them.
"""
import hashlib, json, os, sys
import numpy as np
import networkx as nx

core = "/w/SciGraphs/core"
if os.path.isdir(core):
    sys.path.insert(0, core)
from scigraphs_core.mesh.layouts import basic as ref_basic
from scigraphs_core.mesh.layouts import hierarchical as ref_hier

directory = sys.argv[1]
path = os.path.join(directory, "closed-form.jsonl")
manifest = json.load(open(os.path.join(directory, "closed-form-manifest.json")))
digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
if digest != manifest["sha256"]["closed-form.jsonl"]:
    sys.exit("closed-form.jsonl does not match its manifest")


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
    worst = float(np.abs(ours[:, 0] - theirs[:, 0]).max())
    for side in (first, [i for i in range(len(ours)) if i not in set(first)]):
        worst = max(worst, gap(np.sort(ours[side, 1]), np.sort(theirs[side, 1])) if side else 0.0)
    return worst


def bipartite_3d_gap(graph, ours):
    """Max coordinate gap against SciGraphs' rings, comparing each ring as a SET of
    (x, y, z) points: the node order inside a ring is the partition's own, which the
    2D arm's Ponytail already says is not the thing under test."""
    theirs = ref_hier._bipartite_layout_3d(graph, 1.0)
    worst = 0.0
    for plane in (-0.5, 0.5):
        ours_side = ours[np.isclose(ours[:, 2], plane, atol=1e-6)]
        theirs_side = theirs[np.isclose(theirs[:, 2], plane, atol=1e-9)]
        if not len(ours_side) or not len(theirs_side):
            continue
        assert len(ours_side) == len(theirs_side), (
            f"plane {plane}: {len(ours_side)} of our nodes vs {len(theirs_side)} of the "
            "reference's -- the partition disagrees, not the geometry"
        )
        # Our z is written to f32, the reference's is exact, so compare z to f32 too.
        worst = max(worst, float(np.abs(ours_side[:, 2] - plane).max()))
        for axis in (0, 1):
            worst = max(
                worst,
                gap(np.sort(ours_side[:, axis]), np.sort(theirs_side[:, axis])),
            )
    return worst


def random_3d_gap(case, key):
    """Deviation of our draw from the uniform [0,1)^3 the contract names, as the max of
    the per-axis mean and variance errors. A COORDINATE comparison is impossible here
    (our stream is not numpy's), so this is the honest metric: the decision's
    "per-axis mean ~ scale/2, variance ~ scale^2/12" (docs/decisions/contract-3d.md:166-170).
    """
    ours = block(case, key)
    worst = 0.0
    for axis in range(3):
        column = ours[:, axis]
        mean_error = abs(column.mean() - 0.5)
        var_error = abs(column.var() - 1.0 / 12.0)
        worst = max(worst, mean_error, var_error)
    return float(worst)


KEYS = ("ring", "spiral", "bipartite", "random_3d", "spiral_3d", "bipartite_3d")
layouts = {key: {"cases": 0, "worst": 0.0} for key in KEYS}
for text in open(path):
    case = json.loads(text)
    n = case["n"]
    graph = nx.Graph()
    graph.add_nodes_from(range(n))
    graph.add_edges_from((s, t) for s, t in zip(case["source"], case["target"]) if s != t)
    metrics = {
        "ring": gap(block(case, "ring"), np.array([nx.circular_layout(graph)[i] for i in range(n)])),
        "spiral": gap(block(case, "spiral"), np.array([nx.spiral_layout(graph)[i] for i in range(n)])),
        "bipartite": bipartite_gap(graph, block(case, "bipartite")),
        "random_3d": random_3d_gap(case, "random_3d"),
        "spiral_3d": gap(block(case, "spiral_3d"), ref_basic._spiral_layout_3d(n, 1.0)),
        "bipartite_3d": bipartite_3d_gap(graph, block(case, "bipartite_3d")),
    }
    for key, metric in metrics.items():
        layouts[key]["cases"] += 1
        layouts[key]["worst"] = max(layouts[key]["worst"], metric)

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"networkx {nx.__version__}, numpy {np.__version__}, SciGraphs basic/hierarchical",
    "layouts": layouts,
}
json.dump(result, open(os.path.join(directory, "closed-form-result.json"), "w"), indent=1)
print(json.dumps(layouts))
