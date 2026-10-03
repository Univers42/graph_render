# Perf open meta: the studio's id join skips its map when the snapshot keeps document order

Measured 2026-10-03 on branch `perf-open-meta` (from develop `9178255`). Host: dlesieur42,
i5-13600KF. Node 22 from `scripts/orch/node-slim.sh`.

Why: a CPU profile of a 400k open in the studio (`scripts/studio-probe.sh open 400000 webgl2`)
put 621 ms in the worker's `describe`, 404 ms of it in `metaOf` and 349 ms of that in `inOrder`,
which built a `Map` over every document node to join the snapshot's ids back to the document.

## Design

| Piece | Where | What changed |
|---|---|---|
| fast path | `source/meta.ts` `inOrder` | one `===` per node first; the `Map` join only when the orders differ. The motor's dense order is the document's (`index_model` admits nodes in order, first wins), so a document without duplicate ids always takes it |
| tags | `source/meta.ts` `NO_TAGS` | one frozen empty array shared by every node without tags, in place of one new array per node |
| loop | `source/meta.ts` `metaOf` | an indexed loop in place of `entries()` destructuring |

Both paths are O(n). A mismatch is still found and named by the `Map` join, as before.

## Gates

| Check | Result |
|---|---|
| `scripts/studio.sh check` | rc 0; node:test 445 + 564 + 98 pass, 0 fail, 0 skipped; eslint, tsc, vite build |
| mutation: the fast path always returns the document | `tests/meta.test.ts` fails 3 of 8 (the two snapshot-order tests and the unknown-id refusal); restored |

## Numbers

`scripts/orch/node-slim.sh node deploy/perf/meta-join.ts <n>`, the synthetic `random` graph,
median of 7 runs in one process. "Before" is develop's `meta.ts` dropped into the same tree. Two
rounds, arms alternated.

| n | before, rounds 1, 2 (ms) | after, rounds 1, 2 (ms) |
|---:|---|---|
| 400000 | 281, 349 | 68, 37 |
| 1000000 | 720, 712 | 180, 107 |

## What it does not do

- `describe` (`motor/session.ts`) still decodes every snapshot id into a fresh string before the
  join: about 200 ms of the 621 ms at 400k. Skipping that needs `session.ts`, which the
  `perf-open-synth-columns` job is editing.
- Caveat: the host was shared, load 49–62 during the run. The rounds move by up to 2× for the
  after arm; the ratio between arms held at 4–9× in every pair.
