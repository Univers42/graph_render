"""The Graphviz side of the oracle: write one DOT graph, run one engine, read `-Tplain`.

The pieces every arm of `oracle-graphviz.py` needs and nothing else — the engine's
invocation, the plain format's two parsers, and the JSONL/JSON files they read and write.
It is its own module because the driver's three arms (the record arm, the sweep, the
closed cases) all reach for exactly this and none of them for each other.

`parse_plain` returns the parsed floats; `printed_nodes` returns the **strings** `-Tplain`
printed. Both are needed and they are not interchangeable: a closed case is compared byte
for byte, so re-printing a parsed value would grade our arithmetic against the oracle's
rounding instead of against its own text.

Ponytail: the plain format's node order is the DOT declaration order, which is dense
`n0..n{n-1}`, so the mapping back to the fixture's source/target columns is trivial — but the
engine may drop isolated nodes or merge duplicates, so `parse_plain` asserts the node count
matches and refuses otherwise.
"""

import json
import os
import subprocess
import sys

POINTS_PER_INCH = 72.0

# The seed handed to the engine as `-Gstart`, overridable so a determinism sweep can compare
# one run against another at a different seed without a second copy of the harness:
# `GM_ORACLE_START=7 docker run -e GM_ORACLE_START=7 ... oracle-graphviz.py ... circo ...`.
# The default is the seed every recorded run used, so nothing else moves.
START_SEED = int(os.environ.get("GM_ORACLE_START", "1"))


def write_dot(path, n, source, target):
    """One undirected DOT graph, nodes declared dense so the plain order is the index order."""
    lines = ["graph g {"]
    for i in range(n):
        lines.append(f"  n{i};")
    for s, t in zip(source, target):
        lines.append(f"  n{s} -- n{t};")
    lines.append("}")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


def run_engine(engine, dot_path):
    """`<engine> -Tplain -Gstart=<seed> <dot>`, or the engine's own complaint and no answer."""
    cmd = [engine, "-Tplain", f"-Gstart={START_SEED}", dot_path]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.exit(f"{engine} failed on {dot_path}: {proc.stderr}")
    return proc.stdout


def parse_plain(text, n):
    """`(bbox, nodes)` from a plain drawing: the graph line's size in points and every
    node's centre in points, keyed by node id. Refuses a node count that is not the
    fixture's, which is the one way the engine could silently answer about another graph."""
    bbox = None
    nodes = {}
    for line in text.splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "graph" and len(parts) >= 4:
            bbox = (float(parts[2]) * POINTS_PER_INCH, float(parts[3]) * POINTS_PER_INCH)
        elif parts[0] == "node" and len(parts) >= 4:
            nodes[parts[1]] = [
                float(parts[2]) * POINTS_PER_INCH,
                float(parts[3]) * POINTS_PER_INCH,
            ]
    if bbox is None:
        sys.exit("plain output has no graph line")
    if len(nodes) != n:
        sys.exit(f"plain output has {len(nodes)} nodes, expected {n}")
    return bbox, nodes


def engine_points(engine, tmp, name, count, edges):
    """The engine's own node coordinates over one DOT graph, as dense-indexed points."""
    dot = os.path.join(tmp, f"{name}.dot")
    write_dot(dot, count, [a for a, _ in edges], [b for _, b in edges])
    _, nodes = parse_plain(run_engine(engine, dot), count)
    return [tuple(nodes[f"n{i}"]) for i in range(count)]


def printed_nodes(engine, tmp, name, count, edges):
    """The engine's coordinates as the two strings `-Tplain` printed for each.

    The text, not the parsed float: the comparison is byte for byte, and re-printing a parsed
    value would grade our `f64` against the oracle's rounding instead of its arithmetic.
    """
    dot = os.path.join(tmp, f"{name}.dot")
    write_dot(dot, count, [a for a, _ in edges], [b for _, b in edges])
    rows = {}
    for line in run_engine(engine, dot).splitlines():
        parts = line.split()
        if len(parts) >= 4 and parts[0] == "node":
            rows[parts[1]] = (parts[2], parts[3])
    return [rows[f"n{i}"] for i in range(count)]


def edges_of(record):
    """The fixture's dense edge endpoints, in its own column order."""
    return list(zip(record["source"], record["target"]))


def read_json(path):
    with open(path) as handle:
        return json.load(handle)


def read_lines(path):
    with open(path) as handle:
        return [json.loads(line) for line in handle]


def write_lines(path, rows):
    with open(path, "w") as out:
        for row in rows:
            out.write(json.dumps(row) + "\n")