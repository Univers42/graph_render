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


def main():
    if len(sys.argv) != 4:
        sys.exit("usage: oracle-graphviz.py <fixtures-dir> <engine> <out-dir>")
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
    main()
