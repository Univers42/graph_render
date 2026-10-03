#!/bin/bash
# The evidence run for docs/measurements/fix-dag-roundtrip.md, in the order the report cites it.
cd /mnt/storage/bench/graph_render-wt/fix-dag-roundtrip || exit 1
run() { echo "### $*"; "$@"; echo "EXIT=$?"; }

run scripts/orch/gr cargo test --workspace --no-fail-fast
run scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown

# The dagre differential's input: our own crossing counts, whole.
run scripts/orch/gr cargo test -q -p graph-core --lib layout::sugiyama::measurement -- --ignored dump_crossing_measurements
scripts/orch/node-slim.sh node scratch/dagrt/crossings.mjs

run scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8
echo "NEGCTL_DEGREE_EXIT=$?"
run scripts/scigraphs-conformance.sh