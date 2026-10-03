"""Differential of the six igraph-family force layouts against python-igraph 0.11.9, run in
the ge-python-oracle image:

  graph-cli emit-igraph-fixtures --seeds 100
  docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/oracle-igraph.py target/igraph-fixtures
  graph-cli oracle-igraph

Per seed, igraph lays the same graph out from our start positions (`seed=`) where the
layout takes one; LGL and the reference RNG are otherwise seeded through `random`. Both
arms are scored by normalised stress against graph distance after the optimal uniform
scale, and the recorded worst per layout is max(ours / igraph) over the seeds where `ours`
was emitted. A layout with no `ours` column has zero cases: graph-cli fails it.

**The metric is two-dimensional, and so is the reference.** `ours` carries `x`/`y` only and
the score is a stress ratio between two drawings of the same dimension, so `DIMS[name]`
below is `2` for every layout here and is passed explicitly rather than left to igraph's
own default. SciGraphs' 3-D arms (`igraph_layouts.py:74`, `:99`, `:342`) pass `dim=3`, but
those are the *3-D* layouts; the 2-D DrL at `:406` passes `dim=2`, which is the one this
comparison is against. Measured: `layout_kamada_kawai(dim=3)` **refuses** outright on a
2-column start matrix (`igraph/_igraph.InternalError: Invalid start position matrix size in
3d Kamada-Kawai layout`), so `dim=3` is not available to this arm at all, and forcing it on
`drl` moves its worst from 0.494 to 0.894 by comparing a 3-D drawing to a 2-D metric. M21
proposed `dim=3` for `drl` and `kamada_kawai`; both halves are recorded `false` in
`docs/measurements/fix-harness-py.md` with that measurement as the evidence.

`drl` is given `options="default"` explicitly. That is the preset the port mirrors
(`igraph_layouts.py:200-214`, `edge_cut = 32.0/40.0 = 0.8` = `drl.rs:58`) and it is also
igraph's own default, so naming it costs nothing and stops the agreement from being a
coincidence of a library default: measured identical worst (0.4937) either way.

Ponytail: stress is not what FR, DrL, LGL or Graphopt optimise, so the ratio is a quality
floor, not a coordinate agreement; only pairs inside one component are scored, so a
disconnected graph is judged on its components alone. A reference stress of zero (a graph
with no scored pair) is skipped, and a ratio uses a 1e-3 floor under igraph's stress (so near-trees where both arms reach ~0 do not divide noise by noise, and a real gap smaller than 1e-3 there is invisible).
"""
import json
import os
import random
import sys

import igraph
import numpy as np

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oracle_common import (  # noqa: E402
    finite,
    read_manifest,
    require_cases,
    require_seeds,
)

if len(sys.argv) != 2:
    sys.exit("usage: oracle-igraph.py <fixtures-dir>")
directory = sys.argv[1]
manifest, digest = read_manifest(directory, "igraph")
with open(os.path.join(directory, "igraph.jsonl")) as handle:
    lines = handle.readlines()
require_seeds(manifest, lines, "igraph")

FLOOR = 1e-3

# The width of every drawing this arm compares. `ours` carries `x`/`y` and nothing else, so
# the stress ratio is only meaningful between two drawings of the same width.
DIMENSION = 2

# key -> (method name, takes a start layout as `seed`, the `dim` to pass or None)
REFERENCES = {
    "fruchterman_reingold": ("layout_fruchterman_reingold", True, 2),
    "kamada_kawai": ("layout_kamada_kawai", True, 2),
    "drl": ("layout_drl", True, 2),
    "lgl": ("layout_lgl", False, None),
    "davidson_harel": ("layout_davidson_harel", True, None),
    "graphopt": ("layout_graphopt", True, None),
}

# `dim` is load-bearing and igraph rejects it outright where the layout has no choice:
# `layout_lgl`, `layout_davidson_harel` and `layout_graphopt` raise
# `TypeError: got an unexpected keyword argument 'dim'` (measured), so for those three the
# dimensionality is igraph's own and fixed at 2 — which is what SciGraphs' 2-D arms rely on
# too. `None` means "igraph has no `dim` to set", and the shape check in `reference_layout`
# still refuses a drawing of any other width, so the metric can never compare across
# dimensions.
DIMENSIONLESS = "igraph has no dim parameter for this layout; it is 2-D only"


# SciGraphs' Davidson-Harel parameters (`docs/layouts/layout.force.davidson_harel.md`), which
# our defaults follow; python-igraph's own defaults are density-dependent and anneal longer.
EXPLICIT = {
    "davidson_harel": dict(
        maxiter=10, fineiter=0, cool_fact=0.95, weight_node_dist=1.0, weight_border=0.0,
        weight_edge_lengths=1.0, weight_edge_crossings=1.0, weight_node_edge_dist=1.0,
    ),
    # The preset the port mirrors, named so the agreement is stated rather than inherited
    # from a library default that could change under us.
    "drl": dict(options="default"),
}


def reference_layout(graph, key, start, seed):
    """igraph's own drawing, at the dimensionality this two-dimensional metric needs."""
    method, takes_start, dim = REFERENCES[key]
    random.seed(seed)
    kwargs = {"seed": start} if takes_start else {}
    if dim is not None:
        kwargs["dim"] = dim
    kwargs.update(EXPLICIT.get(key, {}))
    coords = np.array(getattr(graph, method)(**kwargs).coords, dtype=float)
    # The comparison is `ours` (x/y, 2-D) against the reference under one stress ratio, so a
    # reference drawing of any other width is refused rather than scored. This is the check
    # that makes `dim` load-bearing here without a silent comparison across dimensions.
    if coords.shape[1] != DIMENSION:
        sys.exit(f"{key}: igraph returned {coords.shape[1]} dimensions, want {DIMENSION}")
    return coords


def normalised_stress(coords, dist):
    """Stress with weights D^-2 over finite, positive pairs, at the best uniform scale.

    None when the case cannot be scored: no finite positive pair, or a **collapsed** drawing,
    where the best uniform scale is 0/0. That case used to be given `scale = 0.0`, which
    made every pair's residual exactly `-d` and reported a large *reference* stress from a
    drawing with no geometry at all — a fabricated measurement, recorded as if it were one.
    """
    pair = np.isfinite(dist) & (dist > 0)
    if not pair.any():
        return None
    diff = coords[:, None, :] - coords[None, :, :]
    euclid = np.sqrt((diff ** 2).sum(axis=2))
    d = dist[pair]
    e = euclid[pair]
    w = 1.0 / d ** 2
    denom = (w * e * e).sum()
    if denom <= 0.0:
        return None
    scale = (w * d * e).sum() / denom
    return finite(float((w * (scale * e - d) ** 2).sum() / w.sum()), "igraph stress")


layouts = {key: {"cases": 0, "worst": 0.0, "reference_worst": 0.0} for key in REFERENCES}
for text in lines:
    case = json.loads(text)
    graph = igraph.Graph(n=case["n"], edges=list(zip(case["source"], case["target"])))
    graph.simplify()
    dist = np.array(graph.distances(), dtype=float)
    start = list(zip(case["initial"]["x"], case["initial"]["y"]))
    for key, ours in case["ours"].items():
        theirs = reference_layout(graph, key, start, case["seed"])
        s_ref = normalised_stress(theirs, dist)
        s_our = normalised_stress(np.column_stack([ours["x"], ours["y"]]).astype(float), dist)
        if s_ref is None or s_our is None:
            continue
        row = layouts[key]
        row["cases"] += 1
        # `finite` before the accumulator: `max` is false for a NaN, so a NaN ratio would
        # leave `worst` at its 0.0 initialiser and the layout would report as a perfect match.
        ratio = finite(s_our / max(s_ref, FLOOR), f"{key} stress ratio")
        row["worst"] = max(row["worst"], ratio)
        row["reference_worst"] = max(row["reference_worst"], s_ref)

require_cases(layouts, tuple(REFERENCES), "igraph")

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"python-igraph {igraph.__version__} (C core {igraph.__igraph_version__})",
    "layouts": layouts,
}
with open(os.path.join(directory, "igraph-result.json"), "w") as out:
    json.dump(result, out, indent=1)
print(json.dumps(layouts))
