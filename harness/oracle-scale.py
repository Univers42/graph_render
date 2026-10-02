"""Differential of the three `scale.*` rows against SciGraphs' own engine functions, run
in the ge-python-oracle image (numpy 2.3.3, networkx 3.6):

  graph-cli emit-scale-fixtures
  docker run --rm -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-scale.py target/scale-fixtures
  graph-cli oracle-scale

The reference functions are imported and called unchanged; nothing here re-implements
one. Each fixture line carries the arguments of the function its `reference` names, the
motor's answer in the same shape, and — for the coarse-level arm — the map from the
reference's super-node index to the motor's representative.

  lod.apply_budget          the label mask, with the node degree as `order_key` and one
                            node per block, at budgets 0, 1, 2, n, one over a path, and
                            one whose cut lands inside a class of equal degrees
  lod.frustum_cull_spheres  the visible mask, over the square orthographic camera the
                            fixture states, on a rectangle that culls half the nodes and
                            on one whose nodes straddle its sides
  simplify.build_coarse_level
                            the community pass's links, over the same partition on both
                            arms, on a star, a path, two communities joined by one edge,
                            and a self-loop on a collapsed member

Agreement is exact equality: the outputs are masks and index lists, so a failing case is a
mismatch and there is no tolerance to round it away. One thing cannot be byte-compared,
because `np.argsort` is not stable: inside a class of equal `order_key` the reference's
order is the sort's, while the motor breaks ties by ascending dense index (D2). **Whether
a case is one of those is read from the degrees and the budget alone** (`tie_class`), never
from which nodes the reference kept, so the number of compared cases does not move with the
host's sort. Such a case is still compared against the motor, in three parts that between
them pin it (`tie_verdict`): the same kept count, exact agreement on every node outside the
class, and inside it the motor's own stated order.

Also reported rather than compared, because nothing on the motor's side answers them: the
`gaps` below — reference functions the motor has no counterpart for, chiefly
`adaptive.py`'s cut, which needs a hierarchy `build_hierarchy` builds with an infomap
detector, and `simplify.py`'s backbone modes, which the `scale.simplify` row does not port.

`--break` is the negative control, and it refuses to write over the fixtures the honest run
recorded from: pass `--out DIR` with it. `--break=<case>` drops one kept node from that
case's motor mask instead of flipping the whole mask, which is the control a tied case
needs — a tied case still has to keep exactly as many nodes as the budget.

Ponytail: the cull is compared only through an orthographic camera, so it says the
rectangle test agrees and nothing about a perspective divide. The reference also reads
`radii` in two different units in its two culling functions (clip space in
`frustum_cull_spheres`, world units in `projected_pixel_radius`); the fixture states
which of the two it feeds, and the emit refuses a rectangle whose single radius cannot
express all three axes.
"""
import hashlib
import json
import os
import sys

import numpy as np

directory = sys.argv[1]
engine = os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir, "SciGraphs", "engine")
sys.path.insert(0, engine)
from scigraphs_engine import lod as ref_lod  # noqa: E402
from scigraphs_engine import simplify as ref_simplify  # noqa: E402

GAPS = {
    "adaptive.select_cut": "needs a hierarchy of coarse levels, which build_hierarchy "
    "builds with an infomap detector the image does not carry; tick_budget has no "
    "counterpart to be given",
    "simplify.backbone_mask": "the MST/disparity/top-k backbone is not a reversible "
    "reduction and the scale.simplify row does not port it",
    "simplify.build_hierarchy": "same detector as adaptive.select_cut; the motor has no "
    "coarse level, only a journal",
    "lod.node_tier": "thresholds are on-screen pixels of a 1080-tall image and a headless "
    "motor has none; the tier comes from the node count instead",
    "lod.projected_pixel_radius": "reads radii in world units where frustum_cull_spheres "
    "reads them in clip space, so no single fixture satisfies both",
}

path = os.path.join(directory, "scale.jsonl")
manifest = json.load(open(os.path.join(directory, "scale-manifest.json")))
digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
if digest != manifest["sha256"]["scale.jsonl"]:
    sys.exit("scale.jsonl does not match its manifest")


def option(flag):
    """The value of `--flag`, or of `--flag=value`, or None when neither is there."""
    if flag in sys.argv[2:]:
        return ""
    prefix = flag + "="
    for arg in sys.argv[2:]:
        if arg.startswith(prefix):
            return arg[len(prefix):]
    return None


# `--break` rewrites the motor's recorded answer, which is the negative control the row
# needs: a differential that stopped comparing anything would still go red. It refuses to
# write into the fixtures directory, because the run that writes there is the one
# `graph-cli oracle-scale` records its verdict from.
break_case = option("--break")
out = option("--out")
if out is None:
    if break_case is not None:
        sys.exit("--break writes a deliberate mismatch: give it --out DIR of its own")
    out = directory
os.makedirs(out, exist_ok=True)


def budget_mask(inp):
    """lod.apply_budget over the fixture's order key, as a 0/1 column."""
    kept = ref_lod.apply_budget(
        np.asarray(inp["order_key"], dtype=np.float64),
        np.asarray(inp["counts"], dtype=np.float64),
        inp["budget"],
    )
    return [int(b) for b in kept]


def cull_mask(inp):
    """lod.frustum_cull_spheres over the fixture's camera, as a 0/1 column."""
    kept = ref_lod.frustum_cull_spheres(
        np.asarray(inp["centers"], dtype=np.float64),
        np.asarray(inp["radii"], dtype=np.float64),
        np.asarray(inp["persp_mat"], dtype=np.float64),
    )
    return [int(b) for b in kept]


def coarse_links(inp, mapping):
    """simplify.build_coarse_level's super-edges, read in the motor's index space.

    The reference returns None when the labels give fewer than two groups or one per node
    (`simplify.py:170-171`); that is the same answer as no links, because a community step
    whose representative is its only member removes nothing.
    """
    out = ref_simplify.build_coarse_level(
        coords=np.asarray(inp["coords"], dtype=np.float64),
        edges=np.asarray(inp["edges"], dtype=np.int64),
        labels=np.asarray(inp["labels"], dtype=np.int64),
        colors=np.asarray(inp["colors"], dtype=np.float64),
        weights=None,
        base_radius=inp["base_radius"],
    )
    if out is None:
        return []
    # np.unique put the super-nodes in ascending label order, which is the order the
    # fixture's map is written in.
    reps = [rep for _, rep in mapping]
    links = {
        (min(reps[int(a)], reps[int(b)]), max(reps[int(a)], reps[int(b)]))
        for a, b in zip(out["se_src"], out["se_dst"])
    }
    return [list(pair) for pair in sorted(links)]


def tie_class(key, counts, budget):
    """The nodes of the class of equal order keys the budget's cut falls inside, or None.

    Decided from the degrees, the block counts and the budget alone — never from which
    nodes the reference kept, which would make the number of compared cases a property of
    the host's `np.argsort` rather than of the fixtures.

    `apply_budget` (`lod.py:81-85`) sorts descending and keeps the longest prefix whose
    cumulative count fits, so the cut is between positions `kept - 1` and `kept`. The class
    of the key at `kept - 1` is where the two arms may disagree, and it disagrees only when
    it is not wholly inside one side of the cut: `low < kept <= high`.
    """
    if budget <= 0 or not key:
        return None
    order = sorted(range(len(key)), key=lambda node: -key[node])
    at = {node: pos for pos, node in enumerate(order)}
    spent, kept = 0.0, 0
    for pos, node in enumerate(order):
        if spent + counts[node] > budget:
            break
        spent += counts[node]
        kept = pos + 1
    cls = [node for node in range(len(key)) if key[node] == key[order[kept - 1]]]
    return cls if min(at[n] for n in cls) < kept <= max(at[n] for n in cls) else None


def tie_verdict(cls, theirs, ours):
    """Why `ours` is not one of the masks the budget's rule admits, or None if it is.

    Three checks, and only the middle one is about equality:

    1. the same number of nodes kept — the budget is the rule's, and the tie is about
       *which* nodes, never how many;
    2. exact agreement on every node outside `cls`, where the two arms have the same
       order and must have the same answer;
    3. inside `cls`, the motor's own stated order (D2, ascending dense index), which is
       exactly that the kept members are the class's lowest `n` indices.
    """
    if sum(ours) != sum(theirs):
        return f"kept {sum(ours)} nodes against the budget's {sum(theirs)}"
    outside = [n for n in range(len(ours)) if n not in cls]
    if any(ours[n] != theirs[n] for n in outside):
        return f"disagrees outside the tie class {cls}"
    kept = sorted(n for n in cls if ours[n] == 1)
    if kept != sorted(cls)[:len(kept)]:
        return f"inside {cls} it kept {kept}, not the lowest dense indices"
    return None


def broken_mask(case):
    """The motor's recorded answer with one node dropped, for `--break=<case>`.

    A tied case is the one that needs this: flipping a whole mask is caught by the same
    arithmetic as any other case, so the control that matters is one only the tie check can
    notice — a mask that keeps fewer nodes than the budget. A case with no mask (the
    coarse-level links) has no kept node to drop and returns `None`.
    """
    mask = case["ours"].get("mask")
    if not mask:
        return None
    dropped = list(mask)
    dropped[max(i for i, v in enumerate(dropped) if v == 1)] = 0
    return dropped


layouts = {key: {"emitted": 0, "cases": 0, "exact": 0, "ties": 0, "worst": 0.0}
           for key in ("apply_budget", "frustum_cull_spheres", "build_coarse_level")}
ties = []
broken = None
for text in open(path):
    case = json.loads(text)
    name = case["reference"].split(".")[-1]
    if name not in layouts:
        sys.exit(f"{case['case']}: unknown reference {case['reference']}")
    ours = case["ours"]["mask"] if "mask" in case["ours"] else case["ours"]["links"]
    row = layouts[name]
    row["emitted"] += 1
    if break_case is not None and break_case in ("", case["case"]) and broken is None:
        broken = case["case"]
        ours = broken_mask(case) or [1 - v for v in ours]
    if name == "build_coarse_level":
        theirs = coarse_links(case["input"], case["map"]["community"])
    else:
        theirs = budget_mask(case["input"]) if name == "apply_budget" else cull_mask(case["input"])
    cls = None
    if name == "apply_budget":
        cls = tie_class(case["input"]["order_key"], case["input"]["counts"],
                        case["input"]["budget"])
    why = tie_verdict(cls, theirs, ours) if cls is not None else (
        None if theirs == ours else "the masks differ")
    if why is None:
        row["cases"] += 1
        if cls is None:
            row["exact"] += 1
        else:
            row["ties"] += 1
            ties.append({"case": case["case"], "reference": theirs, "motor": ours,
                         "tie_class": cls})
            print(f"TIED {case['case']}: the cut falls inside {cls} — reference {theirs} "
                  f"against motor {ours}, and the reference's order inside it is its own")
    else:
        row["worst"] += 1
        print(f"MISMATCH {case['case']}: {why} — reference {theirs} against motor {ours}")

for key, row in layouts.items():
    if row["ties"]:
        print(f"  {key}: {row['ties']} case(s) whose cut is inside a class of equal "
              f"degrees, compared against the rule rather than the reference's order")
if break_case and not broken:
    sys.exit(f"--break={break_case} names no case in {path}")

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"SciGraphs scigraphs_engine lod/simplify on numpy {np.__version__}, exact",
    # `tolerance: false` and the `closed` section are the house's way of saying the arms
    # returned the same bytes rather than two numbers inside a measured ceiling: graph-cli's
    # `closed_cases` refuses a `closed_exact: false` outright.
    "tolerance": False,
    # keyed by function, and only over the cases the two arms returned the same bytes for:
    # a tied case agrees with the rule, not with the reference's arbitrary order in it.
    "closed": {key: row["exact"] for key, row in layouts.items()},
    "closed_exact": not any(row["worst"] for row in layouts.values()),
    "layouts": layouts,
    "ties": ties,
    "gaps": GAPS,
    "broken": broken,
}
json.dump(result, open(os.path.join(out, "scale-result.json"), "w"), indent=1)
print(json.dumps(layouts))
