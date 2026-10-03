#!/usr/bin/env python3
"""The Graphviz side of the node-overlap differential: does `-Goverlap=prism` remove overlap,
and how well, on the same input our pass sees?

Runs inside the `ge-graphviz-oracle` image (Graphviz 16.1.0 built **with** the triangulation
library, so `remove_overlap`'s real body is compiled — `lib/neatogen/overlap.c:17` gates it on
`HAVE_GTS && SFDP`). Without GTS every `-Goverlap` value hits the stub that prints
"not built with triangulation library", so the whole comparison would be against a no-op.

    docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \\
        python3 harness/gv_overlap.py --nodes 150 --seed 7 --out target/gv-overlap

**What is compared, and what is not.** Two numbers per engine, on the same pinned positions:

- **overlapping pairs before and after**, counted by an exhaustive `O(n^2)` scan of the
  `-Tplain` node positions against each node's own `width`. Same predicate our `graph-cli
  overlap` row uses, so the two are on one scale.
- **mean displacement** from the input positions, in points.

**Not** a bitwise match, and cannot be. Graphviz's PRISM is a stress-majorising algorithm on a
Delaunay triangulation; ours is a uniform-grid Jacobi sweep. They will never agree coordinate for
coordinate, and `docs/decisions/node-overlap.md` §8 says so before any number is printed. What
this establishes is a **ceiling**: that prism really removes overlap on this input, and how far
our displacement and our residual sit from it.

**A trap this script exists to state.** With GTS built, `adjustMode[1]` **is** PRISM
(`lib/neatogen/adjust.c:775-800`) and `getAdjustMode` maps `overlap=false` onto `adjustMode[1]`
(`adjust.c:846-849`) — so in a GTS build `-Goverlap=false` and `-Goverlap=prism` are the same
algorithm and print byte-identical output. Measuring "did prism change anything" by comparing
`prism` against `false` therefore reports *no change* for a prism that is working perfectly.
The liveness check here compares against **`voronoi`** instead, which is a different algorithm
and must differ; and against the no-GTS build, which leaves the stub.

Ponytail (positions): the input positions are **pinned** (`pos="x,y!"`) and deliberately
overlapping. Neato's own `sep` already keeps its layout clear of a `width`-sized node, so
without pinning the engine would have nothing to remove and the comparison would measure
nothing — that was measured, not assumed. Direction: a pinned input is not a natural neato
drawing, so the stress of the *result* is not comparable with a neato run over a free layout;
only the overlap and displacement columns are. Escape hatch: drop the `!` to let neato lay the
graph out, at the cost of an input with no overlaps to remove.
"""

import argparse
import json
import math
import os
import subprocess
import sys

POINTS_PER_INCH = 72.0
RADIUS_INCHES = 0.3  # width/2 of the nodes below; the disc radius in points is this * 72


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
    """The side of the box the `n` discs are dropped into, in inches.

    `n` discs of radius `r` need `n pi r^2` of area; at a packing fraction of 0.35 — well
    under the 0.907 a dense disc packing reaches, so the input is genuinely crowded — the box
    is `sqrt(n pi r^2 / 0.35)` on a side.

    **This was wrong once, with a diameter where a radius belonged**, which made the box twice
    as wide as intended, dropped the input's overlapping-pair count to 22 out of 11 175, and
    made the whole differential measure a nearly-clear drawing. Radius, not diameter.
    """
    return math.sqrt(n * math.pi * radius_inches**2 / 0.35)


def run_engine(engine, dot_path, mode):
    """`<engine> -Tplain -Goverlap=<mode>`, or the engine's complaint and no answer.

    `-Gstart` is deliberately **not** passed: the positions are pinned, so the engine's own
    random initial placement is irrelevant and leaving it out keeps the run independent of
    `GM_GV_START`, which `gv_plain.py` reads for a different differential.
    """
    cmd = [engine, "-Tplain", f"-Goverlap={mode}", dot_path]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        noise = [ln for ln in proc.stderr.splitlines() if ln.strip()]
        sys.exit(f"{engine} -Goverlap={mode} failed on {dot_path}: {proc.stderr}")
    return proc.stdout


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
    "Graphviz removed 3756 pairs, we removed N" sentence is a sentence about the same thing.
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
    nodes = parse_plain(run_engine(engine, dot_path, mode))
    return {
        "mode": mode,
        "overlapping_pairs": count_overlapping(nodes),
        "mean_displacement": mean_displacement(before, nodes),
        "nodes": len(nodes),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nodes", type=int, default=150)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument(
        "--radius", type=float, default=RADIUS_INCHES, help="node radius in inches"
    )
    parser.add_argument("--engine", default="neato")
    parser.add_argument("--out", default=None, help="write the JSON report here")
    args = parser.parse_args()

    dot_path = os.path.join(os.environ.get("GM_SCRATCH", "/tmp"), f"gv-overlap-{args.seed}.gv")
    build_dot(dot_path, args.nodes, args.seed, args.radius)
    before_inches = input_positions(args.nodes, args.seed, args.radius)
    # `-Tplain` prints points, the DOT file says inches: the before/after delta has to be in
    # the same unit or it is a number about nothing.
    before = [(x * POINTS_PER_INCH, y * POINTS_PER_INCH) for x, y in before_inches]

    modes = ["false", "prism", "voronoi", "scale"]
    report = {
        "engine": args.engine,
        "nodes": args.nodes,
        "seed": args.seed,
        "radius_inches": args.radius,
        "input_overlapping_pairs": count_overlapping(
            [(x, y, args.radius * 2 * POINTS_PER_INCH) for x, y in before]
        ),
        "input_span_inches": _span(args.nodes, args.radius),
        "modes": {mode: measure(args.engine, dot_path, before, mode) for mode in modes},
    }
    # The liveness check, stated rather than assumed. `voronoi` is a different algorithm, so a
    # build where prism still ran the stub would show prism == voronoi == false; this reports
    # whether prism moved the drawing at all.
    report["prism_differs_from_voronoi"] = (
        report["modes"]["prism"]["mean_displacement"]
        != report["modes"]["voronoi"]["mean_displacement"]
    )
    report["prism_is_not_the_stub"] = report["prism_differs_from_voronoi"]

    text = json.dumps(report, indent=2, sort_keys=True)
    print(text)
    if args.out:
        os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
        with open(args.out, "w") as out:
            out.write(text + "\n")


if __name__ == "__main__":
    main()