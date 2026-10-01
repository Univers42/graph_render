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
with no `-Gstart` at all. Both engines are deterministic unconditionally, so this
harness proves determinism, not seed stability. The one measured sensitivity: the
output is byte-stable to the last digit, and a 1e-6-point perturbation of one node
coordinate changes byte 95, so a `cmp` here is not vacuous.
"""
import hashlib
import json
import os
import subprocess
import sys
import tempfile

POINTS_PER_INCH = 72.0
START_SEED = 1


def write_dot(path, n, source, target):
    lines = ["graph g {"]
    for i in range(n):
        lines.append(f"  n{i};")
    for s, t in zip(source, target):
        lines.append(f"  n{s} -- n{t};")
    lines.append("}")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


def run_engine(engine, dot_path):
    cmd = [engine, "-Tplain", f"-Gstart={START_SEED}", dot_path]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
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


def compare(argv):
    """The differential arm: our own coordinates against the engine's, per seed.

    Additive. This is the third mode of this file, after the two-argument run below and
    the `--start` override, and it exists because the *engine* plumbing above and the
    *comparison* are different jobs with different shapes: the first produces a raw record
    of what the engine said, and this one grades it. Keeping them apart is what lets the
    determinism `cmp` in the ADR keep running over a file this mode also writes.

        oracle-graphviz.py --compare <engine> <fixtures-dir> <out-dir>

    where `<fixtures-dir>` holds the `<engine>.jsonl` and `<engine>-manifest.json` that
    `graph-cli emit-graphviz-fixtures --engine <engine>` wrote, and this writes
    `<engine>-result.json` beside them for `graph-cli oracle-graphviz --engine <engine>`
    to judge. The engine's own answers go to `<out-dir>/graphviz-<engine>.jsonl`, the file
    the determinism check `cmp`s.

    The metric is the largest absolute node-coordinate difference **in points**, after both
    arms are rescaled onto one bounding box: Graphviz's own node-centre box is the target and
    both arms are mapped onto it with one uniform scale taken from the larger axis. Rescaling
    removes the two degrees of freedom that say nothing about the layout — Graphviz translates
    its drawing so the bounding box's lower-left is the origin, and an iterative engine's
    absolute scale is whatever its initial placement happened to be — and removes nothing
    else. One uniform scale rather than one per axis, so an aspect-ratio error arrives as a
    larger gap instead of hiding inside a per-axis fit, and so a degenerate axis (a 2-node
    graph is a straight line, one span zero) does not divide by zero.

    For an *iterative* engine this gap is a real algorithmic difference and not a rounding
    artefact, which is why `docs/measurements/p13-gv2-neato.md` is five orders of magnitude
    larger than the closed-form arms' and why the ceiling is measured rather than guessed.
    """
    if len(argv) != 3:
        sys.exit("usage: oracle-graphviz.py --compare <engine> <fixtures-dir> <out-dir>")
    engine, fixtures_dir, out_dir = argv
    manifest = read_json(os.path.join(fixtures_dir, f"{engine}-manifest.json"))
    fixtures = read_lines(os.path.join(fixtures_dir, f"{engine}.jsonl"))
    os.makedirs(out_dir, exist_ok=True)
    worst, theirs = sweep(engine, out_dir, fixtures)
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": manifest["sha256"][f"{engine}.jsonl"],
        "oracle": f"Graphviz 16.1.0 {engine} -Tplain -Gstart={START_SEED}",
        "layouts": {engine: {"cases": len(theirs), "worst": worst}},
    }
    with open(os.path.join(fixtures_dir, f"{engine}-result.json"), "w") as out:
        json.dump(result, out, indent=1)
    print(f"{engine}: {len(theirs)} seeds, worst {worst:.3e} points")
    return 0


def sweep(engine, out_dir, fixtures):
    """Every fixture through the engine: the worst gap in points, and the raw output.

    The raw output goes to `<out-dir>/graphviz-<engine>.jsonl` in the same record shape the
    two-argument mode writes, so the ADR's determinism check — run this twice, `cmp` the file
    — stays available for this mode too, and a reviewer can read what the engine actually
    said per seed rather than only the summary.
    """
    worst = 0.0
    theirs = []
    with tempfile.TemporaryDirectory() as tmp:
        for record in fixtures:
            points = engine_points(engine, tmp, f"g{record['seed']}", record["n"], record)
            ours = ours_of(record, engine)
            worst = max(worst, gap(ours, points))
            theirs.append(
                {
                    "seed": record["seed"],
                    "engine": engine,
                    "n": record["n"],
                    "points": points,
                }
            )
    write_lines(os.path.join(out_dir, f"graphviz-{engine}.jsonl"), theirs)
    return worst, theirs


def engine_points(engine, tmp, name, count, record):
    """`<engine> -Tplain -Gstart=1` over one fixture's DOT graph, in dense-index order."""
    dot = os.path.join(tmp, f"{name}.dot")
    write_dot(dot, count, record["source"], record["target"])
    _, nodes = parse_plain(run_engine(engine, dot), count)
    return [tuple(nodes[f"n{i}"]) for i in range(count)]


def ours_of(record, engine):
    """The fixture's own coordinates for `engine`, as points in dense-index order.

    The column is keyed by the engine's own name, so a differential holding two engines
    reads two columns out of one file and neither can pick up the other's.
    """
    column = record[engine]
    return [(column["x"][i], column["y"][i]) for i in range(record["n"])]


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


def read_json(path):
    with open(path) as handle:
        return json.load(handle)


def read_lines(path):
    with open(path) as handle:
        return [json.loads(line) for line in handle]


def write_lines(path, rows):
    with open(path, "w") as handle:
        for row in rows:
            handle.write(json.dumps(row) + "\n")


def main():
    if len(sys.argv) == 5 and sys.argv[1] == "--compare":
        return compare(sys.argv[2:])
    if len(sys.argv) != 4:
        sys.exit(
            "usage: oracle-graphviz.py <fixtures-dir> <engine> <out-dir>\n"
            "       oracle-graphviz.py --compare <engine> <fixtures-dir> <out-dir>"
        )
    fixtures_dir, engine, out_dir = sys.argv[1], sys.argv[2], sys.argv[3]
    jsonl_path = os.path.join(fixtures_dir, "spectral.jsonl")
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
                plain = run_engine(engine, dot_path)
                bbox, nodes = parse_plain(plain, n)
                record = {
                    "seed": rec["seed"],
                    "engine": engine,
                    "bbox": {"width": bbox[0], "height": bbox[1]},
                    "nodes": nodes,
                }
                out.write(json.dumps(record) + "\n")
                count += 1

    digest = hashlib.sha256(open(out_path, "rb").read()).hexdigest()
    manifest = {
        "engine": engine,
        "seeds": count,
        "sha256": {f"graphviz-{engine}.jsonl": digest},
        "graphviz": "16.1.0",
    }
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=1)
    print(f"{engine}: {count} seeds -> {out_path}")


if __name__ == "__main__":
    sys.exit(main() or 0)
