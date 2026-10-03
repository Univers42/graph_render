"""How far networkx 3.6's own forceatlas2_layout diverges from itself, and how far our port diverges from it, at several iteration budgets. Run in the ge-python-oracle image (networkx 3.6 from its pinned source tarball, numpy 2.3.3):

  graph-cli emit-fa2-fixtures --seeds 200 --out target/fa2-chaos
  docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/fa2-chaos.py target/fa2-chaos
  docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/fa2-chaos.py target/fa2-chaos 2,5,10,20,40,100
  docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/fa2-chaos.py target/fa2-chaos --write-reference

**The budget list must contain the gated budget.** The port's coordinates in the fixtures exist only at the budget the emit ran (`params.max_iter`, `GATED_MAX_ITER` in `crates/graph-cli/src/oracle_python/fa2.rs`), so a list without it measures the reference's chaos and nothing else — every row's `ours_vs_nx` came back "not compared" while the sweep exited 0, the reading nobody asked for. The default list is therefore built at run time from the fixture's own `max_iter`, a requested list without it is refused by name, and each skipped budget names itself.

Each fixture line is rebuilt as an nx.Graph over nodes 0..n-1 (self-loops dropped, parallel edges merged) and given runs of the SAME reference function, from the port's own initial positions with **that line's own** `params` (a file whose lines disagree on them is refused). The arms: `ours`, the coordinates the emit wrote, i.e. the port; `nx`, networkx from the fixture's start; `nx_ulp`, networkx from that start scaled by (1 + 2**-23), one ulp of a float32.

`nx_ulp` is the control the whole metric rests on: the same library, the same code path and the same arithmetic, differing only in a perturbation smaller than the port's own f32 snapshot rounding. Anything `nx_ulp` diverges by is ForceAtlas2 amplifying arithmetic, not the port disagreeing with networkx, and it grows with the budget. The gap per pair is `max |a - b|` over both coordinates over the larger of the two extents — scale-free, and the same normalization the gating harness uses.

The port's arm is comparable only at the budget the fixture ran, so `ours_vs_nx` and the `ours_over_nx_vs_nx` ratio are accumulated **only** there: a worst seed published for a budget the port never ran at is a seed the reader cannot reproduce. At that budget the ratio is "not worse than networkx by more than X", the comparison that survives a chaotic budget, and under 1 says the port tracks the reference more closely than the reference reproduces itself.

`fa2-chaos.json` holds the parameters the reference ran at (so two reports at different parameters are distinguishable), the failed seeds, and per budget the median / p99 / max of both gaps over every seed with the worst seed of each. `--write-reference` instead writes the `nx` arm as `fa2-nx-reference.jsonl` — the pinned reference the Rust test measures a perturbed port against, so the ceiling's negative control needs no networkx at all.

Every line is measured inside a per-case guard, so one malformed line is recorded against its seed, the sweep continues, the report lists the failures and the exit is non-zero. `sys.exit` is a `BaseException`, so a refusal inside a case still aborts the sweep rather than being swallowed as a case failure.

Ponytail: the budgets are measured on the gate model's sizes (2 to 601 nodes); a graph denser or larger is not measured here and its chaos growth is not bounded by this table. The escape hatch is the budget itself: `graph-cli emit-fa2-fixtures --max-iter K` re-measures the whole comparison at any other K.
Ponytail (the `max(chaos, 1e-300)` divisor on the ratio): it turns a self-divergence that is exactly 0 into a huge finite ratio instead of a division by zero. Failing input: a budget at which networkx reproduces its own perturbed run bit for bit, which is a real measurement — the port's gap is then the whole ratio, and the number says so rather than refusing. Direction: it can only over-state the ratio, never under-state it.
Ponytail (the per-case guard): it cannot tell a malformed fixture from a genuine bug in this file, so it records both as a failed seed. Failing input: any seed whose line is truncated, carries a wrong type, or trips a real bug here. Direction: a run over a broken fixture set reports fewer cases than it has seeds and exits non-zero, not exit 0 with a partial table.
"""
import json
import os
import statistics
import sys

import networkx as nx
import numpy as np

# One ulp of a float32, the precision our own coordinates are rounded to on the way out.
EPS = 2.0**-23
# The budgets asked for when none are named. The gated budget is prepended at run time from
# the fixtures' own `params.max_iter`, so the no-argument sweep measures the port at all.
SPREAD_BUDGETS = [5, 10, 20, 40, 100]
PARAMS = ("max_iter", "jitter_tolerance", "scaling_ratio", "gravity")


def default_budgets(ours_iter):
    """The budget list a no-argument run sweeps: the gated budget first, then the spread."""
    return [ours_iter, *[b for b in SPREAD_BUDGETS if b != ours_iter]]


def lines(directory):
    """Every non-blank fixture line's text, in the file's order."""
    with open(os.path.join(directory, "fa2.jsonl")) as handle:
        for text in handle:
            if text.strip():
                yield text


def prepare(text):
    """`(case, graph, start)`: the simple graph the port ran on, and its own start."""
    case = json.loads(text)
    graph = nx.Graph()
    graph.add_nodes_from(range(case["n"]))
    graph.add_edges_from(
        (s, t) for s, t in zip(case["source"], case["target"]) if s != t
    )
    start = np.column_stack([case["initial"]["x"], case["initial"]["y"]]).astype(float)
    return case, graph, start


def seed_of(text):
    """The seed a failed line belongs to, or `None` when it is not even JSON."""
    try:
        return json.loads(text).get("seed")
    except (ValueError, AttributeError):
        return None


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


def skipped_budget(budget, ours_iter):
    """One budget's `ours_vs_nx`, naming the budget it skipped — never a shared dict
    aliased across budgets, which made every row read as the same unmeasured row."""
    return {
        "cases": 0, "median": None, "p99": None, "max": None,
        "skipped": (
            f"not compared: the fixtures ran at max_iter={ours_iter}, so the port has no "
            f"coordinates at budget {budget} to compare"
        ),
    }


def worst_seed(values, seeds):
    """The seed the maximum gap came from: `case["seed"]`, never the list's ordinal."""
    return seeds[int(np.argmax(values))] if values else None


def one_case(case, graph, start, params, budgets, ours_iter):
    """One case's measurements, `{budget: (chaos, ours_gap, ratio)}`.

    `ours_gap` and `ratio` are `None` at every budget but the one the port ran at, so the
    caller cannot accumulate a port gap where there is none. All of it is computed before
    anything is recorded, so a case that fails leaves no partial row behind.
    """
    ours = np.column_stack([case["fa2"]["x"], case["fa2"]["y"]]).astype(float)
    out = {}
    for budget in budgets:
        theirs = run(graph, start, params, budget)
        jittered = run(graph, start * (1.0 + EPS), params, budget)
        chaos = gap(theirs, jittered)
        ours_gap = ratio = None
        if budget == ours_iter:
            ours_gap = gap(ours, theirs)
            # "Not worse than networkx by more than X", the ratio the full-iteration
            # comparison is reported on: ours divided by the same library's own
            # reproducibility at that budget. Below 1 the port tracks the reference
            # more closely than the reference reproduces itself.
            ratio = ours_gap / max(chaos, 1e-300)
        out[budget] = (chaos, ours_gap, ratio)
    return out


def measure(directory, budgets, ours_iter, params_seen):
    """Both gaps per seed, per budget: the samples and the seeds that failed.

    `ours_vs_nx` is accumulated only at `ours_iter` (see the module docstring). `params_seen`
    is filled with the first line's parameters and is the file-wide agreement check; the value
    every case is measured with is `case["params"]`, its own.
    """
    arms = ("nx_vs_nx", "ours_vs_nx", "ratio", "seeds")
    rows = {budget: {arm: [] for arm in arms} for budget in budgets}
    failed = []
    for at, text in enumerate(lines(directory)):
        # The parse is inside the guard too: a line that is not even JSON has to be recorded
        # against its position and the sweep continued, not abort the generator.
        try:
            case, graph, start = prepare(text)
            check_params(case, params_seen)
            measured = one_case(case, graph, start, case["params"], budgets, ours_iter)
        except Exception as error:  # noqa: BLE001 - one bad seed must not end the sweep
            seed = seed_of(text)
            failed.append({
                "line": at, "seed": seed, "error": f"{type(error).__name__}: {error}",
            })
            print(f"line {at} (seed {seed}): {error}", file=sys.stderr)
            continue
        for budget, (chaos, ours_gap, ratio) in measured.items():
            rows[budget]["nx_vs_nx"].append(chaos)
            rows[budget]["seeds"].append(case["seed"])
            if ours_gap is not None:
                rows[budget]["ours_vs_nx"].append(ours_gap)
                rows[budget]["ratio"].append(ratio)
    return rows, failed


def check_params(case, seen):
    """Refuse a fixture file whose lines disagree on the parameters of the reference.

    Each case is measured with its own `params`, not with the first line's applied to all of
    them: a sweep that measured case 7 with case 0's gravity would report a disagreement the
    emit never made, and one that trusted line 0 silently would hide it.
    """
    if not seen:
        seen.append(case["params"])
        return
    if case["params"] != seen[0]:
        sys.exit(
            f"seed {case['seed']}: parameters {json.dumps(case['params'], sort_keys=True)} "
            f"differ from {json.dumps(seen[0], sort_keys=True)} on the first fixture line"
        )


def budget_rows(rows, budgets, ours_iter):
    """Per budget the JSON the doc's table is written from, with each skipped budget's
    `ours_vs_nx` naming itself and the port arm's worst seed unpublished off-budget."""
    out = {}
    for budget in budgets:
        compared = budget == ours_iter
        seeds = rows[budget]["seeds"]
        out[str(budget)] = {
            "nx_vs_nx": summarize(rows[budget]["nx_vs_nx"]),
            "ours_vs_nx": summarize(rows[budget]["ours_vs_nx"]) if compared
            else skipped_budget(budget, ours_iter),
            "ours_over_nx_vs_nx": summarize(rows[budget]["ratio"]) if compared
            else skipped_budget(budget, ours_iter),
            "worst_seed": {
                "nx_vs_nx": worst_seed(rows[budget]["nx_vs_nx"], seeds),
                "ours_vs_nx": worst_seed(rows[budget]["ours_vs_nx"], seeds) if compared
                else None,
            },
        }
    return out


def report(directory, params, budgets, ours_iter):
    """The measurement per budget, as the JSON the doc's table is written from.

    The provenance names the four parameters that determine the reference next to the two
    library versions: two reports with the same versions and a different `gravity` are two
    different measurements, and a table that cannot tell them apart is not reproducible.
    """
    rows, failed = measure(directory, budgets, ours_iter, [])
    result = {
        "fixture_max_iter": ours_iter,
        "params": {key: params[key] for key in PARAMS},
        "eps": EPS,
        "oracle": f"networkx {nx.__version__} forceatlas2_layout on numpy {np.__version__}",
        "cases": len(rows[budgets[0]]["nx_vs_nx"]),
        "failed_seeds": failed,
        "budgets": budget_rows(rows, budgets, ours_iter),
    }
    path = os.path.join(directory, "fa2-chaos.json")
    with open(path, "w") as out:
        json.dump(result, out, indent=1)
    print(json.dumps(result["budgets"], indent=1))
    print(f"wrote {path}; {len(failed)} failed seeds: {[row['seed'] for row in failed]}")
    return 1 if failed else 0


def write_reference(directory):
    """The `nx` arm at the fixture's own budget, one line per seed: what the port is
    compared against, pinned so a Rust test can measure a perturbed port without it."""
    params = params_of(directory)
    ours_iter = params["max_iter"]
    path = os.path.join(directory, "fa2-nx-reference.jsonl")
    with open(path, "w") as out:
        for text in lines(directory):
            case, graph, start = prepare(text)
            theirs = run(graph, start, case["params"], ours_iter)
            out.write(json.dumps({
                "seed": case["seed"], "n": case["n"], "max_iter": ours_iter,
                "x": theirs[:, 0].tolist(), "y": theirs[:, 1].tolist(),
            }) + "\n")
    print(json.dumps({"wrote": path, "max_iter": ours_iter}))
    return 0


def params_of(directory):
    """The reference parameters of the first fixture line; `measure` refuses a file whose
    later lines disagree, so this is the only place that reads one."""
    try:
        return prepare(next(lines(directory)))[0]["params"]
    except StopIteration:
        sys.exit(f"{os.path.join(directory, 'fa2.jsonl')} holds no case")


def main(argv):
    """Refuse a budget list without the gated budget, then sweep or pin the reference."""
    if len(argv) < 2:
        sys.exit("usage: fa2-chaos.py <fixture-dir> [<budgets>] [--write-reference]")
    directory, rest = argv[1], argv[2:]
    if "--write-reference" in rest:
        return write_reference(directory)
    params = params_of(directory)
    ours_iter = params["max_iter"]
    budgets = (
        [int(b) for b in rest[0].split(",")] if rest and rest[0] != "--write-reference"
        else default_budgets(ours_iter)
    )
    if ours_iter not in budgets:
        sys.exit(
            f"no requested budget equals the gated max_iter={ours_iter} (the budget "
            f"graph-cli gates); add it. Budgets requested: {budgets}"
        )
    return report(directory, params, budgets, ours_iter)


if __name__ == "__main__":
    sys.exit(main(sys.argv) or 0)