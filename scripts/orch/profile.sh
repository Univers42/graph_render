#!/usr/bin/env bash
# profile.sh [n] [ticks] [warm] — profile `graph-cli tick` (crates/graph-cli/src/bench/tick.rs) in the
# ge-profile image (docker/profile.Dockerfile), writing everything under target/profile/n<n>/:
#   tick.md          the tick table, native speed (wall time per tick)
#   hyperfine.json   five whole runs, wall clock, for run-to-run spread
#   callgrind.*      instructions and simulated L1/LL misses per function, inside one engine tick
#   dhat.*           heap blocks and bytes over the whole run
# Defaults: n=100000, ticks=3, warm=2. Exit 0 = every tool ran; non-zero = the first that failed.
#
# The binary is release with line tables only (CARGO_PROFILE_RELEASE_DEBUG), in its own target dir so
# the workspace's release artifacts are not rebuilt; Cargo.toml is untouched, so no gate evidence is
# voided.
#
# Callgrind with --cache-sim stands in for cachegrind, which has no --toggle-collect window.
#
# Caveat: callgrind runs the program 50-100x slower on one simulated core, so a 1M run
# takes tens of minutes; its counts are exact but its cache model is valgrind's (LL = the host's
# last-level size, no prefetcher), so a miss ratio is a comparison between builds, not a prediction of
# hardware. The toggle window drops everything outside the engine tick, the model build and the index
# included.
# Wall-clock rows are inflated by other host load: the tick table prints the load average.
set -euo pipefail
n=${1:-100000} ticks=${2:-3} warm=${3:-2}
here=$(dirname "$(readlink -f "$0")")
out=target/profile/n$n
bin=target/profile/build/release/graph-cli
args="tick --n $n --ticks $ticks --warm $warm"
# `tick` steps through `ForceSession::step_with`, which is generic and inlined into graph-cli, so
# the window is each engine's tick: a window on `ForceSession>::step` collected 0 instructions.
window="'--toggle-collect=*barnes_hut::sim::Sim>::tick*' '--toggle-collect=*particle_mesh::tick::<*'"
GR_IMAGE=ge-profile GR_MEM=${GR_MEM:-12g} "$here/gr" -e CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
  bash -euo pipefail -c "
    cargo build -q --release -p graph-cli --target-dir target/profile/build
    mkdir -p $out
    $bin $args | tee $out/tick.md
    hyperfine -N --runs 5 --export-json $out/hyperfine.json '$bin $args'
    valgrind --tool=callgrind --cache-sim=yes $window --callgrind-out-file=$out/callgrind.out \
      $bin $args 2> $out/callgrind.log
    callgrind_annotate --auto=no --inclusive=yes $out/callgrind.out > $out/callgrind.txt
    valgrind --tool=dhat --dhat-out-file=$out/dhat.json $bin $args 2> $out/dhat.log
    chmod -R a+rX $out
  "
echo "profile: $out"
