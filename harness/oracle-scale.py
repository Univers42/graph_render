"""Differential of the three `scale.*` rows against SciGraphs' own engine functions, run
in the ge-python-oracle image (numpy 2.3.3, networkx 3.6):

  graph-cli emit-scale-fixtures
  docker run --rm -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-scale.py fixtures/scale
  graph-cli oracle-scale

The reference functions are imported and called unchanged; nothing here re-implements
one. Each fixture line carries the arguments of the function its `reference` names, the
motor's answer in the same shape, and — for the coarse-level arm — the map from the
reference's super-node index to the motor's representative.

  lod.apply_budget          the label mask, with the node degree as `order_key` and one
                            node per block, at budgets 0, 1, 2, n and one over a path
  lod.frustum_cull_spheres  the visible mask, over the square orthographic camera the
                            fixture states, on a rectangle that culls half the nodes and
                            on one whose nodes straddle its sides
  simplify.build_coarse_level
                            the community pass's links, over the same partition on both
                            arms, on a star, a path, two communities joined by one edge,
                            and a self-loop on a collapsed member

Agreement is exact equality: the outputs are masks and index lists, so a failing case is a
mismatch and there is no tolerance to round it away. Two things are reported rather than
compared, because the reference cannot decide them:

  ties   `np.argsort` is not stable, so `apply_budget`'s order inside a class of equal
         `order_key` is the sort's, while the motor breaks ties by ascending dense index
         (D2). A case whose cut falls inside such a class is listed with both masks.
  gaps   reference functions the motor has no counterpart for: `adaptive.py`'s cut, which
         needs a hierarchy `build_hierarchy` builds with an infomap detector, and
         `simplify.py`'s backbone modes, which the `scale.simplify` row does not port.

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

# `--break` flips one case's expected answer in the motor's favour: the negative control
# the row needs, so a differential that stopped comparing anything would still go red.
BREAK = "--break" in sys.argv[2:]


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


def straddled(key, mask):
    """Whether the budget's cut falls inside a class of equal order keys.

    If the smallest kept key is not strictly above the largest dropped one, any order
    within that class gives the same budget and a different mask, so the reference's
    `np.argsort` — which is not stable — is deciding the answer rather than the rule.
    """
    kept = [k for k, m in zip(key, mask) if m]
    dropped = [k for k, m in zip(key, mask) if not m]
    return bool(kept) and bool(dropped) and min(kept) <= max(dropped)


layouts = {key: {"cases": 0, "worst": 0.0, "ties": 0} for key in ("apply_budget",
                                                                 "frustum_cull_spheres",
                                                                 "build_coarse_level")}
ties = []
broken = None
for text in open(path):
    case = json.loads(text)
    name = case["reference"].split(".")[-1]
    if name not in layouts:
        sys.exit(f"{case['case']}: unknown reference {case['reference']}")
    ours = case["ours"]["mask"] if "mask" in case["ours"] else case["ours"]["links"]
    if broken is None and BREAK:
        broken = case["case"]
        ours = [1 - v for v in ours]
    if name == "build_coarse_level":
        theirs = coarse_links(case["input"], case["map"]["community"])
    else:
        theirs = budget_mask(case["input"]) if name == "apply_budget" else cull_mask(case["input"])
    if theirs == ours:
        layouts[name]["cases"] += 1
    elif name == "apply_budget" and straddled(case["input"]["order_key"], theirs):
        # The rule is the same on both sides; only the order inside the tie is not.
        layouts[name]["ties"] += 1
        ties.append({"case": case["case"], "reference": theirs, "motor": ours})
        print(f"UNDECIDED {case['case']}: the cut falls inside a tie — reference {theirs} "
              f"against motor {ours}, and neither order is wrong")
    else:
        layouts[name]["worst"] += 1
        print(f"MISMATCH {case['case']}: reference {theirs} against motor {ours}")

for key, row in layouts.items():
    if row["ties"]:
        print(f"  {key}: {row['ties']} case(s) the reference cannot decide, listed under ties")

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"SciGraphs scigraphs_engine lod/simplify on numpy {np.__version__}, exact",
    # `tolerance: false` and the `closed` section are the house's way of saying the arms
    # returned the same bytes rather than two numbers inside a measured ceiling: graph-cli's
    # `closed_cases` refuses a `closed_exact: false` outright.
    "tolerance": False,
    "closed": {key: row["cases"] for key, row in layouts.items()},
    "closed_exact": not any(row["worst"] for row in layouts.values()),
    "layouts": layouts,
    "ties": ties,
    "gaps": GAPS,
    "broken": broken,
}
json.dump(result, open(os.path.join(directory, "scale-result.json"), "w"), indent=1)
print(json.dumps(layouts))
