# Job perf-style-rank (agent build: a restyle at 1M does not re-sort what did not change)

Goal: a reveal step at 1M spends ≤ 5 ms in `styleFrom` (now 115 ms), and a restyle whose weights did
change spends ≤ 25 ms. The output stays byte-identical.

Owner note: `packages/graph-render` belongs to peer graph-render-08, which was not live on 2026-10-03
19:35. No open branch touches `src/style.ts` or `src/labels.ts` (checked with `git diff` against every
unmerged remote branch). Edit only the paths listed below.

Facts (verified on develop after perf-p6-data-path landed):
- `docs/measurements/perf-p6-data-path.md` §4–5: `styleFrom` costs 115.2 ms of a 117 ms restyle at 1M.
  The 5 Node rounds were 129.1, 117.4, 110.8, 115.2 and 113.0 ms.
- `packages/graph-render/src/style.ts:120-124` `rankOf` fills a `Uint32Array(n)` with 0..n-1 and sorts it
  with the closure comparator `(w[b] ?? 0) - (w[a] ?? 0) || a - b`. That is O(n log n) comparator calls.
- `style.ts:126-155` `styleFrom` also allocates and fills `radius` (`Float32Array(n)`, `radiusFor` then
  `clamped`, `style.ts:92-99`) and runs `bucketsOf` (`style.ts:101-118`, an O(n) counting sort) on every call.
- Studio callers:
  - `packages/graph-studio/src/studio/pipeline.ts` calls it on every restyle (`git grep -n styleFrom`).
  - `packages/graph-studio/src/state/keepForces.ts:49` calls it only for `.maxRadius`, when the frame has
    no `r`/`w`/`h`.
- After perf-p6-data-path, `packages/graph-studio/src/look/styleOf.ts` memoizes the weights
  (`WEIGHTS.read([meta, sizeBy, analysis])`) and the colours (`COLOURS.read(...)`). A reveal step
  changes only the hidden mask (`look/reveal.ts:7-12` copies it and never writes the weights or colours).
  So on a reveal step, `input.weights` and `input.colours` are the same objects as on the last call.
- `Style.rank` has one reader: `packages/graph-render/src/labels.ts:181-191` `planLabels`, which walks
  the rank in order until its budget runs out.
  - Its contract: "Node indices, heaviest first; ties keep index order."
  - `tests/look-style.test.ts:129` pins `[3, 1, 2, 0]`.
- Nothing writes into `style.radius`: `git grep -nE '(weights|radius)\[[^]]+\]\s*=[^=]' packages/` finds
  only `style.ts:132` (`three/projection.ts` writes its own `out.radius`).
- `src/colour/normalise.ts:131` `ascending` is a second comparator sort of the same shape. It is out of
  scope: it runs only when the colouring changes. Name it in the report as the next candidate.

Do, in order:
1. Baseline. Write `packages/graph-render/tests/bench-style.ts`, a printing bench modelled on
   `tests/bench-tween.ts`: no asserts, not a `.test.ts`.
   - Input: n = 1 000 000 seeded weights in 0..1 (`mulberry32`). Half are quantised to 1/256 so
     there are many ties.
   - Time `styleFrom`, and separately the rank, the radius loop and `bucketsOf`.
   - 3 rounds, alternating arms; report medians and `/proc/loadavg`.
   - Run it with `scripts/orch/node-slim.sh node --experimental-strip-types packages/graph-render/tests/bench-style.ts`.
2. Replace `rankOf` with an O(n) stable LSD radix sort over a 32-bit key, in a new
   `packages/graph-render/src/rank.ts` exporting `rankByWeight(weights: Float32Array): Uint32Array`.
   - Canonicalise each weight first: `-0` and `NaN` become `+0`. `NaN` matches what `radiusFor` does;
     the old comparator's NaN order was undefined.
   - Then take its f32 bits `u`. The ascending key is `u ^ 0x80000000` for a positive and `~u` for a
     negative. Invert it for heaviest-first.
   - Start from the identity order. A stable LSD pass keeps index order inside ties.
   - Use 11-bit digits (3 passes). Skip a pass whose histogram puts all n items in one bucket:
     weights in 0..1 share their top bits.
   - Allocate the scratch per call (two `Uint32Array(n)`). Pooling is not needed once step 3 makes the
     calls rare.
   - Caveat: none is needed for the sort, which is exact. State that plainly.
3. Memoize in `styleFrom`, with one entry each:
   - `{ radius, maxRadius, rank }`, keyed by the `weights` reference plus
     `sizing.base`/`gain`/`min`/`max`;
   - `{ bucketStart, bucketItems }`, keyed by the `colours` reference plus the palette length.
   - Write the `Caveat:` line: a caller that rewrites a weights or colours array in place and passes the
     same object again gets the stale order and radius. The studio callers allocate a new array per
     change (cite `styleOf.ts`); state the escape hatch, which is to pass a new array.
   - The cache keeps one entry and is replaced on a miss, so memory stays bounded.
4. Tests in a new `packages/graph-render/tests/style-rank.test.ts` (`node:test`, ≤ 300 lines):
   - **oracle:** `rankByWeight` equals the old comparator sort, kept in the test as the reference,
     element for element. Cases:
     - n = 0, 1 and 2;
     - all weights equal;
     - mixed `+0`/`-0`;
     - `±Infinity`;
     - quantised ties;
     - a seeded 100 000.
   - **NaN:** one NaN ranks where a `+0` at the same index would.
   - **sensitivity (negative control):** a variant that breaks ties in reverse index order must differ
     from the reference on the tie fixture. A test that cannot tell stable from unstable proves nothing.
   - **radius:** byte-equal to the per-node `clamped(radiusFor(w, sizing), sizing)` loop, and the same
     `maxRadius`.
   - **memo:**
     - the same `weights` and `sizing` return the same `rank`/`radius` objects (`===`);
     - a new array with the same contents gives equal values;
     - a changed `sizing.gain` gives a new radius;
     - the same holds for the colours and the buckets.
   - The existing `look-style.test.ts` rank case stays green and unedited.
5. Run step 1's bench again on the new code. Report three figures, each with the medians of 3
   alternating rounds:
   - a call with fresh weights;
   - a call with the same weights and a new hidden mask (a reveal step);
   - the rank alone.
   Say that the host was loaded.
6. Report `docs/measurements/perf-style-rank.md`:
   - the before and after tables with the load;
   - the gate table with exit codes;
   - whether the goal's two budgets were met or missed;
   - a "what it does not do" list: `normalise.ts:131`, and in-place mutation (the Caveat above).

Paths you may edit:
- `packages/graph-render/src/style.ts`;
- `packages/graph-render/src/rank.ts` (new);
- `packages/graph-render/tests/style-rank.test.ts` (new);
- `packages/graph-render/tests/bench-style.ts` (new);
- `docs/measurements/perf-style-rank.md`.

Nothing in `packages/graph-studio` (peer graph-render-6f is on force-warm-seed), `labels.ts`, `crates/`
or `app/`. No new dependency, and no type assertions. House limits: 40 lines a function, 300 a file,
4 parameters.

Done when:
- `scripts/studio.sh check` exits 0.
- `scripts/orch/gate.sh target/rows-style-rank scripts/orch/rows/perf-style-rank.rows` writes a
  `summary.txt` with every row PASS. That means:
  - `studio-check`, `backend`, `filters` and `smoke` exit 0;
  - `negctl-backend`, `negctl-filters` and `negctl-smoke` exit non-zero.
- The bench shows a reveal-step `styleFrom` at ≤ 5 ms median at 1M, or the report says by how much it missed.

Return: the branch tip, the before/after medians (styleFrom fresh, styleFrom on a reveal step, rank
alone), the gate summary lines, and every deviation from this brief.
