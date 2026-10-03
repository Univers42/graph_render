"""twopi's six closed cases and their closed answers — data, and nothing else.

`CLOSED_CASES` is the edge list of each small graph in the shape graph-core's
`probe::graph` builds them (creation order, which is the order the sibling angle sweep
depends on). `CLOSED_ANSWERS` is the answer each one has, node by node, in points,
derived from `lib/twopigen/circle.c`: the centre at the origin, ring `r` at radius `72*r`,
and each subtree's share of `2*PI` in proportion to the leaves below it.

These are tables rather than code, and they are half of `harness/oracle-twopi.py` by line
count, so they live here: that file keeps the arm and this keeps the numbers, and neither
has to grow past the house limit for the other. `gv_frames.py` imports `CLOSED_CASES` from
`oracle-twopi.py` by path (its filename is not importable by name), which this split does
not change — that file still re-exports the name.

The translation onto the drawing's lower-left corner is the one degree of freedom `-Tplain`
cannot show, so every case is compared after each arm has had it applied; that half lives
with the arm, in `oracle-twopi.rendered`.
"""

import math

POINTS_PER_INCH = 72.0

# The plain format prints five significant digits, so the closed-case rendering does too:
# that is the resolution the oracle carries, and comparing at any finer one would grade our
# `f64` against its rounded text.
DIGITS = 5

# Graphviz's default node size in inches, which is what makes the bounding box's lower-left
# corner half a node outside the node-centre bounding box. The fixtures set no `width`,
# `height` or `fixedsize`, so this is the value `-Tplain` used.
NODE_SIZE_INCH = (0.75, 0.5)


def star_angles():
    """The four leaf angles of a star on Graphviz's defaults: 45 + k*90 degrees."""
    return [math.pi / 4 + k * math.pi / 2 for k in range(4)]


# The closed cases, in the shape graph-core's `probe::graph` builds them: an edge list in
# creation order, which is the order the sibling angle sweep depends on.
CLOSED_CASES = {
    "one-node": [],
    "two-nodes": [(0, 1)],
    "three-path": [(0, 1), (1, 2)],
    "four-cycle": [(0, 1), (1, 2), (2, 3), (3, 0)],
    "five-star": [(0, 1), (0, 2), (0, 3), (0, 4)],
    "six-branch": [(0, 1), (0, 2), (0, 3), (2, 4), (4, 5)],
}

# The closed answers, in points, node by node, derived from `lib/twopigen/circle.c`: the
# centre at the origin, ring r at radius 72*r, and each subtree's share of 2*PI in
# proportion to the leaves below it. The translation onto the drawing's lower-left corner is
# the one degree of freedom `-Tplain` cannot show, so every case is compared after each arm
# has had it applied — see `oracle-twopi.rendered`.
CLOSED_ANSWERS = {
    "one-node": [(0.0, 0.0)],
    "two-nodes": [(0.0, 0.0), (-72.0, 0.0)],
    "three-path": [(0.0, -72.0), (0.0, 0.0), (0.0, 72.0)],
    "four-cycle": [(0.0, 0.0), (0.0, 72.0), (0.0, 144.0), (0.0, -72.0)],
    "five-star": [(0.0, 0.0)]
    + [(72.0 * math.cos(a), 72.0 * math.sin(a)) for a in star_angles()],
    "six-branch": [
        (-36.0, -36.0 * math.sqrt(3.0)),
        (-144.0, 0.0),
        (0.0, 0.0),
        (72.0, -72.0 * math.sqrt(3.0)),
        (36.0, 36.0 * math.sqrt(3.0)),
        (72.0, 72.0 * math.sqrt(3.0)),
    ],
}