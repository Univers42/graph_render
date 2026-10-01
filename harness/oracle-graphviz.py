#!/usr/bin/env python3
"""Graphviz layout oracle: run a Graphviz engine over the fixtures and record node
positions in points, keyed by node id, with the graph bounding box.

Run in the ge-graphviz-oracle image:

  graph-cli emit-spectral-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
      python3 harness/oracle-graphviz.py target/spectral-fixtures twopi target/gv-twopi

For each fixture record, writes a DOT graph (undirected, nodes n0..n{n-1}), runs
`<engine> -Tplain -Gstart=1`, and records the node positions in points (the plain
format reports inches; 1 inch = 72 points) plus the graph bounding box.

Determinism is proven by running this twice over the same fixtures and `cmp`-ing the
outputs; twopi, osage and circo are all byte-identical across runs. A full 1000-seed sweep
is not affordable for circo (n=501 alone takes ~53s, so the sweep is ~1.5h), so the
determinism check runs over a strided 20-seed subset spanning n=2..552.

Ponytail: the plain format's node order is the DOT declaration order, which is dense
n0..n{n-1}, so the mapping back to the fixture's source/target columns is trivial —
but the engine may drop isolated nodes or merge duplicates, so the harness asserts the
node count matches and refuses otherwise. `START_SEED` is passed as `-Gstart` because
the job asked for a fixed seed where the engine takes one, and it is measured to be
INERT for twopi, osage and circo (patchwork: docs/measurements/p13-gv1-patchwork.md): the same fixture hashes identically with start=1, 7, 99
and with no `-Gstart` at all. Each is deterministic unconditionally, so this harness
proves determinism, not seed stability. The one measured sensitivity: the output is
byte-stable to the last digit, and a 1e-6-point perturbation of one node coordinate
changes byte 95, so a `cmp` here is not vacuous.

Four flags, all optional, so the three-argument call above is unchanged. `--start=N` sets
the `-Gstart` value (default 1), which exists because seed sensitivity has to be *measured*
rather than asserted; `--fixtures=NAME` names the fixture file in the fixtures directory
(`spectral.jsonl` when unset); `--differential` compares our coordinates with the engine's,
reading `<engine>.jsonl` and `<engine>-manifest.json` and writing `<engine>-result.json`
for `graph-cli oracle-graphviz --engine <engine>` to read; `--shards N --shard I` /
`--merge` shard that sweep.

**Sharding.** `circo` costs ~40 s on a 440-node fixture and ~0.5 s on a 40-node one, so
1000 seeds serially is hours. `--shards N --shard I` runs every Nth fixture and writes
`<engine>-result-shard<I>.json` into the output directory; `--merge` folds the shards into
the one `<engine>-result.json` the Rust check reads, and refuses a merge whose case count
does not add up to the manifest's seed count. The partition is `index % N == I` over the
fixture file's own order, so it does not depend on any timing, and the fold is `max` over
the worsts — an order-free reduction. An engine with neither flag is run exactly as it is
on develop: one shard, and the same file names.

**Three child modules**, each holding one kind of thing, so no arm of this file is the
place the metric lives: `gv_plain.py` (write one DOT graph, run one engine, read
`-Tplain`), `gv_closed.py` (the analytically determined cases and the one uniform rescale
both arms go through) and `gv_frames.py` (the closed cases whose answers are already in the
frame `-Tplain` prints, which is `osage` alone — `circo`'s are in the layout's own frame, so
`gv_closed.rendered` applies the half-node offset `-Tplain` translates by). The split is why
this file has room for the sharding above and stays under the house limit.
"""

import hashlib
import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# `harness/` is inside `FINGERPRINTED` (`crates/graph-cli/src/fingerprint.rs:21`), and importing
# a module by name makes CPython write `harness/__pycache__/*.pyc` — a transient file inside a
# fingerprinted tree, which moves the fingerprint for as long as it exists. That would make `emit`
# (fingerprint without the `.pyc`) and `oracle-graphviz` (fingerprint with it) disagree on a clean
# checkout, and the check would refuse a run that was in fact the right one. The flag is set before
# the child modules are imported, so the bytecode is never written; `oracle-twopi.py` sets it the
# same way around the one path import it makes.
sys.dont_write_bytecode = True

from gv_closed import CLOSED, answer_of, closed_case, gap
from gv_frames import FRAMED_CLOSED, framed_cases
from gv_plain import (
    START_SEED,
    edges_of,
    engine_points,
    parse_plain,
    read_json,
    read_lines,
    run_engine,
    write_dot,
    write_lines,
)

USAGE = (
    "usage: oracle-graphviz.py <fixtures-dir> <engine> <out-dir> [--start=N] "
    "[--fixtures=NAME] [--differential] [--shards N --shard I | --merge]"
)


class Options:
    """The flags, parsed: `start`, `fixtures`, `differential`, `merge`, `shards`, `shard`."""

    def __init__(self, argv):
        self.start, self.fixtures = START_SEED, "spectral.jsonl"
        self.differential = self.merge = False
        self.shards, self.shard = 1, 0
        positional = []
        index = 0
        while index < len(argv):
            arg = argv[index]
            index += 1
            if arg == "--differential":
                self.differential = True
            elif arg == "--merge":
                self.merge = True
            elif arg in ("--shards", "--shard"):
                value = int(argv[index])
                index += 1
                if arg == "--shards":
                    self.shards = value
                else:
                    self.shard = value
            elif arg.startswith("--start="):
                self.start = int(arg.split("=", 1)[1])
            elif arg.startswith("--fixtures="):
                self.fixtures = arg.split("=", 1)[1]
            elif arg.startswith("--"):
                sys.exit(f"unknown flag {arg}\n{USAGE}")
            else:
                positional.append(arg)
        if len(positional) != 3:
            sys.exit(USAGE)
        self.fixtures_dir, self.engine, self.out_dir = positional
        if self.shards < 1 or not 0 <= self.shard < self.shards:
            sys.exit(f"--shard {self.shard} is outside --shards {self.shards}")


def ours_of(record, engine):
    """The fixture's own coordinates, as points in dense-index order."""
    column = record[engine]
    return [(column["x"][i], column["y"][i]) for i in range(record["n"])]


def closed_cases(engine, tmp, start):
    """Every closed case one engine is graded on, or none.

    Two renderings, and the reason is the frame each engine's answers are written in:
    `osage`'s port keeps Graphviz's own translation, so its answers are printed as they
    stand; `circo`'s are in the layout's own frame, so `gv_closed.rendered` applies the
    half-node `-Tplain` translates by. An engine in neither table gets an empty mapping,
    and `graph-cli oracle-graphviz` then says nothing about byte agreement for it.
    """
    if engine in CLOSED:
        return {
            name: closed_case(engine, tmp, name, edges, answer_of(shape), start)
            for name, (edges, shape) in CLOSED[engine].items()
        }
    if engine in FRAMED_CLOSED:
        return framed_cases(engine, tmp, FRAMED_CLOSED[engine], START_SEED)
    return {}


def main():
    argv = sys.argv[1:]
    if not argv or argv[0] in ("-h", "--help"):
        sys.exit(USAGE)
    options = Options(argv)
    if options.merge:
        return merge_main(options.fixtures_dir, options.engine, options.out_dir, options.shards)
    if options.differential:
        return differential_main(options)
    return record_main(options)


def record_main(options):
    """The plain positional call: record where the engine put every node."""
    engine = options.engine
    jsonl_path = os.path.join(options.fixtures_dir, options.fixtures)
    os.makedirs(options.out_dir, exist_ok=True)
    out_path = os.path.join(options.out_dir, f"graphviz-{engine}.jsonl")
    manifest_path = os.path.join(options.out_dir, f"graphviz-{engine}-manifest.json")

    count = 0
    with tempfile.TemporaryDirectory() as tmp:
        with open(out_path, "w") as out:
            for line in open(jsonl_path):
                rec = json.loads(line)
                n = rec["n"]
                dot_path = os.path.join(tmp, f"g{rec['seed']}.dot")
                write_dot(dot_path, n, rec["source"], rec["target"])
                bbox, nodes = parse_plain(run_engine(engine, dot_path, options.start), n)
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


def differential_main(options):
    """`--differential`: compare the native arm in `<engine>.jsonl` with Graphviz's own.

    One shard writes `<engine>-result-shard<I>.json` into `out_dir` plus the points it
    measured; with a single shard it writes `<engine>-result.json` into `fixtures_dir`,
    which is what `graph-cli oracle-graphviz` reads. [`merge_main`] is the fold.
    """
    engine, shards, shard = options.engine, options.shards, options.shard
    os.makedirs(options.out_dir, exist_ok=True)
    stem = f"{engine}.jsonl"
    manifest = read_json(os.path.join(options.fixtures_dir, f"{engine}-manifest.json"))
    fixtures = read_lines(os.path.join(options.fixtures_dir, stem))
    mine = [r for at, r in enumerate(fixtures) if at % shards == shard]
    worst, theirs = 0.0, []
    with tempfile.TemporaryDirectory() as tmp:
        for record in mine:
            seed, count = record["seed"], record["n"]
            points = engine_points(
                engine, tmp, f"g{seed}", count, edges_of(record), options.start
            )
            worst = max(worst, gap(ours_of(record, engine), points))
            theirs.append({"seed": seed, "n": count, "points": points})
        # The closed cases cost one small graph each and are the same in every shard, so
        # only shard 0 pays for them and the others record no verdict.
        closed = closed_cases(engine, tmp, options.start) if not shard else {}
    suffix = "" if shards == 1 else f"-shard{shard}"
    write_lines(os.path.join(options.out_dir, f"graphviz-{engine}{suffix}.jsonl"), theirs)
    exact = all(row["exact"] for row in closed.values()) if closed else None
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": manifest["sha256"][stem],
        "oracle": f"Graphviz 16.1.0 {engine} -Tplain -Gstart={options.start}",
        "layouts": {engine: {"cases": len(theirs), "worst": worst}},
        "closed": closed,
    }
    if exact is not None:
        result["closed_exact"] = exact
    where, name = (options.out_dir, f"{engine}-result{suffix}.json") if shards > 1 else (
        options.fixtures_dir,
        f"{engine}-result.json",
    )
    with open(os.path.join(where, name), "w") as out:
        json.dump(result, out, indent=1)
    print(f"{engine} shard {shard}/{shards}: {len(theirs)} seeds, worst {worst:.3e} points")
    return 0 if exact is not False else 1


def merge_main(fixtures_dir, engine, out_dir, shards):
    """`--merge`: fold the per-shard results into the one `<engine>-result.json`.

    The fold is `max` over the worsts and `sum` over the cases, both order-free. It
    **refuses** a sweep whose shards do not add up to the manifest's seed count, and a shard
    whose fixtures or tree are not the ones on disk: a merge that quietly dropped a shard
    would report a smaller sweep than it ran.
    """
    parts = [
        read_json(os.path.join(out_dir, f"{engine}-result-shard{at}.json"))
        for at in range(shards)
    ]
    manifest = read_json(os.path.join(fixtures_dir, f"{engine}-manifest.json"))
    check_shards_agree(parts, manifest, engine)
    cases = sum(part["layouts"][engine]["cases"] for part in parts)
    if cases != manifest["seeds"]:
        sys.exit(f"{cases} cases over {shards} shards, want {manifest['seeds']} seeds")
    worst = max(part["layouts"][engine]["worst"] for part in parts)
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": manifest["sha256"][f"{engine}.jsonl"],
        "oracle": parts[0]["oracle"],
        "layouts": {engine: {"cases": cases, "worst": worst}},
        "closed": parts[0]["closed"],
        "closed_exact": parts[0].get("closed_exact"),
        "shards": shards,
    }
    with open(os.path.join(fixtures_dir, f"{engine}-result.json"), "w") as out:
        json.dump(result, out, indent=1)
    print(
        f"{engine}: {cases} seeds over {shards} shards, worst {worst:.3e} points; "
        f"closed {len(result['closed'])} exact: {result['closed_exact']}"
    )
    return 0 if result["closed_exact"] is not False else 1


def check_shards_agree(parts, manifest, engine):
    """Every shard must have run the same tree over the same fixtures, or the max of their
    worsts is a max over different questions."""
    for at, part in enumerate(parts):
        if part["fingerprint"] != manifest["fingerprint"]:
            sys.exit(f"shard {at} ran against another tree: re-emit and re-run")
        if part["sha256"] != manifest["sha256"][f"{engine}.jsonl"]:
            sys.exit(f"shard {at} ran against other fixtures than these")


if __name__ == "__main__":
    sys.exit(main() or 0)