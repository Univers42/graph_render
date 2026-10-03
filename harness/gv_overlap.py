#!/usr/bin/env python3
"""The Graphviz side of the node-overlap differential: does `-Goverlap=prism` remove overlap,
and how well, on an input with the crowding our pass is measured on?

Runs inside **`ge-graphviz-oracle-gts`** (Graphviz 16.1.0 built **with** the triangulation
library, `docker/graphviz-gts-oracle.Dockerfile`), where `remove_overlap`'s real body is
compiled — `lib/neatogen/adjust.c:778` gates the PRISM entry on `HAVE_GTS && SFDP`.

    docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle-gts \\
        python3 harness/gv_overlap.py --nodes 150 --seed 7 --out target/gv-overlap-150.json

**Never tag `ge-graphviz-oracle` from the GTS file.** The shared tag is the pinned conformance
oracle for every worktree; rebuilding it with GTS moves the sfdp and yifan-hu reference bytes.
The shared `ge-graphviz-oracle` is the **negative control** for this row instead: it has no GTS,
so prism falls back, prints `Overlap value "prism" unsupported - ignored`
(`adjust.c:828`, the `print == 0` placeholder) and emits the identical drawing to `voronoi`.
This script exits non-zero there, which is the point of running it there.

**What is compared, and what is not.** Two numbers per engine:

- **overlapping pairs before and after**, counted by an exhaustive `O(n^2)` scan of the
  `-Tplain` node positions against each node's own `width`. Same predicate our `graph-cli
  overlap` row uses, so the two are on one scale.
- **mean displacement** from the input positions.

**Not** a bitwise match, and cannot be. Graphviz's PRISM is a stress-majorising algorithm on a
Delaunay triangulation; ours is a uniform-grid Jacobi sweep. They will never agree coordinate for
coordinate, and `docs/decisions/node-overlap.md` §8 says so before any number is printed. What
this establishes is a **ceiling**: that prism really removes overlap on this input, and how far
our displacement and our residual sit from it.

**One unit, no conversions.** Every number here is in the unit `-Tplain` prints, which is the
unit `pos` was authored in and the unit `width` is measured in: measured by pinning two nodes 10
apart with `width=6`, which prints canvas `16` and margin `3` — `separation + width` and
`width / 2`. An earlier version multiplied the *input* by 72 and left the output alone, so
`mean_displacement` subtracted points from points-in-another-unit and reported ~500 where the real
number is ~1. It also carried a stale `POINTS_PER_INCH` constant that nothing needed; gone.

**A trap this script exists to state.** With GTS built, `adjustMode[1]` **is** PRISM
(`adjust.c:776-780`) and `getAdjustMode` maps `overlap=false` onto `adjustMode[1]`
(`adjust.c:838-848`) — so in a GTS build `-Goverlap=false` and `-Goverlap=prism` are the same
algorithm and print byte-identical output. Measuring "did prism change anything" by comparing
`prism` against `false` therefore reports *no change* for a prism that is working perfectly.
The liveness check here compares against **`voronoi`**, a different algorithm that must differ,
and it compares **bytes** rather than displacements, because two means printed to 12 places can
agree while the drawings differ. stderr is reported per mode as well: the stub announces itself
there and is silent on stdout, so a harness that keeps only stdout cannot tell a working prism
from a stub — and one that did exactly that reported a stub as live.

**The control that keeps the count honest.** `overlap=true` means "leave the drawing alone", so
its `overlapping_pairs` must come back equal to `input_overlapping_pairs`. If it did not, the
predicate would be measuring nothing and every `0` in the report would be vacuous. Measured: it
comes back equal, and `control_left_input_alone` is asserted here rather than left to the reader.

Ponytail (positions): the input positions are **pinned** (`pos="x,y!"`) and deliberately
overlapping. Neato's own `sep` already keeps its layout clear of a `width`-sized node, so
without pinning the engine would have nothing to remove and the comparison would measure
nothing — that was measured, not assumed. Direction: a pinned input is not a natural neato
drawing, so the stress of the *result* is not comparable with a neato run over a free layout;
only the overlap and displacement columns are. Escape hatch: drop the `!` to let neato lay the
graph out, at the cost of an input with no overlaps to remove.
"""

import argparse
import hashlib
import json
import math
import os
import subprocess
import sys

RADIUS_INCHES = 0.3  # width/2 of the nodes below, in the same unit `pos` and `-Tplain` use


def build_dot(path, n, seed, radius_inches):
    """`n` nodes at random positions in a box far too small for them, pinned and overlapping."""
    import random

    rng = random.Random(seed)
    span = _span(n, radius_inches)
    lines = ["graph G {"]
    lines.append(
        f"  node [shape=circle, width={2 * radius_inches}, height={2 * radius_inches}, "
        "fixedsize=true];"
    )
    for i in range(n):
        x = rng.uniform(0, span)
        y = rng.uniform(0, span)
        lines.append(f'  n{i} [pos="{x:.4f},{y:.4f}!"];')
    lines.append("}")
    with open(path, "w") as out:
        out.write("\n".join(lines) + "\n")
    return span


def input_positions(n, seed, radius_inches):
    """The same positions `build_dot` pinned, in the same order, for the before/after delta."""
    import random

    rng = random.Random(seed)
    span = _span(n, radius_inches)
    return [(rng.uniform(0, span), rng.uniform(0, span)) for _ in range(n)]


def _span(n, radius_inches):
    """The side of the box the `n` discs are dropped into.

    `n` discs of radius `r` need `n pi r^2` of area; at a packing fraction of 0.35 — well
    under the 0.907 a dense disc packing reaches, so the input is genuinely crowded — the box
    is `sqrt(n pi r^2 / 0.35)` on a side.

    **This was wrong once, with a diameter where a radius belonged**, which made the box twice
    as wide as intended and dropped the input's overlapping-pair count to 22 out of 11 175, so
    the whole differential measured a nearly-clear drawing. Radius, not diameter.
    """
    return math.sqrt(n * math.pi * radius_inches**2 / 0.35)


def run_engine(engine, dot_path, mode):
    """`<engine> -Tplain -Goverlap=<mode>` as `(stdout, stderr)`, or exit on a non-zero status.

    `-Gstart` is deliberately **not** passed: the positions are pinned, so the engine's own
    random initial placement is irrelevant and leaving it out keeps the run independent of
    `GM_GV_START`, which `gv_plain.py` reads for a different differential.
    """
    cmd = [engine, "-Tplain", f"-Goverlap={mode}", dot_path]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.exit(f"{engine} -Goverlap={mode} failed on {dot_path}: {proc.stderr}")
    return proc.stdout, proc.stderr


def parse_plain(text):
    """`-Tplain`'s node lines as `(x, y, width)`, in the printed order."""
    nodes = []
    for line in text.splitlines():
        fields = line.split()
        if fields and fields[0] == "node":
            nodes.append((float(fields[2]), float(fields[3]), float(fields[4])))
    return nodes


def count_overlapping(nodes):
    """Unordered pairs whose centres are closer than the sum of their radii — `O(n^2)`.

    The same predicate `graph-cli overlap` uses, so the two counts are on one scale: a
    "Graphviz removed 100 pairs, we removed N" sentence is a sentence about the same thing.
    """
    count = 0
    for i in range(len(nodes)):
        xi, yi, wi = nodes[i]
        for j in range(i + 1, len(nodes)):
            xj, yj, wj = nodes[j]
            if math.hypot(xi - xj, yi - yj) < (wi + wj) / 2.0:
                count += 1
    return count


def mean_displacement(before, after):
    if not before:
        return 0.0
    total = 0.0
    for (bx, by), (ax, ay, _w) in zip(before, after):
        total += math.hypot(bx - ax, by - ay)
    return total / len(before)


def measure(engine, dot_path, before, mode):
    """One engine, one `-Goverlap` mode, in the unit `-Tplain` prints.

    The output's digest is kept because the liveness check compares **bytes**: `prism` and
    `voronoi` print identical drawings exactly when prism has fallen back onto voronoi, and a
    mean displacement would hide that behind a rounded float.
    """
    out, err = run_engine(engine, dot_path, mode)
    nodes = parse_plain(out)
    return {
        "mode": mode,
        "overlapping_pairs": count_overlapping(nodes),
        "mean_displacement": mean_displacement(before, nodes),
        "nodes": len(nodes),
        "output_sha": hashlib.sha256(out.encode()).hexdigest()[:16],
        "warnings": [line for line in err.splitlines() if line.strip()],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nodes", type=int, default=150)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument(
        "--radius", type=float, default=RADIUS_INCHES, help="node radius, in -Tplain's unit"
    )
    parser.add_argument("--engine", default="neato")
    parser.add_argument("--out", default=None, help="write the JSON report here")
    args = parser.parse_args()

    # The node count is in the name: keyed by seed alone, two runs with different `--nodes`
    # and the same seed write the same file, and the second one silently measures the first
    # one's drawing — which is how a `--nodes 1000` row reported 150 nodes in every mode.
    dot_path = os.path.join(
        os.environ.get("GM_SCRATCH", "/tmp"), f"gv-overlap-{args.nodes}-{args.seed}.gv"
    )
    build_dot(dot_path, args.nodes, args.seed, args.radius)
    # No conversion: see the unit note in the module docstring.
    before = input_positions(args.nodes, args.seed, args.radius)
    input_pairs = count_overlapping([(x, y, 2 * args.radius) for x, y in before])

    modes = ["false", "prism", "voronoi", "scale"]
    measured = {mode: measure(args.engine, dot_path, before, mode) for mode in modes}
    # `overlap=true` is "leave the drawing alone": the control that keeps every 0 above
    # meaningful, because if the count moves with no overlap pass running, the predicate is
    # measuring nothing and the row is void.
    control = measure(args.engine, dot_path, before, "true")
    report = {
        "engine": args.engine,
        "nodes": args.nodes,
        "seed": args.seed,
        "radius": args.radius,
        "unit": "the unit -Tplain prints, which is the unit pos and width are in",
        "input_overlapping_pairs": input_pairs,
        "input_span": _span(args.nodes, args.radius),
        "control_leave_alone": control,
        "modes": measured,
    }
    # Two assertions, both computed here rather than left to the reader.
    report["control_left_input_alone"] = control["overlapping_pairs"] == input_pairs
    report["prism_differs_from_voronoi"] = (
        measured["prism"]["output_sha"] != measured["voronoi"]["output_sha"]
    )
    report["prism_is_not_the_stub"] = report["prism_differs_from_voronoi"] and not measured[
        "prism"
    ]["warnings"]

    text = json.dumps(report, indent=2, sort_keys=True)
    print(text)
    if args.out:
        os.makedirs(os.path.dirname(args.out) or "", exist_ok=True)
        with open(args.out, "w") as out:
            out.write(text + "\n")
    # A build without GTS must fail rather than look green — which is what makes the shared
    # no-GTS oracle usable as this row's negative control.
    if not report["control_left_input_alone"]:
        print("FAILED: the no-op control changed the overlap count, so the predicate is void")
        sys.exit(1)
    if not report["prism_is_not_the_stub"]:
        print("FAILED: prism did not differ from voronoi, so this engine has no live prism")
        sys.exit(1)


if __name__ == "__main__":
    main()