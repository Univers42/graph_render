# open-synth — the JavaScript that builds the 1M-node document

Measured 2026-10-02 on host dlesieur42 (20 cores, load average 9.6 from other jobs),
worktree open-synth at `4c7f8f8`, image `gm-chromium` (software raster, SwiftShader),
back-end webgl2. The probe is `deploy/perf/open.py`, a 0.5 ms sampling profiler over the
page and its motor worker; the graph is the perf driver's own, opened at one million nodes.

## What changed

`packages/graph-studio/src/source/synthetic.ts` built the link counts in a
`Map<string, number>` keyed by node id — one entry per node, then a string-keyed `get` and
a `set` for **both ends of every edge**. At 1 000 000 nodes with degree 2 that is four
million string hashes over a million-entry map, and it cost more than the rest of the
generator put together.

The generator already knows both endpoints' node indices: it just drew them. So the counts
are now a `Uint32Array` filled in as each edge is made (`bump`, `synthetic.ts:119`), and the
weight pass (`applyDegreeWeights`, `synthetic.ts:199`) reads them. While there, the node id
strings are built once with the node that owns them and every edge end reuses that one
string (`edgeWriter`, `synthetic.ts:99`) instead of concatenating a fresh `n-${i}` per edge
end — that is three string allocations per edge removed, not a change of output.

`Kt` and `Vt` from the job's profile are both in the worker chunk, and they are not what the
job assumed. In the pre-change bundle `app/dist/assets/worker-57PHvDDv.js`:

| minified | source | what it is |
|---|---|---|
| `Kt` | `motor/session.ts:106` `generated()` | the whole JS side: call the generator, then `JSON.stringify` the document |
| `Jt` | `motor/session.ts:123` `documentFor()` | `Kt`'s caller |
| `Vt` | `source/synthetic.ts:179` `syntheticRecords()` | the generator, weight pass included |
| `Bt` | `source/synthetic.ts:166` `applyDegreeWeights()` | the `Map` pass, called from `Vt` |

So `Kt` was never a generator function: its self time is the `JSON.stringify` of a million
nodes and two million edges, in `packages/graph-studio/src/motor/session.ts`, which is not
this job's path and is untouched here. `Vt`/`Bt` are the generator, and they are what moved.

## The measurement

`scripts/studio.sh build` twice — once from `4c7f8f8`'s `synthetic.ts`, once from the rewrite
— then the two `app/dist` trees swapped between runs, so the runs interleave and a drifting
host hits both sides. Raw output: `target/open-synth/run-<side>-<n>.txt` (gitignored scratch).

```sh
scripts/studio.sh check                                                  # exit 0
scripts/studio.sh build                                                  # exit 0
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/open.py 1000000 webgl2                             # exit 0
```

Worker rows, milliseconds of self time, inclusive of callees, from each run's own profile.
`generated()` is the honest unit: it is every millisecond the motor spent in JavaScript
building the document, whatever the sampler happened to attribute inside it.

| run | `open s` | `generated()` incl | generator incl | weight pass incl |
|---|---:|---:|---:|---:|
| before 1 | 18.68 | 3590.9 | 2235.3 (`Vt`) | 1794.3 (`Bt`) |
| before 2 | 15.72 | 3846.7 | 2346.0 (`Vt`) | 1950.0 (`Bt`) |
| before 3 | 18.68 | 3530.1 | 1995.5 (`Vt`) | 1553.0 (`Bt`) |
| after 1 | 16.21 | 2527.5 | below the 40 printed rows | below the 40 printed rows |
| after 2 | 14.30 | 1804.4 | below the 40 printed rows | below the 40 printed rows |
| after 3 | 12.75 | 1717.8 | below the 40 printed rows | below the 40 printed rows |

Mean `generated()`: 3655.9 ms before, 2016.6 ms after — a fall of **1639 ms**. Median:
3590.9 → 1804.4, a fall of **1787 ms**. The probe prints 40 inclusive rows and the generator
is no longer among them: after the rewrite it is under the smallest printed row of each run
(653.6, 564.9, 500.7 ms), against 1995–2346 ms before.

A sampling profile on a loaded host cannot resolve a difference that small, so the generator
was also timed on its own, in one Node process with both modules loaded and the samples
alternating (`target/open-synth/probe-generator.ts`, gitignored scratch, wall clock around
`syntheticRecords` alone, no `JSON.stringify`):

| nodes | before median | after median | fall |
|---|---:|---:|---:|
| 1 000 | 2 ms | 2 ms | 0 ms |
| 10 000 | 16 ms | 6 ms | 10 ms |
| 100 000 | 124 ms | 40 ms | 84 ms |
| 1 000 000 | 2503 ms | 579 ms | **1925 ms** |

Five samples each, medians reported; the 1M row was 3509/2503/2625/2175/2028 before and
616/579/411/535/840 after. The fall is proportional to the graph and roughly linear after,
superlinear before — the signature of a hash map growing to a million entries.

**Ponytail:** software raster in a container on a shared host at load average 9.6; wall clock
moves with the load, never with the code. The `open s` column (median 18.68 → 14.30 s) is
reported because it is the number a user feels, but it is the noisiest figure here and it
also contains the wasm build, the ingest parse and the first layout. This probe is not the
gate and never was: the gate is `scripts/studio.sh check` and `quick.rows`.

## What the numbers say

| Finding | Evidence |
|---|---|
| The `Map` was the generator's whole cost at 1M | generator inclusive 1995–2346 ms before, unprinted after; 1925 ms fall in the isolated probe |
| The weight pass alone was ~1.8 s of it | `Bt` inclusive 1553–1950 ms across the three before runs |
| The rewrite is free below 10k nodes | 0 ms fall at 1000 nodes, 10 ms at 10 000 — nothing to lose at the sizes the studio shows by default |
| `Kt` is `JSON.stringify`, not the generator, and it is untouched | `motor/session.ts:113`, self 1355–2055 ms across all six runs; the largest single JavaScript row left on this path |
| Output did not move | `tests/ingest.test.ts:33` still matches its pinned byte string; `tests/synthetic-weights.test.ts` `deepStrictEqual`s every record against the old `Map` pass for 3 seeds × 3 sizes, both shapes |

Not measured here: the wasm half of the open (`gm_build`, the canonical-json parse and
`index_model` together are still ~8–9 s of the worker), `JSON.stringify` in
`motor/session.ts:113` and what it would cost to hand the records to the wasm as typed
arrays instead of as text, anything at a node count between 10 000 and 1 000 000 in the
browser, and any GPU. The `open s` column above was not used to justify the change.