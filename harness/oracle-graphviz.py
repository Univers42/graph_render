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

**Five child modules**, each holding one kind of thing, so no arm of this file is the
place the metric lives: `gv_plain.py` (write one DOT graph, run one engine, read
`-Tplain`), `gv_closed.py` (the analytically determined cases and the one uniform rescale
both arms go through), `gv_frames.py` (the closed cases whose answers are already in the
frame `-Tplain` prints, which is `osage`, `fdp`, `sfdp` and `dot` — `circo`'s are in the
layout's own frame, so `gv_closed.rendered` applies the half-node offset `-Tplain`
translates by),
`gv_sized.py` (the size-pinned DOT writer, for the one engine that sizes nodes from labels)
and `gv_arms.py` (the three entry points below: record, differential, merge). The split is
why this file is a driver over the flags and the metric and stays under the house limit.
"""

import os
import sys

# `harness/` is inside `FINGERPRINTED` (`crates/graph-cli/src/fingerprint.rs:21`), and importing
# a module by name makes CPython write `harness/__pycache__/*.pyc` — a transient file inside a
# fingerprinted tree, which moves the fingerprint for as long as it exists. That would make `emit`
# (fingerprint without the `.pyc`) and `oracle-graphviz` (fingerprint with it) disagree on a clean
# checkout, and the check would refuse a run that was in fact the right one. The flag is set before
# the child modules are imported, so the bytecode is never written; `oracle-twopi.py` sets it the
# same way around the one path import it makes.
sys.dont_write_bytecode = True

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from gv_arms import differential_main, merge_main, record_main
from gv_closed import CLOSED, answer_of, closed_case
from gv_frames import FRAMED_CLOSED, framed_cases
from gv_plain import START_SEED, dot_path, edges_of, engine_points, graph_of
from gv_sized import sized_points

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


def engine_arms(engine, tmp, record, start):
    """Where one engine's node coordinates come from: the pinned DOT or the bare one.

    The fixture decides, by whether it carries a `box` column, so an engine gains or loses the
    pinning by changing what the emit writes rather than by a name checked here.
    """
    if "box" in record:
        return sized_points(engine, tmp, f"g{record['seed']}", record, start)
    return engine_points(
        engine,
        dot_path(tmp, f"g{record['seed']}"),
        graph_of(record["n"], edges_of(record)),
        start,
    )


def closed_cases(engine, tmp, start):
    """Every closed case one engine is graded on, and the peer names it compared nothing for.

    Two renderings, and the reason is the frame each engine's answers are written in:
    `osage`'s port keeps Graphviz's own translation, so its answers are printed as they
    stand; `circo`'s are in the layout's own frame, so `gv_closed.rendered` applies the
    half-node `-Tplain` translates by. An engine in neither table gets an empty mapping,
    and `graph-cli oracle-graphviz` then says nothing about byte agreement for it.

    `start` reaches `framed_cases`: it used to be replaced by `START_SEED` at the call site,
    so `--start=7` drew the framed cases at seed 1 while the `oracle` string recorded 7. For
    `fdp` the seed is load-bearing (`docs/measurements/p13-gv2-fdp.md:364`, the
    `fdp-oracle-start-effective` row), so the byte-agreement gate was seed-invariant while
    claiming not to be.
    """
    if engine in CLOSED:
        return {
            name: closed_case(engine, tmp, (name, edges, answer_of(shape)), start)
            for name, (edges, shape) in CLOSED[engine].items()
        }, []
    if engine in FRAMED_CLOSED:
        return framed_cases(engine, tmp, FRAMED_CLOSED[engine], start)
    return {}, []


def main():
    argv = sys.argv[1:]
    if not argv or argv[0] in ("-h", "--help"):
        sys.exit(USAGE)
    options = Options(argv)
    if options.merge:
        return merge_main(options.fixtures_dir, options.engine, options.out_dir, options.shards)
    if options.differential:
        return differential_main(options, ours_of, engine_arms, closed_cases)
    return record_main(options, ours_of)


if __name__ == "__main__":
    sys.exit(main() or 0)