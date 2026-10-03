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
import re
import subprocess
import sys
from collections import namedtuple

POINTS_PER_INCH = 72.0

# The Graphviz these recorded runs were measured against: every pinned closed answer in
# `gv_closed.py` and `gv_frames.py` is what this version printed, and
# `docs/measurements/p13-gv1-*.md` and `p13-gv2-*.md` attribute their byte-stability claims
# to it. A literal in the result file asserted it without asking the binary, so an image
# with any other Graphviz still claimed 16.1.0. `graphviz_version()` asks `dot` instead.
PINNED_GRAPHVIZ = "16.1.0"

# One engine's wall clock, in seconds. `circo` is documented at ~40 s on a 440-node fixture
# (`oracle-graphviz.py:39`) and a 1000-seed sweep is sharded around exactly that.
#
# Ponytail: a flat ceiling for every engine and every size, so it will cut a slow engine on
# a big fixture short rather than hang, and it will never notice a small fixture that
# should have taken milliseconds. It bounds the harness, not the measurement: no wall clock
# reaches any output (D3), and a `TimeoutExpired` is a refusal, not a zero.
ENGINE_TIMEOUT = 900.0

# One graph to draw: the node count and the two endpoint columns, split once so
# `engine_points` and `printed_nodes` take four parameters rather than six, and so neither
# re-derives the other's idea of the edge list.
Graph = namedtuple("Graph", "count source target")


def graph_of(count, edges):
    """A `Graph` from a node count and an edge list, in creation order."""
    return Graph(count, [a for a, _ in edges], [b for _, b in edges])


def dot_path(tmp, name):
    """Where one case's DOT graph is written."""
    return os.path.join(tmp, f"{name}.dot")

# The seed handed to the engine as `-Gstart` when no `--start=N` says otherwise. Two ways to
# move it, both read here so no arm has to know about the other: the `--start=N` flag, and the
# `GM_GV_START` environment variable for a run that cannot pass a flag
# (`docker run -e GM_GV_START=7 ...`). The default is the seed every recorded run used, so
# nothing else moves. The seed is inert for twopi, osage and circo, and it is not for neato,
# which seeds a `drand48` initial placement from it and so draws every fixture differently at
# 1, 7 and 99: a neato differential is comparable only at the start value its `oracle` string
# records (`docs/measurements/p13-gv2-neato.md`).
START_SEED = int(os.environ.get("GM_GV_START", "1"))


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


# Per-engine allowance for a *build* notice this image cannot avoid, measured not guessed.
#
# `sfdp` calls `remove_overlap` unconditionally (`lib/sfdpgen/spring_electrical.c:1181`) and in an
# image built without the triangulation library that function is an empty stub which prints one
# line and returns (`lib/neatogen/overlap.c:588-610`). The notice sets Graphviz's error flag, so
# the process exits 1 while stdout already holds the complete, finished `-Tplain` drawing. The
# coordinates are therefore sfdp's own: overlap removal changed nothing, because it ran no code.
# Measured: `-Goverlap` false/true/scale/prism/vor all produce byte-identical stdout and the same
# exit 1, so no flag value can suppress it — the notice is removed here, not worked around.
# Every other engine keeps the strict rule: a non-zero exit is a failure.
ENGINE_BENIGN_STDERR = {
    "sfdp": ("Error: remove_overlap: Graphviz not built with triangulation library",)
}


def graphviz_version():
    """The Graphviz version `dot -V` reports, refusing one that is not `PINNED_GRAPHVIZ`.

    `dot -V` prints on stderr (`dot - graphviz version 16.1.0 (20260904.0139)`), so both
    streams are read. Refusing is the point: the pinned closed answers in `gv_closed.py`
    and `gv_frames.py` are what one version printed, and a comparison against another
    version's drawing is not the comparison those answers state.
    """
    proc = subprocess.run(["dot", "-V"], capture_output=True, text=True)
    found = re.search(r"version (\S+)", proc.stderr + proc.stdout)
    if found is None:
        sys.exit(f"dot -V reported no version: {proc.stderr.strip()}")
    if found.group(1) != PINNED_GRAPHVIZ:
        sys.exit(f"Graphviz {found.group(1)}, want {PINNED_GRAPHVIZ}: re-pin or re-measure")
    return found.group(1)


def run_engine(engine, dot_path_, start=None):
    """`<engine> -Tplain -Gstart=<seed> <dot>`, or the engine's own complaint and no answer.

    `start` defaults to [`START_SEED`] but is resolved **inside** the body, not bound at `def`
    time: a `def`-time default freezes the value the module held when it was imported, so a
    later rebind of `START_SEED` never reached here. `GM_GV_START` is read at import and
    `oracle-graphviz.py` passes `--start` explicitly, so this is the third path to the seed
    and all three now read one place.
    """
    if start is None:
        start = START_SEED
    cmd = [engine, "-Tplain", f"-Gstart={start}", dot_path_]
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=ENGINE_TIMEOUT)
    except subprocess.TimeoutExpired:
        sys.exit(f"{engine} ran longer than {ENGINE_TIMEOUT:.0f}s on {dot_path_}")
    if proc.returncode != 0:
        benign = ENGINE_BENIGN_STDERR.get(engine, ())
        lines = [ln.strip() for ln in proc.stderr.splitlines() if ln.strip()]
        # Every line on stderr must be the notice this engine is *known* to print, and
        # there must be one: an engine that exited non-zero having said nothing is a
        # failure, not a drawing. `sfdp` alone has an entry, so for every other engine
        # `benign` is empty and this refuses on the non-zero exit alone.
        if not benign or not lines or any(ln not in benign for ln in lines):
            sys.exit(f"{engine} failed on {dot_path_}: {proc.stderr}")
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


def engine_points(engine, tmp, name, count, edges, start=None):
    """The engine's own node coordinates over one DOT graph, as dense-indexed points.

    Six positional parameters, over the house's four, and left so on purpose:
    `harness/scigraphs-conformance/sc_graphviz.py:59` calls this signature positionally and
    that file is owned by another job, so re-aritying it here would break the conformance gate.
    `framed_case` and `gv_closed.closed_case`, the two the review named for their arity, are
    both down to four. What m63 also named about *these* — the `start=START_SEED` default,
    bound once at `def` time so a later rebind never arrived — is fixed: `start` defaults to
    `None` and `run_engine` resolves it inside its body.
    """
    dot = dot_path(tmp, name)
    write_dot(dot, count, [a for a, _ in edges], [b for _, b in edges])
    _, nodes = parse_plain(run_engine(engine, dot, start), count)
    return [tuple(nodes[f"n{i}"]) for i in range(count)]


def printed_nodes(engine, dot, graph, start=None):
    """The engine's coordinates as the two strings `-Tplain` printed for each.

    The text, not the parsed float: the comparison is byte for byte, and re-printing a parsed
    value would grade our `f64` against the oracle's rounding instead of its arithmetic.

    Both of `parse_plain`'s checks are restated here rather than inherited: this reader keeps
    the strings, so it cannot go through `parse_plain`, and without them a *successful* engine
    that printed an empty or truncated drawing surfaced as `KeyError: 'n0'` instead of the
    diagnostic — a stack trace where the answer was "the engine drew nothing".
    """
    write_dot(dot, graph.count, graph.source, graph.target)
    text = run_engine(engine, dot, start)
    rows, drew_graph = {}, False
    for line in text.splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "graph":
            drew_graph = True
        elif parts[0] == "node" and len(parts) >= 4:
            rows[parts[1]] = (parts[2], parts[3])
    if not drew_graph:
        sys.exit(f"{engine} printed no graph line for {dot}")
    if len(rows) != graph.count:
        sys.exit(f"{engine} printed {len(rows)} nodes for {dot}, expected {graph.count}")
    return [rows[f"n{i}"] for i in range(graph.count)]


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