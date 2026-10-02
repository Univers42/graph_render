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

Ponytail: stress is not what FR, DrL, LGL or Graphopt optimise, so the ratio is a quality
floor, not a coordinate agreement; only pairs inside one component are scored, so a
disconnected graph is judged on its components alone. A reference stress of zero (a graph
with no scored pair) is skipped, and a ratio uses a 1e-3 floor under igraph's stress (so near-trees where both arms reach ~0 do not divide noise by noise, and a real gap smaller than 1e-3 there is invisible). Stress in 3D is a *different* measurement from stress in 2D, not the same number in a bigger box, which is why the 3D arms carry their own ceilings (``docs/measurements/p12-t4b.md``).
"""
import hashlib, json, os, random, sys
import igraph
import numpy as np

directory = sys.argv[1]
# The set's own name is the dimension: `igraph3d` is 3D, anything else is the 2D set.
NAME = os.path.basename(os.path.normpath(directory)).removesuffix("-fixtures")
if NAME not in ("igraph", "igraph3d"):
    sys.exit(f"{NAME}: not an igraph fixture set (want igraph or igraph3d)")
DIM = 3 if NAME == "igraph3d" else 2
path = os.path.join(directory, f"{NAME}.jsonl")
manifest = json.load(open(os.path.join(directory, f"{NAME}-manifest.json")))
digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
if digest != manifest["sha256"][f"{NAME}.jsonl"]:
    sys.exit(f"{NAME}.jsonl does not match its manifest")

FLOOR = 1e-3

# key -> (method name, takes a start layout as `seed`)
REFERENCES = {
    "fruchterman_reingold": ("layout_fruchterman_reingold", True),
    "kamada_kawai": ("layout_kamada_kawai", True),
    "drl": ("layout_drl", True),
    "lgl": ("layout_lgl", False),
    "davidson_harel": ("layout_davidson_harel", True),
    "graphopt": ("layout_graphopt", True),
}


# SciGraphs' Davidson-Harel parameters (`docs/layouts/layout.force.davidson_harel.md`), which
# our defaults follow; python-igraph's own defaults are density-dependent and anneal longer.
EXPLICIT = {
    "davidson_harel": dict(
        maxiter=10, fineiter=0, cool_fact=0.95, weight_node_dist=1.0, weight_border=0.0,
        weight_edge_lengths=1.0, weight_edge_crossings=1.0, weight_node_edge_dist=1.0,
    ),
}


def reference_layout(graph, key, start, seed):
    method, takes_start = REFERENCES[key]
    random.seed(seed)
    kwargs = {"seed": start} if takes_start else {}
    kwargs.update(EXPLICIT.get(key, {}))
    # Only the three arms this job adds take `dim`; LGL, Davidson-Harel and Graphopt are
    # 2D-only ids in the registry and their igraph calls take no such argument, so asking
    # for it would be a TypeError rather than a 3D layout.
    if DIM == 3 and key in DIM_ARMED:
        kwargs["dim"] = DIM
    return np.array(getattr(graph, method)(**kwargs).coords, dtype=float)


# The layouts with a 3D arm, hence a 3D igraph counterpart. SciGraphs calls FR, KK and
# DrL at `'dim': 3` (`igraph_layouts.py:74`, `:99`, `:342`).
DIM_ARMED = {"fruchterman_reingold", "kamada_kawai", "drl"}


def normalised_stress(coords, dist):
    """Stress with weights D^-2 over finite, positive pairs, at the best uniform scale."""
    pair = np.isfinite(dist) & (dist > 0)
    if not pair.any():
        return None
    diff = coords[:, None, :] - coords[None, :, :]
    euclid = np.sqrt((diff ** 2).sum(axis=2))
    d = dist[pair]
    e = euclid[pair]
    w = 1.0 / d ** 2
    denom = (w * e * e).sum()
    scale = (w * d * e).sum() / denom if denom > 0 else 0.0
    return float((w * (scale * e - d) ** 2).sum() / w.sum())


layouts = {
    key: {
        "cases": 0, "worst": 0.0, "reference_worst": 0.0,
        # Seeds where no finite stress ratio exists, split by which side degenerated.
        "unscoreable": 0, "unscoreable_reference": 0,
    }
    for key in (DIM_ARMED if DIM == 3 else REFERENCES)
}
for text in open(path):
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
        row = layouts[key]
        row["cases"] += 1
        row["worst"] = max(row["worst"], s_our / max(s_ref, FLOOR))
        row["reference_worst"] = max(row["reference_worst"], s_ref)

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"python-igraph {igraph.__version__} (C core {igraph.__igraph_version__}) at dim={DIM}",
    "layouts": layouts,
}
json.dump(result, open(os.path.join(directory, f"{NAME}-result.json"), "w"), indent=1)
print(json.dumps(layouts))
