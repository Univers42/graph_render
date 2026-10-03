"""Differential of layout.circular.hierarchy against SciGraphs' own
_circular_hierarchy_layout (SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:693-732),
run in the ge-python-oracle image with the SciGraphs/ submodule mounted so the arm is the
reference function itself and not a restatement of it:

  graph-cli emit-circular-hierarchy-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-circular-hierarchy.py target/circular-hierarchy-fixtures
  graph-cli oracle-circular-hierarchy

Each seed's graph is rebuilt as an nx.Graph over nodes 0..n-1 in order (self-loops dropped,
parallel edges merged: the port runs on the same undirected simple graph) and handed to the
reference at the fixture's own `scale`, which is SciGraphs' dispatcher default of 5.0.
Node insertion order and adjacency order therefore agree with the port's dense index and
its CSR rows, which is what makes the ring-by-ring comparison node by node.

Metric: the largest absolute coordinate difference over both axes. The layout is a closed
form, so agreement is a tolerance and nothing weaker; the result holds the worst per layout
and also how many seeds matched bit for bit after the snapshot's f32 narrowing, and
graph-cli holds the ceiling.

Ponytail: the arm is handed an nx.Graph, never an nx.DiGraph. The port reads a directed
input undirected, because the motor's Topology carries directedness per edge and has no
whole-graph flag, so SciGraphs' in-degree-0 roots and its three-biggest-fan-outs fallback
(hierarchical.py:705-711) are not ported and are not compared. The gate model is one
connected random graph per seed, so a single node, an empty graph and a disconnected one
are not exercised here; graph-core's own tests hold those against the reference's printed
answers. The reference prints a progress line per call, so it is silenced rather than left
to interleave with the result.
"""
import contextlib
import io
import json
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import networkx as nx  # noqa: E402
import numpy as np  # noqa: E402

from oracle_common import (  # noqa: E402
    finite,
    read_manifest,
    require_cases,
    require_seeds,
)

sys.path.insert(0, os.path.join("SciGraphs", "core"))
from scigraphs_core.mesh.layouts.hierarchical import (  # noqa: E402
    _circular_hierarchy_layout,
)

directory = sys.argv[1]
name = "circular-hierarchy"
manifest, digest = read_manifest(directory, name)
with open(os.path.join(directory, f"{name}.jsonl")) as handle:
    lines = handle.readlines()
require_seeds(manifest, lines, name)


def theirs_of(case):
    """The reference's own answer over the fixture's graph, at the fixture's own scale."""
    graph = nx.Graph()
    graph.add_nodes_from(range(case["n"]))
    graph.add_edges_from(
        (s, t) for s, t in zip(case["source"], case["target"]) if s != t
    )
    with contextlib.redirect_stdout(io.StringIO()):
        positions = _circular_hierarchy_layout(graph, case["scale"])
    return np.asarray(positions[:, :2], dtype=float)


cases = len(lines)
worst = 0.0
worst_seed = None
exact = 0
for text in lines:
    case = json.loads(text)
    ours = np.column_stack([case[name]["x"], case[name]["y"]]).astype(float)
    theirs = theirs_of(case)
    gap = float(np.abs(ours - theirs).max())
    # `>` is false for a NaN, so the guard is the metric and not the comparison: without it a
    # NaN coordinate leaves `worst` at its 0.0 initialiser and the case reports as a match.
    if finite(gap, "gap") > worst:
        worst, worst_seed = gap, case["seed"]
    # Bit-for-bit after the snapshot's own f32 narrowing: ours arrives f32, so the
    # comparison is on both arms narrowed to f32. np.cos and libm::cos differ in the last
    # bits, so this is a count and not a gate — the gate is the tolerance above.
    if np.array_equal(ours.astype(np.float32), theirs.astype(np.float32)):
        exact += 1

layouts = {
    name: {
        "cases": cases,
        "worst": worst,
        "worst_seed": worst_seed,
        "f32_bit_exact": exact,
    }
}
require_cases(layouts, (name,), name)
result = {
    # Checked by `verdict()` in `crates/graph-cli/src/oracle_python.rs:189-190`, which
    # refuses when this is not `stamp.fingerprint()`; read_manifest binds the fixture bytes.
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"SciGraphs _circular_hierarchy_layout on networkx {nx.__version__}, "
    f"numpy {np.__version__}",
    "layouts": layouts,
}
with open(os.path.join(directory, f"{name}-result.json"), "w") as out:
    json.dump(result, out, indent=1)
print(json.dumps(layouts))
