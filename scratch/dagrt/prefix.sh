#!/bin/bash
# The dag arm's negative control, and the pre-fix crossing baseline, in one temporary
# swap: HEAD's sugiyama sources in place, both measurements run, then the fixed sources
# restored and byte-compared against the copies taken first. Never a git state change:
# `git show HEAD:<path>` only reads.
set -u
cd /mnt/storage/bench/graph_render-wt/fix-dag-roundtrip || exit 1
LOG=scratch/dagrt/prefix.log
SAVE=scratch/dagrt/fixsrc
FILES="crates/graph-core/src/layout/sugiyama/acyclic.rs
crates/graph-core/src/layout/sugiyama/acyclic/tests.rs
crates/graph-core/src/layout/sugiyama/layering.rs
crates/graph-core/src/layout/sugiyama/tests.rs"

mkdir -p "$SAVE"
for f in $FILES; do mkdir -p "$SAVE/$(dirname "$f")"; cp "$f" "$SAVE/$f"; done

restore() {
  for f in $FILES; do cp "$SAVE/$f" "$f"; done
}
trap restore EXIT

for f in $FILES; do git show "7730dad:$f" > "$f"; done
{
  echo "### pre-fix sources in place; the unit tests that are GREEN now:"
  scripts/orch/gr cargo test -q -p graph-core --lib layout::sugiyama 2>&1 | tail -12
  echo "### pre-fix crossing dump, our own side of the dagre differential"
  scripts/orch/gr cargo test -q -p graph-core --lib layout::sugiyama::measurement -- --ignored dump_crossing_measurements 2>&1 | tail -4
  cp target/dag-crossings.json scratch/dagrt/dag-crossings.prefix.json
  scripts/orch/node-slim.sh node scratch/dagrt/crossings.mjs
  echo "### pre-fix roundtrip --seeds 100: the row's own control"
  scripts/orch/gr cargo run -q --release -p graph-cli -- roundtrip --seeds 100
  echo "PREFIX_ROUNDTRIP_EXIT=$?"
} 2>&1 | tee "$LOG"

restore
trap - EXIT
echo "### restore check (empty diff = the fixed sources are back)"
git diff --stat
for f in $FILES; do cmp -s "$f" "$SAVE/$f" || echo "MISMATCH $f"; done