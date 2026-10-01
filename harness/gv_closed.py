"""The analytically determined cases, and the one uniform rescale both arms go through.

`CIRCO_CASES` is the closed-case table for `circo`: fourteen small graphs whose answer
`lib/circogen` determines outright, in the shape graph-core's `probe::graph` builds them.
Each case is `(edges, answer)`, and the answer is either a circle — `(n, (slot per node))`,
so the assertion is the *order* and the radius is one formula — or the measured points, so
the assertion is the drawing. They are pinned node by node, independently, in
`crates/graph-core/src/layout/graphviz/circo/tests.rs`; this file checks them against what
`circo -Tplain` prints.

The rescale is what the sweep's metric runs on: both arms are put onto Graphviz's own
node-centre bounding box with **one** uniform `max` scale, which removes exactly a
translation and a scale — the two degrees of freedom that say nothing about a layout. One
uniform scale rather than one per axis: a per-axis map would let an aspect error and a shape
error arrive as the same unscaled difference, and it survives the degenerate axis a circular
layout produces on a 2-node graph, which a per-axis divide does not.
"""

import math

from gv_plain import POINTS_PER_INCH, printed_nodes

# The plain format prints five significant digits, so the closed-case rendering prints five
# too: that is the resolution the oracle carries, and finer would grade our `f64` against
# its text.
DIGITS = 5

# Graphviz's default node size in inches. `-Tplain` translates the drawing so the *node
# boxes* start at the origin, so the whole offset from the closed answer's frame to the
# oracle's is half a node outside the node-centre bounding box.
NODE_SIZE_INCH = (0.75, 0.5)

# `min_dist + largest_node` in points — one node's slot on a circle — so every radius below
# is `count * SLOT / 2*PI` and circo's whole circle table is one number and some cosines.
SLOT = 72.0 * 1.75

# This port's own frame for the bowtie: the left tip sits here, where the oracle's frame puts
# it at the origin. The same constant is in the layout's tests, and neither is derived from
# the other — both were read off a `-Tplain` run.
TIP = -159.1606


def ring(count):
    """A `count`-node circle's radius in points: `count * SLOT / 2*PI` (`blockpath.c:594`)."""
    return count * SLOT / (2.0 * math.pi)


def slot(count, at):
    """Point `at` of a `count`-node circle: `at * 2*PI / count` round the radius."""
    theta = at * 2.0 * math.pi / count
    return (ring(count) * math.cos(theta), ring(count) * math.sin(theta))


CIRCO_CASES = {
    "one-node": ([], [0.0, 0.0]),
    "two-nodes": ([(0, 1)], [-SLOT / 2, 0.0, SLOT / 2, 0.0]),
    "three-path": ([(0, 1), (1, 2)], [-SLOT, 0.0, 0.0, 0.0, SLOT, 0.0]),
    "five-path": (
        [(0, 1), (1, 2), (2, 3), (3, 4)],
        [(-2 * SLOT, 0.0), (-SLOT, 0.0), (0.0, 0.0), (SLOT, 0.0), (2 * SLOT, 0.0)],
    ),
    "five-star": (
        [(0, 1), (0, 2), (0, 3), (0, 4)],
        [(0.0, 0.0), (SLOT, 0.0), (0.0, SLOT), (-SLOT, 0.0), (0.0, -SLOT)],
    ),
    # The circles, by slot order: a 4-cycle starts at its last node, a 5-cycle at its own
    # root, a 6-cycle at its last, a chorded 5-cycle like a plain one, `K4` at its first leaf
    # and `K5` neither way round. `K4` and `K5` are also the cases whose skeleton pass splits
    # the tree, so they are the ones that exercise `place_residual_nodes`.
    "four-cycle": ([(0, 1), (1, 2), (2, 3), (3, 0)], (4, (3, 2, 1, 0))),
    "triangle": ([(0, 1), (1, 2), (2, 0)], (3, (2, 1, 0))),
    "five-cycle": ([(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)], (5, (4, 3, 2, 1, 0))),
    "six-cycle": (
        [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0)],
        (6, (5, 4, 3, 2, 1, 0)),
    ),
    "chorded-five": ([(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (0, 2)], (5, (4, 3, 2, 1, 0))),
    "complete-four": (
        [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2), (1, 3)],
        (4, (3, 0, 1, 2)),
    ),
    "complete-five": (
        [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (0, 2), (1, 4), (2, 4), (3, 0)],
        (5, (4, 0, 2, 3, 1)),
    ),
    # Two blocks and a cut point, coalesced — not a circle, so the measured layout. `TIP` is
    # this port's frame, where the oracle's has the left tip at the origin; a translation,
    # and exactly what the rescale removes.
    "bowtie": (
        [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 2)],
        [
            (TIP, 0.0),
            (TIP + SLOT, 0.0),
            (TIP + 255.0816, -52.1006),
            (TIP + 255.0816, 52.1006),
            (TIP + 345.32, 0.0),
        ],
    ),
    "glued-stars": (
        [(0, 1), (0, 2), (1, 3), (1, 4), (2, 5)],
        [
            (0.0, 0.0),
            (2 * SLOT, 0.0),
            (-1.5 * SLOT, 0.0),
            (2 * SLOT, -SLOT),
            (2 * SLOT, SLOT),
            (-2.5 * SLOT, 0.0),
        ],
    ),
}

# Which engines have closed cases. An engine with none still gets a sweep and no
# `closed_exact`, and `graph-cli oracle-graphviz` then says nothing about byte agreement.
CLOSED = {"circo": CIRCO_CASES}


def answer_of(shape):
    """A case's expected points: a circle's slots become angles, anything else is as written."""
    if isinstance(shape, tuple):
        count, slots = shape
        return [slot(count, at) for at in slots]
    if shape and isinstance(shape[0], tuple):
        return list(shape)
    return [(shape[k], shape[k + 1]) for k in range(0, len(shape), 2)]


def bbox_of(points):
    """A point cloud's bounding box: `((min_x, min_y), (width, height))`."""
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    return (min(xs), min(ys)), (max(xs) - min(xs), max(ys) - min(ys))


def rescale(points, box):
    """`points` onto `box`, one uniform scale from the larger axis's span."""
    (min_x, min_y), (width, height) = box
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    span = max(max(xs) - min(xs), max(ys) - min(ys))
    if span <= 0.0:
        return [(p[0] - min_x, p[1] - min_y) for p in points]
    scale = max(width, height) / span
    low_x, low_y = min(xs), min(ys)
    return [((x - low_x) * scale, (y - low_y) * scale) for x, y in zip(xs, ys)]


def gap(ours, theirs):
    """The largest absolute coordinate difference in points, on one shared box."""
    box = bbox_of(theirs)
    mine, other = rescale(ours, box), rescale(theirs, box)
    return max(max(abs(a[0] - b[0]), abs(a[1] - b[1])) for a, b in zip(mine, other))


def rendered(points):
    """The closed answer as the node lines `-Tplain` would print for it."""
    (low_x, low_y), _ = bbox_of(points)
    off_x = NODE_SIZE_INCH[0] / 2.0 - low_x / POINTS_PER_INCH
    off_y = NODE_SIZE_INCH[1] / 2.0 - low_y / POINTS_PER_INCH
    return " ".join(
        f"{x / POINTS_PER_INCH + off_x:.{DIGITS}g} {y / POINTS_PER_INCH + off_y:.{DIGITS}g}"
        for x, y in points
    )


def closed_case(engine, tmp, name, edges, answer):
    """One closed case: the engine's arm rendered against the closed answer, exactly."""
    count = 1 + max((max(edge) for edge in edges), default=0)
    theirs = printed_nodes(engine, tmp, f"closed-{name}", count, edges)
    got = " ".join(f"{x} {y}" for x, y in theirs)
    want = rendered(answer)
    return {"nodes": count, "exact": want == got, "want": want, "got": got}