"""Differential of the igraph-family force layouts against python-igraph 0.11.9, run in
the ge-python-oracle image:

  graph-cli emit-igraph-fixtures --seeds 100
  docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/oracle-igraph.py target/igraph-fixtures
  graph-cli oracle-igraph

Per seed, igraph lays the same graph out from our start positions (`seed=`) where the
layout takes one; LGL and the reference RNG are otherwise seeded through `random`. Both
arms are scored by normalised stress against graph distance after the optimal uniform
scale, and the recorded worst per layout is max(ours / igraph) over the seeds where `ours`
was emitted. A layout with no `ours` column has zero cases: graph-cli fails it.

The two `_3d` keys are the same layouts at `dim=3`, the dimension SciGraphs actually asks
for (`igraph_layouts.py:74` for FR, `:99` for KK). They are a **separate pass, not a
replacement**: the 2D keys stay and stay gated, so both dimensions are held to the same
stress metric at the same ceilings. `dim` is a field of the reference below rather than
something derived from the key, so the key that names a dimension and the call that sets
it cannot drift apart.

Ponytail: stress is not what FR, DrL, LGL or Graphopt optimise, so the ratio is a quality
floor, not a coordinate agreement; only pairs inside one component are scored, so a
disconnected graph is judged on its components alone. A reference stress of zero (a graph
with no scored pair) is skipped, and a ratio uses a 1e-3 floor under igraph's stress (so near-trees where both arms reach ~0 do not divide noise by noise, and a real gap smaller than 1e-3 there is invisible).
"""
import hashlib, json, os, random, sys
import igraph
import numpy as np

directory = sys.argv[1]
path = os.path.join(directory, "igraph.jsonl")
manifest = json.load(open(os.path.join(directory, "igraph-manifest.json")))
digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
if digest != manifest["sha256"]["igraph.jsonl"]:
    sys.exit("igraph.jsonl does not match its manifest")

FLOOR = 1e-3

# key -> (method name, takes a start layout as `seed`, dimension or None)
#
# **`dim` is the third field, not a suffix on the key.** The `_3d` entries are the same two
# layouts as their 2D siblings at the dimension SciGraphs calls; everything else about the
# pass — the start handed over, the stress metric, the ceilings in `graph-cli
# oracle_python/igraph.rs` — is identical, which is what makes the two rows comparable.
#
# `None` means **the binding has no `dim` argument at all**, and passing one raises
# `TypeError: unexpected keyword argument 'dim'`: LGL, Davidson-Harel and Graphopt are all
# planar in python-igraph 0.11.9. That is also why SciGraphs passes `dim=3` for FR, KK and DrL
# and not for the other three (`igraph_layouts.py:74`, `:99`, `:342`; LGL's own comment at
# `:453` reads "LGL is 2D only"). Written down because the alternative — guessing that a
# layout takes `dim` because every other one does — is a `TypeError` at run time.
REFERENCES = {
    "fruchterman_reingold": ("layout_fruchterman_reingold", True, 2),
    "kamada_kawai": ("layout_kamada_kawai", True, 2),
    "drl": ("layout_drl", True, 2),
    "lgl": ("layout_lgl", False, None),
    "davidson_harel": ("layout_davidson_harel", True, None),
    "graphopt": ("layout_graphopt", True, None),
    "fruchterman_reingold_3d": ("layout_fruchterman_reingold", True, 3),
    "kamada_kawai_3d": ("layout_kamada_kawai", True, 3),
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
    method, takes_start, dim = REFERENCES[key]
    random.seed(seed)
    kwargs = {"seed": start} if takes_start else {}
    kwargs.update(EXPLICIT.get(key, {}))
    if dim is not None:
        kwargs["dim"] = dim
    return np.array(getattr(graph, method)(**kwargs).coords, dtype=float)


def start_layout(initial, dim):
    """Our start positions at *dim*: the emitted x/y pairs, lifted with a zero third column.

    **The lift is ours and it is stated.** The fixtures carry one start, two columns wide,
    because every planar row reads it. igraph's `seed=` wants an `n x dim` matrix and rejects
    a mismatched one (`fruchterman_reingold.c:503` "Invalid start position"), so at `dim=3`
    the third column is filled with zeros — the plane our start was already drawn in. FR's own
    start box is `[-sqrt(n)/2, +sqrt(n)/2]` per axis, so a zero z is inside it and not an
    extreme point; the comparison is then "same start, one more axis", which is the only
    reading that keeps the 3D ratio about the solver rather than about the start.

    `dim=None` means the binding takes no `dim` and therefore wants a planar start.
    """
    pairs = [list(pair) for pair in zip(initial["x"], initial["y"])]
    if dim in (None, 2):
        return pairs
    return [pair + [0.0] for pair in pairs]


def our_coords(ours):
    """Our own layout as an `n x d` array, in axis order x, y, and z when the id has one.

    **A `_3d` id with no `z` column is a failure, not a planar layout.** The stress of a
    three-axis drawing against the stress of a two-axis one is not a ratio of anything, and it
    would be reported as an ordinary number; so a `_3d` row whose `z` is missing stops here
    instead. The 2D rows carry `z: null` rather than no key at all, so the two are told apart by
    the value and not by the key's absence.
    """
    axes = [ours["x"], ours["y"]]
    if ours.get("z") is not None:
        axes.append(ours["z"])
    if str(ours.get("id", "")).endswith("_3d") and len(axes) < 3:
        return None
    return np.column_stack(axes).astype(float)


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


layouts = {key: {"cases": 0, "worst": 0.0, "reference_worst": 0.0} for key in REFERENCES}
for text in open(path):
    case = json.loads(text)
    graph = igraph.Graph(n=case["n"], edges=list(zip(case["source"], case["target"])))
    graph.simplify()
    dist = np.array(graph.distances(), dtype=float)
    for key, ours in case["ours"].items():
        dim = REFERENCES[key][2]
        theirs = reference_layout(graph, key, start_layout(case["initial"], dim), case["seed"])
        s_ref = normalised_stress(theirs, dist)
        mine = our_coords(ours)
        if mine is None or theirs.shape[1] != mine.shape[1]:
            continue
        s_our = normalised_stress(mine, dist)
        if s_ref is None or s_our is None:
            continue
        row = layouts[key]
        row["cases"] += 1
        row["worst"] = max(row["worst"], s_our / max(s_ref, FLOOR))
        row["reference_worst"] = max(row["reference_worst"], s_ref)

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"python-igraph {igraph.__version__} (C core {igraph.__igraph_version__})",
    "layouts": layouts,
}
json.dump(result, open(os.path.join(directory, "igraph-result.json"), "w"), indent=1)
print(json.dumps(layouts))
