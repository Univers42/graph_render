#!/usr/bin/env python3
"""The `dot` layering and ordering probe: one row per seed, ranks, within-rank order and the
printed coordinate of every node.

`-Tplain` prints a coordinate per node and no rank, no order and no crossing count, so both
are *derived* from the printed coordinates, in this order:

1. **rank.** `set_ycoords` (`position.c:773-786`) stacks the ranks downwards from rank 0 at
   the top, one `(node height + ranksep)` = 72-point step apart, and every fixture node has
   the default `0.75 x 0.5` inch box. So `rank = round((y_max - y) / 72)` with `y_max` the
   largest printed y. The run reports the largest distance any printed y sits off that grid,
   so the derivation is measured rather than assumed; it is 0.0000 of a step, which is what
   makes the rank column the oracle's own layering.
2. **order.** Within one rank the nodes are sorted by their printed x, ties by name. Two
   nodes of one rank cannot share an x in a drawing Graphviz produced, so the tie-break
   makes the sort total (D5) rather than because it fires.

The digest is one line per seed:

    seed n  t,h t,h ...  <n ranks>  <n order>  <n xs>  <n ys>

where `order` is the per-rank left-to-right node lists concatenated, rank 0 first, so a
reader splits the line into `4 * n` trailing fields after the edges: the ranks, the order,
then the two coordinate columns, in that order and each `n` tokens wide. The last two are
the **inch strings `-Tplain` printed** per node, in node-index order `n0, n1, ...`, carried
verbatim: the consumer compares them byte for byte against the plain format's own precision,
so re-printing a parsed float here would grade our arithmetic against the oracle's rounding
rather than against its text. `dot/oracle_probe.rs` parses exactly this, and the 1000-seed
order sweep asserts against it.

Run in the ge-graphviz-oracle image, which carries both the engine and python3:

    scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
        python3 harness/oracle-dot-probe.py target/dotfix --fixtures=dot.jsonl \
        --digest target/probe/dot1000.txt

Every container goes through `scripts/orch/drun`: it adds the memory cap and the cgroup, and
a bare `docker run` is what the house rules forbid.

Ponytail: this runs `dot` once per seed rather than reading
`target/gv-dot-det-a/graphviz-dot.jsonl`, which already holds every printed coordinate. The
re-run costs ~65 s over 1000 seeds and buys a probe that needs nothing but the fixture file,
which is the point of a probe: it has to be runnable from a clean checkout in one command,
and a file some earlier arm happened to leave behind is not that.
"""

import json
import os
import sys
import tempfile
from collections import namedtuple

sys.dont_write_bytecode = True

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from gv_plain import POINTS_PER_INCH, graphviz_version, run_engine, write_dot

# The rank grid: `DEFAULT_NODEHEIGHT` (0.5 inch) + `DEFAULT_RANKSEP` (0.5 inch), in points.
# Every fixture node has the default height, so the step is exact rather than approximate.
RANK_STEP = (0.5 + 0.5) * POINTS_PER_INCH

# One probe row: everything one `-Tplain` run produced for one seed. A namedtuple rather
# than a bare tuple because the row grew two columns and `probe_record` already returned
# seven values positionally; a ninth positional unpack misreads silently, while `row.xs`
# names the column it meant.
Row = namedtuple("Row", "seed count source target ranks order xs ys worst")

USAGE = (
    "usage: oracle-dot-probe.py <fixtures-dir> [--fixtures=dot.jsonl] --digest=PATH\n"
    "       oracle-dot-probe.py <fixtures-dir> --table=N [--start=N]\n"
    "  --digest=PATH  write the per-seed rank, order and printed coordinate rows to PATH\n"
    "  --table=N      print the rank table for the first N seeds, and nothing else\n"
    "  --start=N      the -Gstart value (default 1); measured inert for dot\n"
)


def printed_nodes(text, count):
    """`{node id: (x string, y string)}`, refusing a node count that is not the fixture's.

    The strings are `-Tplain`'s own and they stay strings to the digest line, because the
    comparison downstream is byte for byte; the rank and the order are derived from them by
    [`point_in_points`] below.

    `gv_plain.parse_plain` reads this and the graph line as well; the probe wants the
    refusal and the coordinates, and nothing in it needs the bounding box.
    """
    nodes = {}
    for line in text.splitlines():
        parts = line.split()
        if parts and parts[0] == "node" and len(parts) >= 4:
            nodes[parts[1]] = (parts[2], parts[3])
    if len(nodes) != count:
        sys.exit(f"plain output has {len(nodes)} nodes, expected {count}")
    return nodes


def point_in_points(node):
    """`(x, y)` in points, parsed from one node's two printed inch strings."""
    return [float(node[0]) * POINTS_PER_INCH, float(node[1]) * POINTS_PER_INCH]


def printed_columns(nodes, count):
    """`(xs, ys)`: the printed inch strings of every node, in node-index order.

    Indexed by the id (`n<k>` -> index `k`) and not by the plain format's line order, exactly
    as the rank column already is: a node the engine dropped or renamed is a `KeyError` here
    rather than a silently short row, and the count check in [`printed_nodes`] is what refuses
    a drawing with a node too many.
    """
    return (
        [nodes[f"n{i}"][0] for i in range(count)],
        [nodes[f"n{i}"][1] for i in range(count)],
    )


def ranks_and_order(nodes, count):
    """`(ranks, order, worst grid distance)` for one printed drawing.

    `nodes` is what [`printed_nodes`] returned, so every coordinate is parsed here out of the
    oracle's own inch strings. The three answers are the rank of every node by index, the
    within-rank order as one concatenated list (rank 0 first), and how far the worst printed
    y sits from the 72-point grid — the measurement that says the rank column is a rank and
    not a rounded guess at one.
    """
    at = [point_in_points(nodes[f"n{i}"]) for i in range(count)]
    top = max(coord[1] for coord in at)
    raw = [(top - coord[1]) / RANK_STEP for coord in at]
    worst = max(abs(value - round(value)) for value in raw)
    ranks = [round(value) for value in raw]
    order = []
    for rank in range(max(ranks) + 1):
        here = [i for i in range(count) if ranks[i] == rank]
        # x ascending, then name, so the sort is total (D5) even where two x tie.
        order.extend(sorted(here, key=lambda i: (at[i][0], f"n{i}")))
    return ranks, order, worst


def probe_record(record, tmp, start):
    """One seed: write the DOT graph, run `dot`, read the ranks, order and coordinates back."""
    path = os.path.join(tmp, f"g{record['seed']}.dot")
    write_dot(path, record["n"], record["source"], record["target"])
    nodes = printed_nodes(run_engine("dot", path, start), record["n"])
    ranks, order, worst = ranks_and_order(nodes, record["n"])
    xs, ys = printed_columns(nodes, record["n"])
    return Row(
        record["seed"], record["n"], record["source"], record["target"],
        ranks, order, xs, ys, worst,
    )


def digest_line(row):
    """One digest row: `seed n`, the edges, the ranks, the order, then the two coordinates.

    Four trailing groups of `row.count` tokens, the last two of them the oracle's own printed
    inch strings with nothing done to them.
    """
    pairs = " ".join(f"{s},{t}" for s, t in zip(row.source, row.target))
    return " ".join(
        [str(row.seed), str(row.count), pairs]
        + [str(r) for r in row.ranks]
        + [str(o) for o in row.order]
        + list(row.xs)
        + list(row.ys)
    )


def print_table(rows):
    """The rank table `dot/rank_tests.rs` pins, one line per seed."""
    print("seed n ranks")
    for row in rows:
        print(f"{row.seed} {row.count} " + " ".join(str(r) for r in row.ranks))


def parse_options(argv):
    """The flags, in the `--flag=value` shape the other harness arms use."""
    options = {"fixtures": "dot.jsonl", "digest": None, "table": None, "start": 1}
    for arg in argv:
        name, _, value = arg.partition("=")
        if name == "--fixtures":
            options["fixtures"] = value
        elif name == "--digest":
            options["digest"] = value
        elif name == "--table":
            options["table"] = int(value)
        elif name == "--start":
            options["start"] = int(value)
        else:
            sys.exit(USAGE)
    return options


def main(argv):
    """Probe every fixture, then write the digest or print the rank table.

    `argv` is the whole command line, `argv[0]` included, so the fixture directory is
    `argv[1]` and the flags are everything after it — the shape `oracle-graphviz.py` uses.
    """
    if len(argv) < 2 or argv[1].startswith("-"):
        sys.exit(USAGE)
    fixtures_dir = argv[1]
    options = parse_options(argv[2:])
    graphviz_version()
    rows = []
    worst = 0.0
    path = os.path.join(fixtures_dir, options["fixtures"])
    with open(path) as handle:
        records = [json.loads(line) for line in handle if line.strip()]
    with tempfile.TemporaryDirectory() as tmp:
        for record in records:
            row = probe_record(record, tmp, options["start"])
            rows.append(row)
            worst = max(worst, row.worst)
            if options["table"] is not None and len(rows) >= options["table"]:
                break
    if options["table"] is not None:
        print_table(rows)
        return
    if options["digest"] is None:
        sys.exit(USAGE)
    with open(options["digest"], "w") as out:
        out.write("# seed n t,h ... ranks order xs ys -- harness/oracle-dot-probe.py\n")
        for row in rows:
            out.write(digest_line(row) + "\n")
    print(f"{len(rows)} seeds -> {options['digest']}")
    print(f"largest distance from the rank grid: {worst:.4f} of a step")


if __name__ == "__main__":
    main(sys.argv)