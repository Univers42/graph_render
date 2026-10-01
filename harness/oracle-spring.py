"""The networkx arm of the layout.force.spring differential: SciGraphs' SPRING, which is
networkx 3.6's own spring_layout at dim=2 (networkx_layouts.py:16-24), run in the
ge-python-oracle image with the SciGraphs/ submodule mounted so the seed it is handed is
SciGraphs' own derived one:

  graph-cli emit-spring-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-spring.py target/spring-fixtures
  graph-cli oracle-spring

Each seed's graph is rebuilt as an nx.Graph over nodes 0..n-1 in order (self-loops dropped,
parallel edges merged: the port runs on the same undirected simple graph) and handed to
nx.spring_layout with the fixture's own parameters. One line is written per case, in the
fixtures' own order: {"seed", "n", "spring": {"x", "y"}}. The line is the reference's
*positions* and nothing else.

It computes no metric. The stress correlation is applied to both arms by graph-core
(crates/graph-cli/src/stress/metric.rs), once, exactly as harness/stress-d3.mjs leaves it
to graph-core for the d3 arm: a metric restated here as well would be two
implementations whose disagreement would be indistinguishable from a disagreement between
two layouts. graph-cli owns the deficit, the distribution and the ceiling.

Why not a coordinate gap, which is what the FA2 arm gates: FA2 hands networkx the port's
own initial positions, so the two arms differ only in arithmetic. spring_layout picks its
own start, ours is graph-core's seeded Mulberry32 and networkx's is numpy's
RandomState, so the two diverge inside the first step and a gap would measure the chaos
rather than the port. The metric that survives that is whether each drawing respects the
graph's own hop distances (oracle_python/spring.rs holds the argument and the ceiling).

Ponytail: the reference's method="auto" (layout.py:140-141) is the force branch under 500
nodes and an L-BFGS energy minimiser at or above it; the port is the force branch only, so
a seed at or above 500 compares two different algorithms. The harness says nothing about
that split -- it does not know which branch ran -- and the Rust half reports the worst case
below 500 beside the worst overall precisely because this arm cannot. The gate model is
one connected graph per seed, so an empty or disconnected graph is never exercised here.
"""
import contextlib
import hashlib
import io
import json
import os
import sys

sys.path.insert(0, os.path.join("SciGraphs", "core"))
import networkx as nx  # noqa: E402
import numpy as np  # noqa: E402

from scigraphs_core.repro.determinism import get_layout_seed  # noqa: E402

directory = sys.argv[1]
name = "spring"
path = os.path.join(directory, f"{name}.jsonl")
theirs_path = os.path.join(directory, f"{name}-theirs.jsonl")
manifest = json.load(open(os.path.join(directory, f"{name}-manifest.json")))
digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
if digest != manifest["sha256"][f"{name}.jsonl"]:
    sys.exit(f"{name}.jsonl does not match its manifest")


def theirs_of(case):
    """The reference's own answer over the fixture's graph, at the fixture's parameters."""
    graph = nx.Graph()
    graph.add_nodes_from(range(case["n"]))
    graph.add_edges_from(
        (s, t) for s, t in zip(case["source"], case["target"]) if s != t
    )
    p = case["params"]
    # Once per seed, not once per node: spring_layout returns dict(zip(G, pos)) and
    # get_layout_seed() is a pure function of the pipeline seed, so every call returns the
    # same mapping and indexing entry i of a fresh call reconstructs it exactly -- at n
    # times the cost, which over 1000 seeds is hours instead of minutes. `graph` inserts
    # nodes 0..n-1 in order, so that insertion order is the dense index the fixture's x/y
    # columns are in and the values line up node for node.
    with contextlib.redirect_stdout(io.StringIO()):
        pos = nx.spring_layout(
            graph,
            dim=2,
            iterations=p["iterations"],
            threshold=p["threshold"],
            scale=p["scale"],
            seed=get_layout_seed(),
        )
    return np.asarray([pos[i] for i in range(case["n"])], dtype=float)


cases = 0
with open(theirs_path, "w") as out:
    for text in open(path):
        case = json.loads(text)
        theirs = theirs_of(case)
        out.write(
            json.dumps(
                {
                    "seed": case["seed"],
                    "n": case["n"],
                    name: {"x": theirs[:, 0].tolist(), "y": theirs[:, 1].tolist()},
                }
            )
            + "\n"
        )
        cases += 1

if cases == 0:
    sys.exit("no cases in the fixtures: a differential over nothing proves nothing")

print(json.dumps({"cases": cases, "out": theirs_path, "oracle":
                  f"networkx {nx.__version__} spring_layout(dim=2) on numpy {np.__version__}"}))
