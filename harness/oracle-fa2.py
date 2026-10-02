"""Differential of layout.forceatlas2 against networkx 3.6's forceatlas2_layout, run in the
ge-python-oracle image (networkx 3.6 from its pinned source tarball, numpy 2.3.3):

  graph-cli emit-fa2-fixtures --seeds 1000
  docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/oracle-fa2.py target/fa2-fixtures
  graph-cli oracle-fa2

Each seed's graph is rebuilt as an nx.Graph over nodes 0..n-1 in order (self-loops
dropped, parallel edges merged: the port runs on the same simple graph), and the reference
starts from the port's own initial positions with the port's parameters, networkx's
defaults otherwise. The metric per seed is max |ours - theirs| over every coordinate,
divided by the reference layout's extent (the larger side of its bounding box), so it is
scale-free. The result holds the worst and the median; graph-cli holds the ceiling.

Two layouts are compared per seed, each against its own start: ``fa2`` at ``dim = 2``
(``initial``) and ``fa2_3d`` at ``dim = 3`` (``initial_3d``). They get separate keys and
separate ceilings because they are separate pictures from separate starts — one key
standing for both would let one arm be measured by the other's run. networkx takes the
dimension from the width of the ``pos`` it is handed (``layout.py:1698-1700``), so a
three-column start *is* the ``dim = 3`` call.

Ponytail: the 3D arm's ceiling is its own measurement, not the 2D one widened. Both are
recorded in ``docs/measurements/p12-t4b.md``.

Ponytail: ours arrive rounded to f32 by the snapshot, a floor near 1e-7 of the extent. The
iteration budget in the fixtures is small on purpose, and it is the only honest one: FA2 is
chaotic, so the differential gates at the largest budget at which networkx still
reproduces itself to under 1e-6 from a start perturbed by one float32 ulp
(`harness/fa2-chaos.py`; the full table, and the full-100-iteration comparison that gates
nothing, in `docs/measurements/fa2-chaos.md`). Past that budget a coordinate gap measures
the dynamics amplifying arithmetic rather than this port disagreeing with the reference.
"""
import hashlib, json, os, sys
import numpy as np
import networkx as nx

directory = sys.argv[1]
path = os.path.join(directory, "fa2.jsonl")
manifest = json.load(open(os.path.join(directory, "fa2-manifest.json")))
digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
if digest != manifest["sha256"]["fa2.jsonl"]:
    sys.exit("fa2.jsonl does not match its manifest")

def measure(case, key, start_key, params_key):
    """Worst |ours - theirs| / extent(theirs) for one arm, against its own start."""
    graph = nx.Graph()
    graph.add_nodes_from(range(case["n"]))
    graph.add_edges_from((s, t) for s, t in zip(case["source"], case["target"]) if s != t)
    start = case[start_key]
    axes = [a for a in ("x", "y", "z") if a in start]
    pos = {i: np.array([start[a][i] for a in axes]) for i in range(case["n"])}
    p = case[params_key]
    theirs = nx.forceatlas2_layout(
        graph, pos=pos, max_iter=p["max_iter"], jitter_tolerance=p["jitter_tolerance"],
        scaling_ratio=p["scaling_ratio"], gravity=p["gravity"],
    )
    theirs = np.array([theirs[i] for i in range(case["n"])], dtype=float)
    ours = np.column_stack([case[key][a] for a in axes]).astype(float)
    extent = float((theirs.max(axis=0) - theirs.min(axis=0)).max())
    return float(np.abs(ours - theirs).max()) / max(extent, 1e-300)


gaps = {"fa2": [], "fa2_3d": []}
for text in open(path):
    case = json.loads(text)
    gaps["fa2"].append(measure(case, "fa2", "initial", "params"))
    gaps["fa2_3d"].append(measure(case, "fa2_3d", "initial_3d", "params_3d"))

layouts = {name: {
    "cases": len(gaps[name]),
    "worst": max(gaps[name]) if gaps[name] else None,
    "median": float(np.median(gaps[name])) if gaps[name] else None,
    "worst_seed": int(np.argmax(gaps[name])) if gaps[name] else None,
} for name in gaps}
result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"networkx {nx.__version__} forceatlas2_layout on numpy {np.__version__}",
    "layouts": layouts,
}
json.dump(result, open(os.path.join(directory, "fa2-result.json"), "w"), indent=1)
print(json.dumps(layouts))
