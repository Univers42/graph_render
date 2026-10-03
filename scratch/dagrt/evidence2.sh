#!/bin/bash
# The rest of the evidence run, full output this time (the first pass was piped through
# `tail -120` and lost these lines).
cd /mnt/storage/bench/graph_render-wt/fix-dag-roundtrip || exit 1
run() { echo "### $*"; "$@"; echo "EXIT=$?"; }

run scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown
run scripts/orch/gr cargo test -q -p graph-core --lib layout::sugiyama::measurement -- --ignored dump_crossing_measurements
echo "### crossings vs docs/measurements/phase05-crossings.md"
scripts/orch/node-slim.sh node scratch/dagrt/crossings.mjs
echo "EXIT=$?"
echo "### parallel-arcs crossings (not in the doc table)"
scripts/orch/node-slim.sh node -e '
const g = JSON.parse(require("fs").readFileSync("target/dag-crossings.json","utf8"));
const p = g.find((x) => x.name === "parallel-arcs");
console.log("parallel-arcs ours", p.our_crossings, "nodes", p.nodes.length, "edges", p.edges.length);'
run scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
echo "### roundtrip unit-level dag arm on the whole seeded sweep"
run scripts/orch/gr cargo test -q -p graph-core --lib layout::sugiyama