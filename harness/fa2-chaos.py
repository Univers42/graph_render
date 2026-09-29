"""How far networkx 3.6's own forceatlas2_layout diverges from itself, and how far our
port diverges from it, at several iteration budgets. Run in the ge-python-oracle image
(networkx 3.6 from its pinned source tarball, numpy 2.3.3):

  graph-cli emit-fa2-fixtures --seeds 200 --out target/fa2-chaos
  docker run --rm -v $PWD:/w -w /w ge-python-oracle \\
    python3 harness/fa2-chaos.py target/fa2-chaos 5,10,20,40,100
  docker run --rm -v $PWD:/w -w /w ge-python-oracle \\
    python3 harness/fa2-chaos.py target/fa2-chaos --write-reference

Each fixture line is rebuilt as an nx.Graph over nodes 0..n-1 (self-loops dropped,
parallel edges merged) and given runs of the SAME reference function, from the port's own
initial positions with the port's parameters:

  `ours`   the coordinates the emit wrote, i.e. the port;
  `nx`     networkx from the fixture's start;
  `nx_ulp` networkx from that start scaled by (1 + 2**-23), one ulp of a float32.

At the fixture's own budget the port's gap is also reported as a ratio over `nx_ulp`'s:
"not worse than networkx by more than X" is the comparison that survives a chaotic budget,
and a ratio under 1 says the port tracks the reference more closely than the reference
reproduces itself.

`nx_ulp` is the control the whole metric rests on: the same library, the same code path
and the same arithmetic, differing only in a perturbation smaller than the port's own f32
snapshot rounding. Anything `nx_ulp` diverges by is ForceAtlas2 amplifying arithmetic, not
the port disagreeing with networkx, and it grows with the iteration budget. The gap per
pair is max |a - b| over both coordinates over the larger of the two extents —
scale-free, and the same normalization the gating harness uses.

The port's arm is only comparable at the budget the fixture ran (`params.max_iter`, the
gated one); at any other budget the row is reported as not measured, because comparing a
100-iteration port against a 10-iteration reference measures the budget, not the port.
Written to `fa2-chaos.json`: per budget the median / p99 / max of both gaps over every
seed, and the worst seed of each. `--write-reference` instead writes the `nx` arm as
`fa2-nx-reference.jsonl` — the pinned reference the Rust test measures a perturbed port
against, so the ceiling's negative control needs no networkx at all.

Ponytail: the budgets are measured on the gate model's sizes (2 to 601 nodes); a graph
denser or larger is not measured here and its chaos growth is not bounded by this table.
The escape hatch is the budget itself: `graph-cli emit-fa2-fixtures --max-iter K`
re-measures the whole comparison at any other K.
"""
import json
import os
import statistics
import sys

import networkx as nx
import numpy as np

# One ulp of a float32, the precision our own coordinates are rounded to on the way out.
EPS = 2.0**-23
DEFAULT_BUDGETS = "5,10,20,40,100"


def cases(directory):
    """`(case, graph, start)` per fixture line, in the file's order."""
    for text in open(os.path.join(directory, "fa2.jsonl")):
        case = json.loads(text)
        graph = nx.Graph()
        graph.add_nodes_from(range(case["n"]))
        graph.add_edges_from(
            (s, t) for s, t in zip(case["source"], case["target"]) if s != t
        )
        start = np.column_stack([case["initial"]["x"], case["initial"]["y"]]).astype(float)
        yield case, graph, start


def run(graph, start, params, max_iter):
    """One reference run, as an n x 2 array in node order."""
    pos = {i: start[i].copy() for i in range(graph.number_of_nodes())}
    out = nx.forceatlas2_layout(
        graph, pos=pos, max_iter=max_iter,
        jitter_tolerance=params["jitter_tolerance"],
        scaling_ratio=params["scaling_ratio"], gravity=params["gravity"],
    )
    return np.array([out[i] for i in range(graph.number_of_nodes())])


def gap(a, b):
    """`max |a - b|` over both coordinates, over the larger of the two extents."""
    extent = max(
        float((a.max(axis=0) - a.min(axis=0)).max()),
        float((b.max(axis=0) - b.min(axis=0)).max()),
    )
    return float(np.abs(a - b).max()) / max(extent, 1e-300)


def summarize(values):
    """`(cases, median, p99, max)` of a list of gaps."""
    if not values:
        return {"cases": 0, "median": None, "p99": None, "max": None}
    ordered = sorted(values)
    return {
        "cases": len(ordered),
        "median": statistics.median(ordered),
        "p99": ordered[min(len(ordered) - 1, int(0.99 * len(ordered)))],
        "max": ordered[-1],
    }


def fixture_params(directory):
    """The parameters the emit wrote, from its first line."""
    case = next(cases(directory))[0]
    return case["params"]


def measure(directory, params, budgets, ours_iter):
    """Both gaps per seed, per budget: the samples, and the worst seed of each arm."""
    rows = {b: {"nx_vs_nx": [], "ours_vs_nx": [], "ratio": []} for b in budgets}
    worst = {b: {"nx_vs_nx": [-1.0, None], "ours_vs_nx": [-1.0, None]} for b in budgets}
    for case, graph, start in cases(directory):
        ours = np.column_stack([case["fa2"]["x"], case["fa2"]["y"]]).astype(float)
        for budget in budgets:
            theirs = run(graph, start, params, budget)
            jittered = run(graph, start * (1.0 + EPS), params, budget)
            chaos = gap(theirs, jittered)
            rows[budget]["nx_vs_nx"].append(chaos)
            if budget == ours_iter:
                rows[budget]["ours_vs_nx"].append(gap(ours, theirs))
                # "Not worse than networkx by more than X", the ratio the full-iteration
                # comparison is reported on: ours divided by the same library's own
                # reproducibility at that budget. Below 1 the port tracks the reference
                # more closely than the reference reproduces itself.
                rows[budget]["ratio"].append(gap(ours, theirs) / max(chaos, 1e-300))
            for arm, value in (("nx_vs_nx", chaos), ("ours_vs_nx", gap(ours, theirs))):
                if value > worst[budget][arm][0]:
                    worst[budget][arm] = [value, case["seed"]]
    return rows, worst


def report(directory, params, budgets, ours_iter):
    """The measurement per budget, as the JSON the doc's table is written from."""
    rows, worst = measure(directory, params, budgets, ours_iter)
    not_compared = {
        "cases": 0, "median": None, "p99": None, "max": None,
        "note": f"not compared: the fixtures ran at max_iter={ours_iter}",
    }
    out = {
        "fixture_max_iter": ours_iter,
        "eps": EPS,
        "oracle": f"networkx {nx.__version__} forceatlas2_layout on numpy {np.__version__}",
        "budgets": {
            str(b): {
                "nx_vs_nx": summarize(rows[b]["nx_vs_nx"]),
                "ours_vs_nx": summarize(rows[b]["ours_vs_nx"]) if b == ours_iter
                else not_compared,
                "ours_over_nx_vs_nx": summarize(rows[b]["ratio"]) if b == ours_iter
                else not_compared,
                "worst_seed": {arm: worst[b][arm][1] for arm in worst[b]},
            }
            for b in budgets
        },
    }
    path = os.path.join(directory, "fa2-chaos.json")
    json.dump(out, open(path, "w"), indent=1)
    print(json.dumps(out["budgets"], indent=1))
    print(f"wrote {path}")


def write_reference(directory, params):
    """The `nx` arm at the fixture's own budget, one line per seed: what the port is
    compared against, pinned so a Rust test can measure a perturbed port without it."""
    ours_iter = params["max_iter"]
    path = os.path.join(directory, "fa2-nx-reference.jsonl")
    with open(path, "w") as out:
        for case, graph, start in cases(directory):
            theirs = run(graph, start, params, ours_iter)
            out.write(json.dumps({
                "seed": case["seed"], "n": case["n"], "max_iter": ours_iter,
                "x": theirs[:, 0].tolist(), "y": theirs[:, 1].tolist(),
            }) + "\n")
    print(json.dumps({"wrote": path, "max_iter": ours_iter}))


def main(argv):
    if len(argv) < 2:
        sys.exit(f"usage: fa2-chaos.py <fixture-dir> [{DEFAULT_BUDGETS}] [--write-reference]")
    directory = argv[1]
    rest = argv[2:]
    params = fixture_params(directory)
    if "--write-reference" in rest:
        return write_reference(directory, params)
    budgets = [int(b) for b in rest[0].split(",")] if rest and rest[0] != "--write-reference" \
        else [int(b) for b in DEFAULT_BUDGETS.split(",")]
    return report(directory, params, budgets, params["max_iter"])


if __name__ == "__main__":
    main(sys.argv)
