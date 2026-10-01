"""The closed-case answers `harness/oracle-graphviz.py` grades, per engine.

Split out by the house's 300-line limit, and because these tables are *data* while the file
that reads them is the engine plumbing and the comparison: an engine with no table here has
its closed cases compared by its own harness rather than skipped, which is a decision about
coverage rather than about how the harness works.

A closed case is a coordinate pair held as an exact literal, against which the engine's own
printed `-Tplain` text is compared character for character. That is only meaningful for an
engine whose answer is a deterministic function of the graph, so the table's contents are a
claim about the engine, not a formatting convenience.
"""

# The `osage` closed answers, in points, **in the frame `-Tplain` prints**.
#
# `osageinit.c:198-220` translates the drawing so its lower-left corner is the origin and
# takes the node coordinates with it, so `n0` of a one-node graph is at half a node box,
# (27, 18) points = (0.375, 0.25) inch. No offset is applied on either side here: the port
# keeps that translation, so our arm and the oracle's printed text are directly comparable.
# The arithmetic is `arrayRects`'s — a near-square grid of `58 x 40` point cells filled row
# by row — and each case is derived and pinned beside the derivation in graph-core's
# `layout/graphviz/osage/tests.rs`. An engine with no table here has its closed cases
# compared by its own harness rather than skipped.
OSAGE_CLOSED = {
    "one-node": [(27.0, 18.0)],
    "two-nodes": [(27.0, 18.0), (85.0, 18.0)],
    "three-path": [(27.0, 58.0), (85.0, 58.0), (27.0, 18.0)],
    "four-cycle": [(27.0, 58.0), (85.0, 58.0), (27.0, 18.0), (85.0, 18.0)],
    "five-star": [(27.0, 58.0), (85.0, 58.0), (143.0, 58.0), (27.0, 18.0), (85.0, 18.0)],
    "six-branch": [
        (27.0, 58.0), (85.0, 58.0), (143.0, 58.0),
        (27.0, 18.0), (85.0, 18.0), (143.0, 18.0),
    ],
}

#
# **`neato` is deliberately absent, and its absence is the point.** A closed case here is a
# coordinate pair we hold as an exact literal and want the engine's printed text to match
# character for character. neato has no such literals to hold: it is iterative and its
# drawing is a function of `-Gstart`, so its answers are not derivable by reading the
# source — they are two independent runs compared. Its five small cases are token-exact and
# its sixth agrees to the fourth significant digit, and that comparison lives where the two
# runs are, in graph-core's own tests and `docs/measurements/p13-gv2-neato.md`, not here.
# Pinning six numbers measured from the oracle into a table this file compares the oracle
# against would grade the oracle against itself.
CLOSED = {"osage": OSAGE_CLOSED}
