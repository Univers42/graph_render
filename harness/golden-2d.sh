#!/usr/bin/env bash
# The 2D golden sweep: every registered layout, seeds 0..7, the snapshot's binary face,
# sha256 per (layout, seed). Run inside the ge-rust image:
#
#   scripts/orch/gr bash /w/harness/golden-2d.sh target/golden-now.txt
#
# Its purpose is the one claim p12-t4b's done-when rests on: **adding the 3D arms moved no
# 2D byte.** The diff against `harness/golden-2d-2d-before.txt` — the pin taken before the
# first edit — is that claim, and it is a pin rather than a `target/` file on purpose: a
# pin is versioned, so the comparison is re-runnable on a fresh clone instead of depending
# on an artifact a previous run happened to leave behind. Same pattern as
# `crates/graph-cli/tests/fixtures/fa2-nx-reference.jsonl`.
#
# Re-pin only together with a decision about what moved: a diff here means a 2D layout's
# bytes changed, and the reason belongs in `docs/measurements/p12-t4b.md` before the pin is
# rewritten, not after.
#
# `graph-cli snapshot` is the same entry the hash gate hashes through, so this measures the
# bytes a consumer reads rather than an internal representation of them.
set -euo pipefail

BIN=${GM_BIN:-/w/target/release/graph-cli}
OUT=${1:?usage: golden-2d.sh <output-file>}

# The 28 ids that existed before the 3D arms, in registry order. A 3D id's snapshot is a
# different shape (a z column, and a 0.4 label), so including them here would compare two
# different things.
#
# **These are the registry's exact ids, and that matters more than it looks.** An earlier
# draft of this sweep listed `layout.tidy_tree`, `layout.treemap`, `layout.sugiyama`,
# `layout.osage` and `layout.graphviz.*` — none of which exist — and swallowed the CLI's
# error, so those lines hashed *empty stdout* to `e3b0c44…`, the same constant on every
# seed. The sweep still reported 224 lines and still diffed clean, and proved nothing about
# those five. Hence the line-count check below **and** the per-line check that the digest is
# not the empty-input hash.
IDS="layout.grid layout.tree.tidy layout.treemap.squarified layout.circular.radial \
layout.packing.circle layout.spectral layout.mds.pivot layout.force.barnes_hut \
layout.forceatlas2 layout.dag.sugiyama layout.random layout.circular.ring \
layout.spiral layout.bipartite layout.force.yifan_hu layout.force.fruchterman_reingold \
layout.force.kamada_kawai layout.force.graphopt layout.force.davidson_harel \
layout.force.lgl layout.force.drl layout.twopi layout.packing.osage \
layout.force.spring layout.circular.hierarchy layout.circular.circo \
layout.treemap.patchwork layout.force.neato"

# sha256 of the empty string: what a line whose layout failed to run looks like.
EMPTY_SHA=e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855

: > "$OUT"
for layout in $IDS; do
  for seed in 0 1 2 3 4 5 6 7; do
    # `set -o pipefail` is on, so a CLI failure fails the pipeline rather than piping an
    # empty stream into sha256sum — but the digest is checked as well, because a layout
    # that legitimately emits no bytes would otherwise slip through as a real pin.
    digest=$("$BIN" snapshot --seed "$seed" --layout "$layout" --out-bin - \
      | sha256sum | cut -d' ' -f1)
    if [ "$digest" = "$EMPTY_SHA" ]; then
      echo "golden-2d: $layout seed $seed hashed empty output (id wrong, or run failed)" >&2
      exit 2
    fi
    printf '%s %s %s\n' "$layout" "$seed" "$digest" >> "$OUT"
  done
done

lines=$(wc -l < "$OUT")
if [ "$lines" -ne 224 ]; then
  echo "golden-2d: swept $lines lines, expected 224 (28 layouts x 8 seeds)" >&2
  exit 2
fi
echo "golden-2d: $lines lines, none empty"