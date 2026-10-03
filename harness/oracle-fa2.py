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

The three refusals are `oracle_common`'s and shared with every other arm here: the
fixture bytes must match the manifest (and the digest recorded is the one computed over
the bytes this run read), the line count must be the manifest's seed count, and a case
set of zero is refused rather than reported as `cases: 0`. On top of those a degenerate
reference drawing — every node on one point, so the extent it normalises by is exactly 0
— is refused by name, because dividing by a floored 1e-300 there reports a 1e300 gap as
an ordinary number instead of as the degeneracy it is.

Ponytail: ours arrive rounded to f32 by the snapshot, a floor near 1e-7 of the extent. The
iteration budget in the fixtures is small on purpose, and it is the only honest one: FA2 is
chaotic, so the differential gates at the largest budget at which networkx still
reproduces itself to under 1e-6 from a start perturbed by one float32 ulp
(`harness/fa2-chaos.py`; the full table, and the full-100-iteration comparison that gates
nothing, in `docs/measurements/fa2-chaos.md`). Past that budget a coordinate gap measures
the dynamics amplifying arithmetic rather than this port disagreeing with the reference.
Ponytail (the degenerate-extent refusal): it fires on `extent <= 0.0` exactly, so it cannot
mis-refuse a real drawing, but it also cannot measure one — a reference that collapsed onto
a point is a disagreement about the algorithm, not a coordinate gap, and this arm has no
number for it. Direction: it refuses rather than reporting, so the sweep stops instead of
ranking a degenerate seed.
"""
import json
import os
import sys

sys.dont_write_bytecode = True

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import networkx as nx
import numpy as np

from oracle_common import finite, read_manifest, require_cases, require_seeds

USAGE = "usage: oracle-fa2.py <fixtures-dir>"


ARMS = (("fa2", "initial", "params"), ("fa2_3d", "initial_3d", "params_3d"))


def reference(case, start_key, params_key):
    """The reference layout for one arm: networkx from the port's own start and params. The
    start's width picks the dimension, as networkx reads it from `pos`."""
    graph = nx.Graph()
    graph.add_nodes_from(range(case["n"]))
    graph.add_edges_from((s, t) for s, t in zip(case["source"], case["target"]) if s != t)
    start = case[start_key]
    axes = [a for a in ("x", "y", "z") if a in start]
    pos = {i: np.array([start[a][i] for a in axes]) for i in range(case["n"])}
    params = case[params_key]
    theirs = nx.forceatlas2_layout(
        graph, pos=pos, max_iter=params["max_iter"],
        jitter_tolerance=params["jitter_tolerance"],
        scaling_ratio=params["scaling_ratio"], gravity=params["gravity"],
    )
    return np.array([theirs[i] for i in range(case["n"])], dtype=float), axes


def gap_of(case, key, start_key, params_key):
    """The one metric: `max |ours - theirs|` over every coordinate, over theirs' extent."""
    theirs, axes = reference(case, start_key, params_key)
    ours = np.column_stack([case[key][a] for a in axes]).astype(float)
    extent = float((theirs.max(axis=0) - theirs.min(axis=0)).max())
    if extent <= 0.0:
        sys.exit(f"seed {case['seed']}: the {key} reference drawing has extent {extent}")
    return finite(float(np.abs(ours - theirs).max()) / extent, f"{key} gap")


def layout_of(key, gaps, seeds):
    """One layout the Rust judge reads: `cases`, `worst`, `median`, and the seed the worst
    came from. `worst_seed` is `case["seed"]` at the argmax, not the line's ordinal."""
    worst_at = int(np.argmax(gaps))
    return {
        "cases": len(gaps),
        "worst": finite(max(gaps), f"{key} worst"),
        "median": finite(float(np.median(gaps)), f"{key} median"),
        "worst_seed": seeds[worst_at],
    }


def main(argv):
    """Read the fixtures, measure one gap per seed and arm, and write the result the judge reads."""
    if len(argv) != 2:
        sys.exit(USAGE)
    directory = argv[1]
    manifest, digest = read_manifest(directory, "fa2")
    with open(os.path.join(directory, "fa2.jsonl")) as handle:
        cases = [json.loads(line) for line in handle]
    require_seeds(manifest, cases, "fa2")
    seeds = [case["seed"] for case in cases]
    gaps = {arm[0]: [gap_of(case, *arm) for case in cases] for arm in ARMS}
    require_cases({key: {"cases": len(gaps[key])} for key in gaps}, tuple(gaps), "fa2")
    layouts = {key: layout_of(key, gaps[key], seeds) for key in gaps}
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": digest,
        "oracle": f"networkx {nx.__version__} forceatlas2_layout on numpy {np.__version__}",
        "layouts": layouts,
    }
    with open(os.path.join(directory, "fa2-result.json"), "w") as out:
        json.dump(result, out, indent=1)
    print(json.dumps(layouts))


if __name__ == "__main__":
    main(sys.argv)