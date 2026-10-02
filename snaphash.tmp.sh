#!/bin/sh
# usage: snaphash.sh <binary>  -> one line per (layout, seed, nodes): sha256
for l in layout.mds.pivot layout.spectral; do
  for s in 0 1 2 3 5 8 13 21 34 55 89 144 233 377 599; do
    printf '%s %s default ' $l $s; $1 snapshot --seed $s --layout $l --out-bin - 2>/dev/null | sha256sum | cut -c1-16
  done
  for s in 1 7; do for n in 3000 20000; do
    printf '%s %s %s ' $l $s $n; $1 snapshot --seed $s --nodes $n --layout $l --out-bin - 2>/dev/null | sha256sum | cut -c1-16
  done; done
done
