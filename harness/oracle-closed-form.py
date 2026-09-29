"""Differential of layout.circular.ring, layout.spiral and layout.bipartite against
networkx 3.6's circular_layout, spiral_layout and bipartite_layout, run in the
ge-python-oracle image:

  graph-cli emit-closed-form-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-closed-form.py target/closed-form-fixtures
  graph-cli oracle-closed-form

Metric per layout: the largest absolute coordinate difference (both arms are rescaled to
unit scale). The result holds the worst value per layout; graph-cli holds the ceiling.
The bipartite arm passes networkx our own first column as `nodes=` (see the graph-cli
module's Ponytail) and compares x per node and y as sorted values per column.

Ponytail: the bipartite comparison checks geometry given a partition, not the partition
rule, and not the order of nodes inside a column. The gate model is one connected graph
per seed, so an empty graph, one node and a disconnected graph are not exercised here.
"""
import hashlib, json, os, sys
import numpy as np
import networkx as nx

directory = sys.argv[1]
path = os.path.join(directory, "closed-form.jsonl")
manifest = json.load(open(os.path.join(directory, "closed-form-manifest.json")))
digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
if digest != manifest["sha256"]["closed-form.jsonl"]:
    sys.exit("closed-form.jsonl does not match its manifest")


def block(case, key):
    return np.column_stack([case[key]["x"], case[key]["y"]]).astype(float)


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


layouts = {key: {"cases": 0, "worst": 0.0} for key in ("ring", "spiral", "bipartite")}
for text in open(path):
    case = json.loads(text)
    graph = nx.Graph()
    graph.add_nodes_from(range(case["n"]))
    graph.add_edges_from((s, t) for s, t in zip(case["source"], case["target"]) if s != t)
    metrics = {
        "ring": gap(block(case, "ring"), np.array([nx.circular_layout(graph)[i] for i in range(case["n"])])),
        "spiral": gap(block(case, "spiral"), np.array([nx.spiral_layout(graph)[i] for i in range(case["n"])])),
        "bipartite": bipartite_gap(graph, block(case, "bipartite")),
    }
    for key, metric in metrics.items():
        layouts[key]["cases"] += 1
        layouts[key]["worst"] = max(layouts[key]["worst"], metric)

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"networkx {nx.__version__}, numpy {np.__version__}",
    "layouts": layouts,
}
json.dump(result, open(os.path.join(directory, "closed-form-result.json"), "w"), indent=1)
print(json.dumps(layouts))
