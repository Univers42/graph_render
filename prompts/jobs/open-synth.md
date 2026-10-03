# Job open-synth (agent build; the 1M-node open path, the studio's synthetic source)

Why: the synthetic source that `deploy/perf/drivers/hook.js` opens (`__perf.open(n, layout)`) spends
over a second of the 1M open in plain JavaScript before the motor sees a byte.

Facts (profile `deploy/perf/open.py 1000000 webgl2` on the perf-p5 tree, SwiftShader, load ~10;
worker 14.77 s sampled; re-run the probe for your own baseline):
- Worker self rows in minified JavaScript: `Kt` 1249 ms and `Vt` 1131 ms (`app/dist/assets/
  worker-*.js`); find which source functions they are before changing anything.
- `packages/graph-studio/src/source/synthetic.ts:166` `applyDegreeWeights` builds a
  `Map<string, number>` over every node id, then does a string-keyed `get` and `set` for both ends
  of every edge.
- The generator in the same file builds the edges itself, so it knows each endpoint's node index
  when it makes the edge.

Do:
1. RED first: a `node:test` case in `packages/graph-studio/tests/synthetic-weights.test.ts` that
   keeps today's `applyDegreeWeights` as a test-local reference and asserts `deepStrictEqual` on
   the whole records (every weight) for at least 3 seeds and sizes (10, 1 000, 50 000 nodes), plus
   a graph with an isolated node and one with no edges (all weights 0).
2. Count degrees by node index (a `Uint32Array`), not by string id. Weight stays
   `count / max`, or 0 when `max` is 0. No other output may change: same records, same order, same
   ids, same numbers.
3. If `Kt` or `Vt` is another part of the generator, apply the same rule there: index arithmetic
   over typed arrays instead of string-keyed maps, with the same deepEqual reference test.
4. Measure with `scripts/studio.sh build` and then `deploy/perf/open.py 1000000 webgl2`, 3 runs
   before and 3 after, interleaved. Write `docs/measurements/open-synth.md`.

Out of bounds: `crates/` (jobs open-ingest and open-core), `packages/graph-render`, the perf-p6 UI
work in `packages/graph-studio/src/ui/`, and any public type. No new type assertions. No new
dependency.

Paths: `packages/graph-studio/src/source/synthetic.ts` (300-line limit; split if needed), the new test
file, `docs/measurements/open-synth.md`.

Done when: `scripts/studio.sh check` passes with the new test. quick.rows is green. The JavaScript
part of the 1M open falls by at least 0.5 s, with the measurement file committed. If it falls by
less, report the numbers: they are the result.
