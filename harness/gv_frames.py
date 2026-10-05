"""The closed cases whose answers are already in the frame `-Tplain` prints.

`gv_closed.py` renders a closed answer from the layout's own frame, applying the half-node
offset `-Tplain` translates the drawing by, because that is where `circo`'s answers are
written. An engine whose **port keeps Graphviz's own translation** needs no offset: its
answers are printed as they stand. `osage` is the one such engine here — `osageinit.c:198-220`
translates the drawing so its lower-left corner is the origin and takes the node coordinates
with it, so `n0` of a one-node graph is at half a node box, (27, 18) points.

The graph each case is drawn from is **imported from `harness/oracle-twopi.py`**, by path:
its filename is not importable by name, and these six graphs are twopi's, already defined
once there. Copying them would be a second definition of the graphs the comparison is about.

`sys.dont_write_bytecode` is set across the import, so the fingerprinted `harness/` tree never
gains the `__pycache__` a path import would write — the same reason `oracle-twopi.py` gives
for importing this harness.
"""

import importlib.util
import os
import sys

from gv_closed import DIGITS
from gv_plain import POINTS_PER_INCH, dot_path, graph_of, printed_nodes

# The `osage` closed answers, in points, **in the frame `-Tplain` prints**. The arithmetic is
# `arrayRects`'s — a near-square grid of `58 x 40` point cells filled row by row — and each
# case is derived and pinned beside the derivation in graph-core's
# `layout/graphviz/osage/tests.rs`, so this table is checked, not assumed.
FRAMED_CLOSED = {
    "osage": {
        "one-node": [(27.0, 18.0)],
        "two-nodes": [(27.0, 18.0), (85.0, 18.0)],
        "three-path": [(27.0, 58.0), (85.0, 58.0), (27.0, 18.0)],
        "four-cycle": [(27.0, 58.0), (85.0, 58.0), (27.0, 18.0), (85.0, 18.0)],
        "five-star": [
            (27.0, 58.0), (85.0, 58.0), (143.0, 58.0), (27.0, 18.0), (85.0, 18.0),
        ],
        "six-branch": [
            (27.0, 58.0), (85.0, 58.0), (143.0, 58.0),
            (27.0, 18.0), (85.0, 18.0), (143.0, 18.0),
        ],
    },
    # fdp: what the pinned 16.1.0 printed for `fdp -Tplain -Gstart=1`, stable over four runs each
    # (`docs/measurements/p13-gv2-fdp.md`); not derived, since fdp is iterative.
    "fdp": {
        "one-node": [(27.0, 18.0)],
        "two-nodes": [(27.0, 27.77616), (92.4336, 18.0)],
        "three-path": [
            (27.0, 113.73119999999999),
            (86.5008, 66.41856),
            (88.10640000000001, 18.0),
        ],
        "four-cycle": [
            (27.0, 64.51344),
            (130.3704, 70.97976),
            (92.988, 18.0),
            (64.38528000000001, 117.4896),
        ],
        "five-star": [
            (94.4496, 67.94712),
            (160.76160000000002, 53.830079999999995),
            (53.31744, 18.0),
            (27.0, 114.0552),
            (124.9272, 145.7352),
        ],
        "six-branch": [
            (83.8296, 77.3208),
            (169.07760000000002, 18.0),
            (87.15599999999999, 25.269840000000002),
            (27.0, 124.0416),
            (157.5, 103.75200000000001),
            (134.35920000000002, 164.5488),
        ],
    },
    # dot: the six closed cases as the pinned 16.1.0 `dot -Tplain` printed them, in the frame
    # `-Tplain` prints, byte for byte at five significant digits
    # (`docs/measurements/p13-gv2-dot.md`, "The six closed cases"). The arithmetic is the
    # reference's own conventions — node `0.75 x 0.5` inch, `nodesep` 0.25, `ranksep` 0.5, so
    # ranks and same-rank neighbours are 72 points apart — and every row was re-measured here
    # rather than copied, so the table is checked rather than restated.
    #
    # **The 6-branch row is the seven-edge graph's answer, and `framed_cases` draws the
    # five-edge one.** `twopi_closed.CLOSED_CASES["six-branch"]` is
    # `n0--n1, n0--n2, n0--n3, n2--n4, n4--n5`; the measurement's table is the seven-edge
    # graph (it drops `n2--n4` and `n3--n4` and draws `n4` under `n2`). The two print the
    # **same** coordinates — measured, `dot -Tplain` on both — because `n2` sits directly
    # above `n4` in the five-edge graph, so the two lost constraints move nothing. So this row
    # is the answer to the graph the harness draws, and the agreement is measured rather than
    # assumed; the port's own `dot/order_tests.rs` names the same collision.
    "dot": {
        "one-node": [(27.0, 18.0)],
        "two-nodes": [(27.0, 90.0), (27.0, 18.0)],
        "three-path": [(27.0, 162.0), (27.0, 90.0), (27.0, 18.0)],
        "four-cycle": [(54.0, 234.0), (27.0, 162.0), (27.0, 90.0), (54.0, 18.0)],
        "five-star": [
            (135.0, 90.0), (27.0, 18.0), (99.0, 18.0), (171.0, 18.0), (243.0, 18.0),
        ],
        "six-branch": [
            (99.0, 234.0), (27.0, 162.0), (99.0, 162.0),
            (171.0, 162.0), (99.0, 90.0), (99.0, 18.0),
        ],
    },
    # sfdp: one case only, closed because a single node has one position, the centre of the
    # default 0.75 x 0.5 inch box; measured identical at `-Gstart` 1, 7 and 99. The other five
    # peer cases are deliberately absent: this engine is seed-sensitive, so each prints three
    # different answers at those seeds and no closed answer exists for them
    # (`docs/measurements/p13-gv2-sfdp.md`). `framed_cases` reports them as skipped.
    "sfdp": {"one-node": [(27.0, 18.0)]},
}


def peer_cases():
    """`harness/oracle-twopi.py`'s closed-case graphs, by path, cached.

    `CLOSED_CASES` is the edge list of each of the six small graphs; `DIGITS` and
    `POINTS_PER_INCH` come from the same module so the printed precision is one number.
    """
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "oracle-twopi.py")
    previous = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec = importlib.util.spec_from_file_location("oracle_twopi", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module.CLOSED_CASES


def framed_case(engine, tmp, case, start=None):
    """One closed case: the engine's printed node lines against the answer, exactly.

    `case` is `(name, edges, answer)`, as in `gv_closed.closed_case`, so the call takes four
    parameters rather than six.
    """
    name, edges, answer = case
    count = 1 + max((max(edge) for edge in edges), default=0)
    theirs = printed_nodes(engine, dot_path(tmp, f"closed-{name}"), graph_of(count, edges), start)
    want = " ".join(f"{x / POINTS_PER_INCH:.{DIGITS}g} {y / POINTS_PER_INCH:.{DIGITS}g}"
                    for x, y in answer)
    got = " ".join(f"{x} {y}" for x, y in theirs)
    return {"nodes": count, "exact": want == got, "want": want, "got": got}


def framed_cases(engine, tmp, answers, start=None):
    """`(<compared by name>, <peer names with no restated answer>)` for one engine.

    The second list is the point of the signature. A peer case with no answer in this
    engine's table was compared by nothing, and `oracle-graphviz.py` folds `exact` with
    `all()` over whatever survived — so `sfdp`, whose only closed answer is `one-node`
    (`docs/measurements/p13-gv2-sfdp.md`: it is seed-sensitive, so the other five print three
    different answers at `-Gstart` 1, 7 and 99 and no closed answer exists for them), was
    carrying its byte-agreement verdict on one node out of six. Naming the skips makes the
    verdict's weight visible in the result file instead of only in a printed count; an engine
    that skipped every case is refused, because then it compared nothing at all.
    """
    peers = peer_cases()
    skipped = sorted(name for name in peers if name not in answers)
    if peers and not [name for name in peers if name in answers]:
        sys.exit(f"{engine}: no framed closed case has a restated answer")
    compared = {
        name: framed_case(engine, tmp, (name, edges, answers[name]), start)
        for name, edges in peers.items()
        if name in answers
    }
    return compared, skipped