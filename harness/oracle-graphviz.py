#!/usr/bin/env python3
"""Graphviz layout oracle: run a Graphviz engine over the fixtures and record node
positions in points, keyed by node id, with the graph bounding box.

Run in the ge-graphviz-oracle image:

  graph-cli emit-spectral-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
      python3 harness/oracle-graphviz.py target/spectral-fixtures twopi target/gv-twopi
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
      python3 harness/oracle-graphviz.py target/spectral-fixtures circo target/gv-circo

For each fixture record, writes a DOT graph (undirected, nodes n0..n{n-1}), runs
`<engine> -Tplain -Gstart=1`, and records the node positions in points (the plain
format reports inches; 1 inch = 72 points) plus the graph bounding box.

**Two arms.** Without a flag this is the record arm above. `--differential` is the
two-arm comparison against our own native layout, over the fixtures
`graph-cli emit-graphviz-fixtures --engine <engine>` writes: the metric is the largest
absolute node-coordinate difference in points after both arms are rescaled onto the same
bounding box, and the analytically determined small cases are compared byte for byte at
the plain format's own printed resolution. Both live in `gv_closed.py` (the cases and the
rescale) and `gv_plain.py` (the engine and the parsers); this file is the driver.

**Sharding.** `circo` costs ~40 s on a 440-node fixture and ~0.5 s on a 40-node one, so
1000 seeds serially is hours. `--shards N --shard I` runs every Nth fixture and writes
`<engine>-result-shard<I>.json` into the output directory; `--merge` folds the shards into
the one `<engine>-result.json` the Rust check reads, and refuses a merge whose case count
does not add up to the manifest's seed count. The partition is `index % N == I` over the
fixture file's own order, so it does not depend on any timing, and the fold is `max` over
the worsts — an order-free reduction.

Determinism is proven by running the record arm twice over the same fixtures and `cmp`-ing
the outputs; both twopi and circo are byte-identical across runs. A full 1000-seed sweep is
not affordable for the determinism check either (the same ~40 s per large fixture), so it
runs over a strided 20-seed subset spanning n=2..552.

Ponytail: `START_SEED` is passed as `-Gstart` because the job asked for a fixed seed where
the engine takes one, and it is measured to be INERT for twopi and circo: the same fixture
hashes identically with start=1, 7, 99 and with no `-Gstart` at all. Both engines are
deterministic unconditionally, so this harness proves determinism, not seed stability. The
one measured sensitivity: the output is byte-stable to the last digit, and a 1e-6-point
perturbation of one node coordinate changes byte 95, so a `cmp` here is not vacuous.
"""

import hashlib
import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from gv_closed import CLOSED, answer_of, closed_case, gap
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


def main():
    argv = sys.argv[1:]
    differential = "--differential" in argv
    argv = [arg for arg in argv if arg != "--differential"]
    merge = "--merge" in argv
    argv = [arg for arg in argv if arg != "--merge"]
    shards, shard = shard_flags(argv)
    if len(argv) != 3:
        sys.exit(
            "usage: oracle-graphviz.py <fixtures-dir> <engine> <out-dir> [--differential]"
            " [--shards N --shard I | --merge]"
        )
    fixtures_dir, engine, out_dir = argv
    if merge:
        return merge_main(fixtures_dir, engine, out_dir, shards)
    if differential:
        return differential_main(fixtures_dir, engine, out_dir, shard, shards)
    return record_main(fixtures_dir, engine, out_dir)


def shard_flags(argv):
    """`(--shards, --shard)` out of `argv`, which it consumes in place: `N` containers and
    this one's index, defaulting to one container and the only shard.

    A shard outside `[0, N)` is a refusal rather than a clamp: a clamped or empty shard
    would merge into a sweep that looks smaller than it is, which is the one way a sharded
    differential could flatter itself.
    """
    shards, shard = 1, 0
    for flag, index in (("--shards", 0), ("--shard", 1)):
        while flag in argv:
            at = argv.index(flag)
            value = int(argv[at + 1])
            if shards < 1:
                sys.exit("--shards must be at least 1")
            if index == 0:
                shards = value
            else:
                shard = value
            del argv[at : at + 2]
    if not 0 <= shard < shards:
        sys.exit(f"--shard {shard} is outside --shards {shards}")
    return shards, shard


def record_main(fixtures_dir, engine, out_dir):
    """The record arm: Graphviz's own answer per fixture, and the manifest that pins it."""
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
    return 0


def differential_main(fixtures_dir, engine, out_dir, shard, shards):
    """`--differential`: compare the native arm in `<engine>.jsonl` with Graphviz's own.

    One shard writes `<engine>-result-shard<I>.json` into `out_dir` plus the points it
    measured; with a single shard it writes `<engine>-result.json` into `fixtures_dir`,
    which is what `graph-cli oracle-graphviz` reads. [`merge_main`] is the fold.
    """
    os.makedirs(out_dir, exist_ok=True)
    stem = f"{engine}.jsonl"
    manifest = read_json(os.path.join(fixtures_dir, f"{engine}-manifest.json"))
    fixtures = read_lines(os.path.join(fixtures_dir, stem))
    mine = [r for at, r in enumerate(fixtures) if at % shards == shard]
    cases = CLOSED.get(engine, {})
    worst, theirs = 0.0, []
    with tempfile.TemporaryDirectory() as tmp:
        for record in mine:
            seed, count = record["seed"], record["n"]
            points = engine_points(engine, tmp, f"g{seed}", count, edges_of(record))
            column = record[engine]
            ours = [(column["x"][i], column["y"][i]) for i in range(count)]
            worst = max(worst, gap(ours, points))
            theirs.append({"seed": seed, "n": count, "points": points})
        # The closed cases cost one small graph each and are the same in every shard, so
        # only shard 0 pays for them and the others record `None` for the verdict.
        closed = {} if shard else {
            name: closed_case(engine, tmp, name, edges, answer_of(shape))
            for name, (edges, shape) in cases.items()
        }
    suffix = "" if shards == 1 else f"-shard{shard}"
    write_lines(os.path.join(out_dir, f"graphviz-{engine}{suffix}.jsonl"), theirs)
    exact = all(row["exact"] for row in closed.values()) if closed else None
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": manifest["sha256"][stem],
        "oracle": f"Graphviz 16.1.0 {engine} -Tplain -Gstart={START_SEED}",
        "layouts": {engine: {"cases": len(theirs), "worst": worst}},
        "closed": closed,
        "closed_exact": exact,
    }
    where, name = (out_dir, f"{engine}-result{suffix}.json") if shards > 1 else (
        fixtures_dir,
        f"{engine}-result.json",
    )
    with open(os.path.join(where, name), "w") as out:
        json.dump(result, out, indent=1)
    print(f"{engine} shard {shard}/{shards}: {len(theirs)} seeds, worst {worst:.3e} points")
    return 0


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
        "closed_exact": parts[0]["closed_exact"],
        "shards": shards,
    }
    with open(os.path.join(fixtures_dir, f"{engine}-result.json"), "w") as out:
        json.dump(result, out, indent=1)
    print(
        f"{engine}: {cases} seeds over {shards} shards, worst {worst:.3e} points; "
        f"closed {len(result['closed'])} exact: {result['closed_exact']}"
    )
    return 0 if result["closed_exact"] else 1


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