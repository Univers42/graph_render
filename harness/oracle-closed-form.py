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

if len(sys.argv) != 2:
    sys.exit("usage: oracle-closed-form.py <fixtures-dir>")
fixtures_dir = sys.argv[1]
networkx_version(nx)
manifest, digest = read_manifest(fixtures_dir, "closed-form")
# One open, one pass: the digest and the measurement read the same handle's bytes. Two
# independent opens left a window in which an emit that ran while this arm was reading
# produced a result whose `sha256` covered fewer bytes than it measured.
with open(os.path.join(fixtures_dir, "closed-form.jsonl")) as handle:
    lines = handle.readlines()
require_seeds(manifest, lines, "closed-form")


def block(case, key):
    return np.column_stack([case[key]["x"], case[key]["y"]]).astype(float)


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


layouts = {key: {"cases": 0, "worst": 0.0} for key in ("ring", "spiral", "bipartite")}
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
    }
    for key, metric in metrics.items():
        layouts[key]["cases"] += 1
        # `finite` before the accumulator: `max` is false for a NaN, so a NaN gap would leave
        # `worst` at its 0.0 initialiser and the case would report as a perfect match.
        layouts[key]["worst"] = max(
            layouts[key]["worst"], finite(metric, f"{key} gap")
        )

require_cases(layouts, ("ring", "spiral", "bipartite"), "closed-form")

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"networkx {nx.__version__}, numpy {np.__version__}",
    "layouts": layouts,
}
with open(os.path.join(sys.argv[1], "closed-form-result.json"), "w") as out:
    json.dump(result, out, indent=1)
print(json.dumps(layouts))