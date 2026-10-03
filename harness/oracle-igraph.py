"""Differential of the igraph-family force layouts against python-igraph 0.11.9, run in
the ge-python-oracle image:

  graph-cli emit-igraph-fixtures --seeds 100
  docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/oracle-igraph.py target/igraph-fixtures
  graph-cli oracle-igraph

and, for the 3D arms, the same three steps over ``igraph3d``:

  graph-cli emit-igraph3d-fixtures --seeds 100
  docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/oracle-igraph.py target/igraph3d-fixtures
  graph-cli oracle-igraph3d

**The fixture directory's name is what selects the dimension.** ``igraph`` is the 2D set
and ``igraph3d`` the 3D one; the two differ in more than the seed's width, so they are two
runs of this one file rather than one run guessing. In the 3D set every reference call
carries ``dim = 3`` and the start is an ``[x, y, z]`` triple per node — which is what
python-igraph's ``seed`` takes in 3D (``igraph_layouts.py:301``) and what SciGraphs' own
``IGRAPH_FR``/``IGRAPH_KK``/``IGRAPH_DRL`` pass.

Per seed, igraph lays the same graph out from our start positions (`seed=`) where the
layout takes one; LGL and the reference RNG are otherwise seeded through `random`. Both
arms are scored by normalised stress against graph distance after the optimal uniform
scale, and the recorded worst per layout is max(ours / igraph) over the seeds where `ours`
was emitted. A layout with no `ours` column has zero cases: graph-cli fails it.

**In the ``igraph`` set the metric is two-dimensional, and so is the reference.** `ours` carries `x`/`y` only and
the score is a stress ratio between two drawings of the same dimension, so the `dim` column of `REFERENCES`
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
with no scored pair) is skipped, and a ratio uses a 1e-3 floor under igraph's stress (so near-trees where both arms reach ~0 do not divide noise by noise, and a real gap smaller than 1e-3 there is invisible). Stress in 3D is a *different* measurement from stress in 2D, not the same number in a bigger box, which is why the 3D arms carry their own ceilings (``docs/measurements/p12-t4b.md``).
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
# The set's own name is the dimension: `igraph3d` is 3D, anything else is the 2D set.
NAME = os.path.basename(os.path.normpath(directory)).removesuffix("-fixtures")
if NAME not in ("igraph", "igraph3d"):
    sys.exit(f"{NAME}: not an igraph fixture set (want igraph or igraph3d)")
DIM = 3 if NAME == "igraph3d" else 2
manifest, digest = read_manifest(directory, NAME)
with open(os.path.join(directory, f"{NAME}.jsonl")) as handle:
    lines = handle.readlines()
require_seeds(manifest, lines, NAME)

FLOOR = 1e-3

# key -> (method name, takes a start layout as `seed`, the 2D set's `dim` or None). The 3D
# set passes `dim = 3` to the same layouts and lays out only those.
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

# The layouts with a 3D arm, hence a 3D igraph counterpart. SciGraphs calls FR, KK and
# DrL at `'dim': 3` (`igraph_layouts.py:74`, `:99`, `:342`).
DIM_ARMED = {key for key, (_, _, dim) in REFERENCES.items() if dim is not None}


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
    """igraph's own drawing, at the dimensionality of the set's metric."""
    method, takes_start, dim = REFERENCES[key]
    random.seed(seed)
    kwargs = {"seed": start} if takes_start else {}
    if dim is not None:
        kwargs["dim"] = DIM
    kwargs.update(EXPLICIT.get(key, {}))
    coords = np.array(getattr(graph, method)(**kwargs).coords, dtype=float)
    # The comparison is `ours` (one column per axis) against the reference under one stress
    # ratio, so a reference drawing of any other width is refused rather than scored. This is the check
    # that makes `dim` load-bearing here without a silent comparison across dimensions.
    if coords.shape[1] != DIM:
        sys.exit(f"{key}: igraph returned {coords.shape[1]} dimensions, want {DIM}")
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
    # Not `finite`: a non-finite stress is counted as unscoreable by the caller, not refused.
    return float((w * (scale * e - d) ** 2).sum() / w.sum())


layouts = {
    key: {
        "cases": 0, "worst": 0.0, "reference_worst": 0.0,
        # Seeds where no finite stress ratio exists, split by which side degenerated.
        "unscoreable": 0, "unscoreable_reference": 0,
    }
    for key in (DIM_ARMED if DIM == 3 else REFERENCES)
}
for text in lines:
    case = json.loads(text)
    graph = igraph.Graph(n=case["n"], edges=list(zip(case["source"], case["target"])))
    graph.simplify()
    dist = np.array(graph.distances(), dtype=float)
    axes = ("x", "y", "z")[:DIM]
    start = list(zip(*[case["initial_3d" if DIM == 3 else "initial"][a] for a in axes]))
    for key, ours in case["ours"].items():
        theirs = reference_layout(graph, key, start, case["seed"])
        s_ref = normalised_stress(theirs, dist)
        s_our = normalised_stress(np.column_stack([ours[a] for a in axes]).astype(float), dist)
        if s_ref is None or s_our is None:
            continue
        row = layouts[key]
        # A non-finite stress on either side is skipped, and counted. `max()` would drop a
        # NaN silently and the row would read as if every seed had been compared, which is
        # the vacuous-pass failure mode: the case is genuinely unscoreable (no finite
        # uniform scale exists over a coordinate set that is not finite), so skipping it is
        # honest, but hiding *how many* were skipped is not. Measured at dim=3: igraph's
        # own 3D KK returns a NaN stress on seed 601 (n=3, a triangle) — the reference
        # degenerating, not this port, whose stress there is 2.4e-3.
        if not (np.isfinite(s_ref) and np.isfinite(s_our)):
            row["unscoreable"] += 1
            if not np.isfinite(s_ref):
                row["unscoreable_reference"] += 1
            continue
        row["cases"] += 1
        # `finite` before the accumulator: `max` is false for a NaN, so a NaN ratio would
        # leave `worst` at its 0.0 initialiser and the layout would report as a perfect match.
        ratio = finite(s_our / max(s_ref, FLOOR), f"{key} stress ratio")
        row["worst"] = max(row["worst"], ratio)
        row["reference_worst"] = max(row["reference_worst"], s_ref)

require_cases(layouts, tuple(layouts), NAME)

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"python-igraph {igraph.__version__} (C core {igraph.__igraph_version__}) at dim={DIM}",
    "layouts": layouts,
}
with open(os.path.join(directory, f"{NAME}-result.json"), "w") as out:
    json.dump(result, out, indent=1)
print(json.dumps(layouts))
