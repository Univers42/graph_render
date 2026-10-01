#!/usr/bin/env python3
"""Graphviz layout oracle: run a Graphviz engine over the spectral fixtures and record
node positions in points, keyed by node id, with the graph bounding box.

Run in the ge-graphviz-oracle image:

  graph-cli emit-spectral-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
      python3 harness/oracle-graphviz.py target/spectral-fixtures twopi target/gv-twopi
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
      python3 harness/oracle-graphviz.py target/spectral-fixtures circo target/gv-circo

For each fixture record, writes a DOT graph (undirected, nodes n0..n{n-1}), runs
`<engine> -Tplain -Gstart=1`, and records the node positions in points (the plain
format reports inches; 1 inch = 72 points) plus the graph bounding box.

Determinism is proven by running this twice over the same fixtures and `cmp`-ing the
outputs; both twopi and circo are byte-identical across runs. A full 1000-seed sweep
is not affordable for circo (n=501 alone takes ~53s, so the sweep is ~1.5h), so the
determinism check runs over a strided 20-seed subset spanning n=2..552.

Ponytail: the plain format's node order is the DOT declaration order, which is dense
n0..n{n-1}, so the mapping back to the fixture's source/target columns is trivial —
but the engine may drop isolated nodes or merge duplicates, so the harness asserts the
node count matches and refuses otherwise. `START_SEED` is passed as `-Gstart` because
the job asked for a fixed seed where the engine takes one, and it is measured to be
INERT for twopi and circo: the same fixture hashes identically with start=1, 7, 99 and
with no `-Gstart` at all, and the same three-way measurement for patchwork is recorded
in docs/measurements/p13-gv1-patchwork.md. Both engines are deterministic
unconditionally, so this harness proves determinism, not seed stability. Set
`GM_GV_START` in the environment to measure that per engine; the value used is recorded
in the manifest's `start` field. The one measured sensitivity for twopi and circo: the
output is byte-stable to the last digit, and a 1e-6-point perturbation of one node
coordinate changes byte 95, so a `cmp` here is not vacuous.

Two flags were added later, both optional, so the three-argument call above is unchanged:
`--start=N` sets the `-Gstart` value (default 1), which exists because seed sensitivity has
to be *measured* rather than asserted; `--fixtures=NAME` names the fixture file in the
fixtures directory (`spectral.jsonl` when unset); `--differential` compares our coordinates
with the engine's, reading `<engine>.jsonl` and `<engine>-manifest.json` and writing
`<engine>-result.json` for `graph-cli oracle-graphviz --engine <engine>` to read. The
metric, the rescale and the closed cases are **imported from `harness/oracle-twopi.py`**
rather than copied: a second copy of the metric would be a second definition of the number
the ceiling is measured against.
"""
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile

POINTS_PER_INCH = 72.0
# The seed the job pins. `GM_GV_START` overrides it so an engine's seed sensitivity can be
# measured without editing this file (p13-gv1-patchwork, step 2); unset means the pinned 1.
START_SEED = int(os.environ.get("GM_GV_START", "1"))

USAGE = "usage: oracle-graphviz.py <fixtures-dir> <engine> <out-dir> [--start=N] [--fixtures=NAME] [--differential]"


def write_dot(path, n, source, target):
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
ENGINE_BENIGN_STDERR = {"sfdp": ("Error: remove_overlap: Graphviz not built with triangulation library",)}


def run_engine(engine, dot_path, start=START_SEED):
    cmd = [engine, "-Tplain", f"-Gstart={start}", dot_path]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        benign = ENGINE_BENIGN_STDERR.get(engine, ())
        noise = [ln for ln in proc.stderr.splitlines() if ln.strip() not in benign]
        if noise or not benign:
            sys.exit(f"{engine} failed on {dot_path}: {proc.stderr}")
    return proc.stdout


def parse_plain(text, n):
    bbox = None
    nodes = {}
    for line in text.splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "graph" and len(parts) >= 4:
            bbox = (float(parts[2]) * POINTS_PER_INCH, float(parts[3]) * POINTS_PER_INCH)
        elif parts[0] == "node" and len(parts) >= 4:
            name = parts[1]
            x = float(parts[2]) * POINTS_PER_INCH
            y = float(parts[3]) * POINTS_PER_INCH
            nodes[name] = [x, y]
    if bbox is None:
        sys.exit("plain output has no graph line")
    if len(nodes) != n:
        sys.exit(f"plain output has {len(nodes)} nodes, expected {n}")
    return bbox, nodes


def load_peer(here):
    """`harness/oracle-twopi.py`, by path: its filename is not importable by name.

    The metric (`gap`), the rescale it rests on and the closed cases are shared, not
    copied: a second copy of the metric would be a second definition of the number the
    ceiling is measured against. `sys.dont_write_bytecode` keeps the fingerprinted
    `harness/` tree free of the `__pycache__` a path import would otherwise write.
    """
    path = os.path.join(here, "oracle-twopi.py")
    previous = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec = importlib.util.spec_from_file_location("oracle_twopi", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


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

CLOSED = {"osage": OSAGE_CLOSED}

# The `sfdp` closed answers, in points, **in the frame `-Tplain` prints**.
#
# Exactly one case, and it is closed only because there is nothing left to be random about: a
# graph with a single node has one position, and the reference lays it at the centre of the
# default 0.75 x 0.5 inch node box. Measured identical at `-Gstart` 1, 7 and 99.
#
# The other five cases of `harness/oracle-twopi.py`'s `CLOSED_CASES` are **deliberately absent**
# here. This engine is seed-sensitive — measured, not assumed: the two-node, 3-path, 4-cycle,
# 5-star and 6-branch graphs each print three *different* answers at `-Gstart` 1, 7 and 99,
# because the seeded random start is the layout's only source of symmetry breaking. So there is
# no closed answer for them to be compared against, and `docs/measurements/p13-gv2-sfdp.md`
# records the measured gaps instead. Listing a "closed" answer here that the oracle itself
# contradicts would turn a failing row green for the wrong reason.
SFDP_CLOSED = {
    "one-node": [(27.0, 18.0)],
}

CLOSED["sfdp"] = SFDP_CLOSED


def engine_points(tmp, engine, name, count, edges, start):
    """`<engine> -Tplain` over one DOT graph, as dense-indexed points."""
    dot = os.path.join(tmp, f"{name}.dot")
    write_dot(dot, count, [a for a, _ in edges], [b for _, b in edges])
    _, nodes = parse_plain(run_engine(engine, dot, start), count)
    return [tuple(nodes[f"n{i}"]) for i in range(count)]


def ours_of(record, engine):
    """The fixture's own coordinates, as points in dense-index order."""
    column = record[engine]
    return [(column["x"][i], column["y"][i]) for i in range(record["n"])]


def printed_nodes(tmp, engine, name, count, edges, start):
    """The two strings `-Tplain` printed for each node, in dense order.

    The text, not a re-printed parsed float: the closed cases are compared byte for byte,
    and re-printing would grade our arithmetic against the oracle's rounding.
    """
    dot = os.path.join(tmp, f"{name}.dot")
    write_dot(dot, count, [a for a, _ in edges], [b for _, b in edges])
    rows = {}
    for line in run_engine(engine, dot, start).splitlines():
        parts = line.split()
        if len(parts) >= 4 and parts[0] == "node":
            rows[parts[1]] = (parts[2], parts[3])
    return [rows[f"n{i}"] for i in range(count)]


def closed_case(peer, tmp, engine, name, start):
    """One closed case: the engine's own printed node lines against the closed answer."""
    edges = peer.CLOSED_CASES[name]
    count = 1 + max((max(edge) for edge in edges), default=0)
    got = printed_nodes(tmp, engine, f"closed-{name}", count, edges, start)
    digits = peer.DIGITS
    want = " ".join(
        f"{x / POINTS_PER_INCH:.{digits}g} {y / POINTS_PER_INCH:.{digits}g}"
        for x, y in CLOSED[engine][name]
    )
    text = " ".join(f"{x} {y}" for x, y in got)
    return {"nodes": count, "exact": want == text, "want": want, "got": text}


def sweep(fixtures, engine, start, peer):
    """Every fixture through the engine: the worst gap in points, and the raw output.

    The raw output is what the ADR's determinism evidence is a `cmp` over, and what a
    reviewer reads to see what the oracle actually said per seed.
    """
    worst, theirs = 0.0, []
    with tempfile.TemporaryDirectory() as tmp:
        for record in fixtures:
            edges = list(zip(record["source"], record["target"]))
            points = engine_points(tmp, engine, f"g{record['seed']}", record["n"], edges, start)
            worst = max(worst, peer.gap(ours_of(record, engine), points))
            theirs.append({"seed": record["seed"], "n": record["n"], "points": points})
        closed = (
            {
                name: closed_case(peer, tmp, engine, name, start)
                for name in CLOSED[engine]
            }
            if engine in CLOSED
            else {}
        )
    return worst, theirs, closed


def differential(fixtures_dir, engine, out_dir, start, peer):
    """Compare our coordinates with the engine's and write the result the CLI reads.

    The metric is the peer's, so both Graphviz differentials are measured the same way: the
    largest absolute node-coordinate difference in points, after both arms are rescaled
    onto one bounding box. A closed case that disagrees is a failure, not a note, and so is
    this command's exit code — the closed cases carry the exactness a tolerance rounds off.
    """
    manifest = peer.read(os.path.join(fixtures_dir, f"{engine}-manifest.json"))
    fixtures = peer.read_lines(os.path.join(fixtures_dir, f"{engine}.jsonl"))
    worst, theirs, closed = sweep(fixtures, engine, start, peer)
    os.makedirs(out_dir, exist_ok=True)
    peer.write_lines(os.path.join(out_dir, f"graphviz-{engine}.jsonl"), theirs)
    exact = all(row["exact"] for row in closed.values()) if closed else None
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": manifest["sha256"][f"{engine}.jsonl"],
        "oracle": f"Graphviz 16.1.0 {engine} -Tplain -Gstart={start}",
        "layouts": {engine: {"cases": len(theirs), "worst": worst}},
        "closed": closed,
    }
    if exact is not None:
        result["closed_exact"] = exact
    with open(os.path.join(fixtures_dir, f"{engine}-result.json"), "w") as out:
        json.dump(result, out, indent=1)
    print(f"{engine}: {len(theirs)} seeds, worst {worst:.3e} points; closed {len(closed)} exact: {exact}")
    return 0 if exact is not False else 1


def main():
    argv = sys.argv[1:]
    if not argv or argv[0] in ("-h", "--help"):
        sys.exit(USAGE)
    start, compare, fixtures, positional = START_SEED, False, "spectral.jsonl", []
    for arg in argv:
        if arg == "--differential":
            compare = True
        elif arg.startswith("--start="):
            start = int(arg.split("=", 1)[1])
        elif arg.startswith("--fixtures="):
            fixtures = arg.split("=", 1)[1]
        else:
            positional.append(arg)
    if len(positional) != 3:
        sys.exit(USAGE)
    fixtures_dir, engine, out_dir = positional
    if compare:
        peer = load_peer(os.path.dirname(os.path.abspath(__file__)))
        return differential(fixtures_dir, engine, out_dir, start, peer)
    return record(fixtures_dir, engine, out_dir, start, fixtures)


def record(fixtures_dir, engine, out_dir, start, fixtures):
    """The plain positional call: record where the engine put every node."""
    jsonl_path = os.path.join(fixtures_dir, fixtures)
    os.makedirs(out_dir, exist_ok=True)
    out_path = os.path.join(out_dir, f"graphviz-{engine}.jsonl")
    manifest_path = os.path.join(out_dir, f"graphviz-{engine}-manifest.json")

    count = 0
    with tempfile.TemporaryDirectory() as tmp:
        with open(out_path, "w") as out:
            for line in open(jsonl_path):
                rec = json.loads(line)
                n = rec["n"]
                dot_path = os.path.join(tmp, f"g{rec['seed']}.dot")
                write_dot(dot_path, n, rec["source"], rec["target"])
                plain = run_engine(engine, dot_path, start)
                bbox, nodes = parse_plain(plain, n)
                row = {
                    "seed": rec["seed"],
                    "engine": engine,
                    "bbox": {"width": bbox[0], "height": bbox[1]},
                    "nodes": nodes,
                }
                out.write(json.dumps(row) + "\n")
                count += 1

    digest = hashlib.sha256(open(out_path, "rb").read()).hexdigest()
    manifest = {
        "engine": engine,
        "start": START_SEED,
        "seeds": count,
        "sha256": {f"graphviz-{engine}.jsonl": digest},
        "graphviz": "16.1.0",
    }
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=1)
    print(f"{engine}: {count} seeds -> {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
