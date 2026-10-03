#!/usr/bin/env bash
# memprofile.sh [n...] — the motor's heap under valgrind, the evidence behind
# docs/decisions/memory-guard.md, written under target/memprofile/:
#   massif-<run>.out, .txt   the heap over time and the allocation tree at its peak (ms_print)
#   dhat-<run>.json, .log    every allocation site; "At t-gmax" is the peak, "At t-end" the bytes
#                            still live when the process exits (a leak, or a static)
#   memcheck-<run>.log       leak check at the smallest n: definitely and indirectly lost
#   summary.md               one row per run: peak heap, bytes per node, bytes live at exit
# A run is `graph-cli tick --n <n> --ticks 2 --warm 1` (the scale model, its index and a live
# force session) and, at the smallest n, `tick --grow` (a session carried onto a bigger topology).
# Defaults: n = 10000 100000 1000000. Exit 0 = every tool ran; non-zero = the first that failed.
#
# Caveat: the scale model is one generator (crates/graph-cli/src/bench/scale.rs); a graph much
# denser than it, or a layout other than the force engines, has another bytes-per-node figure.
# Massif samples snapshots, so its peak can miss a short spike between two; dhat's t-gmax is exact.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
ns=("$@")
(($#)) || ns=(10000 100000 1000000)
out=target/memprofile
bin=target/profile/build/release/graph-cli
GR_IMAGE=ge-profile GR_MEM=${GR_MEM:-12g} "$here/gr" -e CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
  bash -euo pipefail -c "
    cargo build -q --release -p graph-cli --target-dir target/profile/build
    mkdir -p $out
    one() { # <run> <n> <tick args...>
      local run=\$1 n=\$2; shift 2
      valgrind --tool=massif --massif-out-file=$out/massif-\$run.out $bin tick \"\$@\" >/dev/null 2>$out/massif-\$run.log
      ms_print $out/massif-\$run.out > $out/massif-\$run.txt
      valgrind --tool=dhat --dhat-out-file=$out/dhat-\$run.json $bin tick \"\$@\" >/dev/null 2>$out/dhat-\$run.log
      local peak gmax tend
      peak=\$(grep -oE 'mem_heap_B=[0-9]+' $out/massif-\$run.out | cut -d= -f2 | sort -n | tail -1)
      gmax=\$(grep -oE 'At t-gmax: +[0-9,]+' $out/dhat-\$run.log | grep -oE '[0-9,]+\$' | tr -d ,)
      tend=\$(grep -oE 'At t-end: +[0-9,]+' $out/dhat-\$run.log | grep -oE '[0-9,]+\$' | tr -d ,)
      echo \"| \$run | \$n | \$peak | \$gmax | \$((gmax / n)) | \$tend |\" >> $out/summary.md
    }
    printf '%s\n' '| run | n | massif peak heap B | dhat t-gmax B | B per node | dhat t-end B |' '|---|---|---|---|---|---|' > $out/summary.md
    for n in ${ns[*]}; do one tick-\$n \$n --n \$n --ticks 2 --warm 1; done
    small=${ns[0]}
    one grow-\$small \$small --n \$small --grow \$((small / 10))
    valgrind --leak-check=full --show-leak-kinds=definite,indirect --errors-for-leak-kinds=definite,indirect \
      --error-exitcode=3 $bin tick --n \$small --ticks 2 --warm 1 >/dev/null 2>$out/memcheck-tick-\$small.log
    chmod -R a+rX $out
  "
cat "$out/summary.md"
grep -E 'definitely lost|indirectly lost|ERROR SUMMARY' "$out"/memcheck-*.log
