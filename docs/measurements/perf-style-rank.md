# Perf style-rank — a reveal step at 1M skips `styleFrom`'s work (355 → 0.001 ms), a weights change ranks in O(n) (353 → 22 ms)

Measured 2026-10-03 in worktree `perf-style-rank`, branch tip `a858d803` plus the uncommitted
change, Node `v22.23.3` (`GM_NODE_IMAGE`, `node:22.23.3-slim`), 1 000 000 nodes. The host has 20
cores and was **loaded** throughout: the 1-minute load average stood between 17.6 and 24.3 on
every round (other jobs' containers), so the spread below is mostly the host's, not the code's.

## Reproduction

```sh
# the after arm: this tree
scripts/orch/node-slim.sh node --experimental-strip-types packages/graph-render/tests/bench-style.ts
# the before arm: a858d803's packages/graph-render under target/bench-base, with the same bench
# file but for the rank import (`rankOf` from style.ts in place of `rankByWeight`)
scripts/orch/node-slim.sh node --experimental-strip-types target/bench-base/packages/graph-render/tests/bench-style.ts
```

The bench (`packages/graph-render/tests/bench-style.ts`) builds seeded weights in 0..1
(`mulberry32(20261003)`, every other one quantised to 1/256, so the rank has long runs of ties),
eight colours and a half-hidden mask. Each arm that needs a new array alternates between two
copies made before the clock starts. One run is 3 rounds, the arm order reversed on every other
round; a round's figure is the median of 5 calls after 2 untimed ones; a run prints the median of
its 3 rounds and the load average at the start of each round. The runs alternated before, after,
before, after, before, after. Every headline figure is the median of the three runs' medians.

## 1. Before — `rankOf`, the comparator sort, no memo

| arm | run 1 | run 2 | run 3 | median |
|---|---:|---:|---:|---:|
| `styleFrom`, fresh weights | 366.877 | 237.062 | 352.829 | **352.829** |
| `styleFrom`, fresh weights and colours | 362.017 | 310.195 | 236.437 | **310.195** |
| `styleFrom`, reveal step (new mask only) | 370.182 | 355.279 | 250.138 | **355.279** |
| rank alone | 358.411 | 216.201 | 230.030 | **230.030** |
| radius loop alone | 5.343 | 3.764 | 3.726 | **3.764** |
| `bucketsOf` alone | 8.304 | 5.961 | 5.793 | **5.961** |
| load at rounds 1, 2, 3 | 17.57, 21.60, 24.25 | 23.17, 22.44, 20.99 | 20.06, 19.97, 18.90 | |

An earlier single run of the same bench on a lighter host (load 5.98 → 5.47) printed 215.673 ms
for a fresh-weights `styleFrom`, 215.062 ms for a reveal step and 206.310 ms for the rank alone.
`perf-p6-data-path.md` §4 put `styleFrom` at 115.2 ms on that job's host; the factor of two is the
host, and the rank's share (206 of 216 ms) matches its attribution.

## 2. After — `rankByWeight` and the two one-entry memos

| arm | run 1 | run 2 | run 3 | median |
|---|---:|---:|---:|---:|
| `styleFrom`, fresh weights | 21.623 | 20.009 | 34.269 | **21.623** |
| `styleFrom`, fresh weights and colours | 28.557 | 27.651 | 31.800 | **28.557** |
| `styleFrom`, reveal step (new mask only) | 0.001 | 0.001 | 0.001 | **0.001** |
| rank alone | 18.765 | 16.881 | 16.046 | **16.881** |
| radius loop alone | 3.370 | 3.393 | 3.858 | **3.393** |
| `bucketsOf` alone | 5.589 | 5.339 | 5.905 | **5.589** |
| load at rounds 1, 2, 3 | 23.88, 23.88, 23.88 | 20.59, 20.06, 20.06 | 18.59, 17.74, 17.74 | |

The load average refreshes every 5 s and an after run lasts about 2 s, so a run's three rounds
often read the same value.

## 3. The goal's budgets

| budget | before | after | verdict |
|---|---:|---:|---|
| reveal step, `styleFrom` ≤ 5 ms | 355.279 ms | 0.001 ms | **met** |
| weights changed, `styleFrom` ≤ 25 ms | 352.829 ms | 21.623 ms | **met**, by 3.4 ms; run 3 read 34.269 ms under load 18.6 |
| rank alone | 230.030 ms | 16.881 ms | 13.6× |

A change that moves both the weights and the colours (a new graph) costs 28.557 ms, 3.6 ms over the
weights budget: the bucket sort is the other 6 ms. The goal does not set a budget for that case.

The output is byte-identical: `tests/style-rank.test.ts` compares `rankByWeight` with the old
comparator sort element for element and the radius byte for byte (§4).

## 4. What changed

- `packages/graph-render/src/rank.ts` (new): `rankByWeight`, a stable LSD radix sort over a 32-bit
  key per weight, three 11-bit passes, scratch allocated per call. `-0` and NaN rank as `+0`. It is
  exact, so it carries no Caveat.
  - A pass whose digit is the same for every key is skipped. On the bench's weights none is: the
    top digit holds the exponent, and 0..1 spans 39 values of it (2048, 2048 and 39 distinct digits
    for passes 0, 1, 2, counted over the bench's input). All three are skipped on the all-zero
    weights of `plainStyle`.
- `packages/graph-render/src/style.ts`: `styleFrom` keeps two one-entry caches, `{radius,
  maxRadius, rank}` keyed by the `weights` object and the four sizing numbers, and `{bucketStart,
  bucketItems}` keyed by the `colours` object and the palette length. The Caveat on them names
  in-place mutation (§6).
- `packages/graph-render/tests/style-rank.test.ts` (new, 18 tests): the oracle cases (n = 0, 1, 2,
  all equal, mixed ±0, ±Infinity, negatives and subnormals, quantised ties, a seeded 100 000), NaN
  as +0, the negative control (reverse-order ties fail the same check), the radius byte for byte
  with NaN and out-of-range weights, and the memo cases for both caches.
  - The oracle catches a broken pass: with the third pass skipped unconditionally
    (`if (pass === 2) continue;`), the file fails (`not ok 1 - tests/style-rank.test.ts`).
  - `tests/look-style.test.ts:129` (`[3, 1, 2, 0]`) is unedited and green.

## 5. Gate

`scripts/orch/gate.sh target/rows-style-rank scripts/orch/rows/perf-style-rank.rows`, exit 1:

| row | exit | expect | verdict | time |
|---|---:|---|---|---:|
| `studio-check` | 0 | 0 | PASS | 62 s |
| `backend` | 0 | 0 | PASS | 20 s |
| `negctl-backend` | 1 | nonzero | PASS | 19 s |
| `filters` | 1 | 0 | **FAIL** | 19 s |
| `negctl-filters` | 1 | nonzero | PASS | 20 s |
| `smoke` | 0 | 0 | PASS | 4 s |
| `negctl-smoke` | 1 | nonzero | PASS | 4 s |

`studio-check` ran 515 graph-render, 605 graph-studio and 100 render tests, 0 failed and 0 skipped.

The `filters` row fails on two of its 15 rows, `colour-by-metric` and `groups-override-metric`,
both with "the probe could not judge it: `analysis` refused: Buffer is not defined". This change
did not cause it:

- The same row fails the same way on `a858d803` with this change stashed (`scripts/studio.sh build`
  exit 0, then `scripts/studio-filters.sh` exit 1, the same two rows).
- `crates/graph-sdk-js/src/analysis-face.ts:62` calls Node's `Buffer.compare`, which the browser
  does not have (`adapters/cells.ts:47` does too). It came in with `cfec8cd0`, before this branch,
  and `origin/develop` (`2f413858`) still has it.
- The fix is in `crates/`, which this job may not touch.

## 6. What it does not do

- `src/colour/normalise.ts:131` `ascending` is the same comparator sort over indices. It runs only
  when the colouring changes, so it was out of scope; it is the next candidate for `rankByWeight`'s
  method.
- **In-place mutation.** The caches are keyed by identity. A caller that rewrites a `weights` or
  `colours` array in place and passes the same object again gets the stale rank, radius and
  buckets. The studio allocates a new array per change (`graph-studio/src/look/styleOf.ts` builds
  both behind memos, and every write in `look/` and `source/meta.ts` goes into an array before it is
  handed out). Escape hatch: pass a new array.
- A sizing change (the node-scale or min/max sliders) re-runs the rank with the radius, about 17 ms
  at 1M, because the brief keys them as one entry. A rank entry keyed on `weights` alone would skip
  it.
- The caches are module-level, one entry each: two views styling different graphs in one page
  (two `<graph-studio>` elements) evict each other on every call and pay the full cost.
  `sceneOf`'s `plainStyle` fallback (`scene.ts:67`) evicts the studio's entry the same way, once.
- The radix pass skip does not fire on weights spread over 0..1 (§4).
