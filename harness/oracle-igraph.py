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
    return np.array(getattr(graph, method)(**kwargs).coords, dtype=float)


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
    start = list(zip(case["initial"]["x"], case["initial"]["y"]))
    for key, ours in case["ours"].items():
        theirs = reference_layout(graph, key, start, case["seed"])
        s_ref = normalised_stress(theirs, dist)
        s_our = normalised_stress(np.column_stack([ours["x"], ours["y"]]).astype(float), dist)
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
