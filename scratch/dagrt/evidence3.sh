#!/bin/bash
# After the pre-fix control swap: the post-fix seed-66 snapshot against the pre-fix one
# (which edges moved, and that nothing else did), then the workspace test floor.
set -u
cd /mnt/storage/bench/graph_render-wt/fix-dag-roundtrip || exit 1
mv scratch/dagrt/s66.json scratch/dagrt/s66.prefix.json
scripts/orch/gr cargo run -q --release -p graph-cli -- snapshot --seed 66 --nodes 68 \
  --layout dag.sugiyama --out-json scratch/dagrt/s66.fixed.json
echo "SNAPSHOT_EXIT=$?"
scripts/orch/node-slim.sh node scratch/dagrt/s66-diff.mjs
echo "DIFF_EXIT=$?"
echo "### workspace floor"
scripts/orch/gr cargo test --workspace --no-fail-fast 2>&1 | grep -E "^(test result|error|failures:|---- )" | sort | uniq -c
echo "WORKSPACE_TESTS_DONE"