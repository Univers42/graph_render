# Job tier-no-tier (agent build, one layout id, a negative result)

Why: `prompts/RESUME.md` queues "Every layout gets the Phase 11 thread tier and a bench-driven
optimization pass. Order: easiest first." `layout.random` is the easiest job in the whole queue and
it produces **no code**: the layout is one sequential PRNG stream, so a thread tier cannot pay for
itself. This job's deliverable is the measured, recorded "no" — so the next reader stops re-asking,
and so `docs/measurements/tiers-audit.md` row 5 stops being a judgement and starts being a
measurement.

Facts:

- **The layout** — `crates/graph-core/src/layout/random.rs:25` `run`, hot loop `:26-28`:

  ```rust
  let mut stream = Mulberry32::new(SEED);
  let (x, y): (Vec<f64>, Vec<f64>) = (0..topology.node_count())
      .map(|_| (stream.next_f64(), stream.next_f64()))
      .unzip();
  ```

  `x` then `y` per node, in index order, off one fixed-seed stream (`SEED` at `random.rs:22`; "the
  same graph must hash the same on every target", `random.rs:5-7`). Node *i*'s pair is draws `2i`
  and `2i+1`, and the state at draw `2i` depends on every draw before it. **This is not a gather.**
  There is no edge work, no reduction, and no second O(n) term to overlap the stream with — the
  layout *is* the stream.

- **The arithmetic that kills it**: a chunk-seeding trick (advance the state serially once per
  chunk, then generate each chunk in parallel from its own seeded `Mulberry32`) is bit-identical,
  but the serial prologue is O(n) and the per-node transform is O(1), so Amdahl puts the ceiling
  below 2×. And the one crossover measurement in the tree says threads **lose** below n=10 000 and
  win above it (`docs/measurements/phase11-threads.md`: 0.52x..0.75x at n=220, 1.34x at 2 workers /
  2.97x at 7 workers at n=10 000). For this layout the prologue is the dominant term by
  construction, so the ratio is bounded *below* 1 at every width the gate runs.

- **`hashgate::WORKER_COUNTS = [1, 2, 3, 4, 7]`** (`crates/graph-cli/src/hashgate/tier.rs:45`) —
  widths `[1,2,3,4,7]`. `bench --workers 8` is refused by `refuse_unproved_widths`
  (`bench/tiers.rs:219-229`) and must stay refused: a width no gate arm ran cannot become a
  threshold row.

- **What the threaded hashgate arm does today**: `crates/graph-cli/src/hashgate.rs:169` is
  `let bytes = if id == BarnesHut::ID { ...run_under(..&Threads..) } else { bytes }`. Every other
  stage, this one included, reuses the scalar run's bytes (`:181-183`). So this layout is already
  "equal" at every width — vacuously, because no threaded arm recomputed it. That is the correct
  state for it, and this job must **leave it that way**.

Do:

1. **Do not** add `layout.random` to `hashgate.rs:169`. A threaded arm that recomputes nothing is
   worse than no threaded arm: it prints "10-way equal" for a stage no arm computed, and that is
   the exact claim the gate exists to make false. If you add the id, you have to add a `run_with`
   and a `StepRange` kernel, which is the thing this job exists to argue against. Write down the
   one-line reason in the module doc at `random.rs:1-11` instead, next to the existing Ponytail.
2. **Measure it anyway, on the plain bench path**, so the "no" has a number behind it:

   ```sh
   scripts/orch/gr cargo run --release -p graph-cli -- bench \
     --layout layout.random --n 220,10000,100000 --repeat 5
   ```

   Two things to get right in how you read that output. `--repeat` is **ignored** on the plain
   path: `bench.rs:205-207` times one `Instant` pair per (size, layout) and prints one run — "Time
   is one run, wall clock, native, not a median" (`bench.rs:20`). Only the campaign path
   (`--out` / `--crossover`, `bench.rs:150-151`) takes the median (`campaign.rs:183-192`). And
   `--n 100000` is under this layout's own ceiling, so nothing is refused. Record the wall time and
   say plainly that it is one run, not a median — or add this layout to the campaign path if a
   median is what the comparison needs.
3. **Write the negative into `docs/measurements/tiers-audit.md` row 5**: the `speedup` cell becomes
   a measured number rather than an Amdahl argument, and the `effort` cell keeps reading **S** with
   the deliverable named as "one recorded row, not code". Do not delete the row and do not mark the
   layout as "not gated" — it is in `LAYOUTS` (`registry.rs:167`), so the gate hashes it every run;
   what it does not get is a *tier*.
4. **Prove the gate still hashes this layout**, so "no tier" is not confused with "no coverage".
   The knob that must turn it red is **`GM_MUTATE_NODE_COUNT`** (`hashgate/knob.rs:46-47`, variant
   `NodeCount`, already in `Knob::ALL` at `:109`): it adds nodes to the model, which changes how
   many draws the stream consumes, so every coordinate moves and this stage's hashes must change.
   Note it is **native arm only** (`knob.rs:46`), so the two wasm32 arms stay equal and only the
   native ones diverge — that is the expected shape for a "is the stage hashed" control, not a
   partial failure. That control answers a different question from `GM_MUTATE_SPLIT_SUM`'s "did
   the threaded arm recompute it" (`knob.rs:85-86`), and for this layout it is the only one of the
   two that applies. **No new knob.**

Hashgate arm: **none**, by design. `--tiers all` (`hashgate/tier.rs:82-91`) adds `native scalar`
plus one `native threads N` arm per `WORKER_COUNTS` = 10 arms, compared line-for-line against arm 0
(`compare.rs:54-56`) and printed as `{ways}-way equal` (`hashgate.rs:227-231`). Those arms exist
for Barnes-Hut. For this layout the base 4 arms (`native run 1/2`, `wasm32 run 1/2`) are the whole
claim: one program, twice per target, identical bytes. Say so in the report.

Gate:

```
hashgate --seeds 8                            -> 0, and layout.random is in the tally
GM_MUTATE_NODE_COUNT=... hashgate --seeds 8   -> non-zero, and layout.random is among the diverged stages
hashgate --seeds 8 --tiers all                -> 0, layout.random 10-way equal (vacuously, and stated as such)
```

The last row is the one to be suspicious of. If it goes red, something added the id to
`hashgate.rs:169` — that is the bug this job exists to prevent, and the red is correct
behaviour. Do not "fix" it by reverting the red without checking that line.

Done when: the plain-bench wall time for `layout.random` at n=220, 10 000 and 100 000 is recorded
with its own noise caveat, `docs/measurements/tiers-audit.md` row 5 cites that measurement instead of
reasoning alone, the module doc at `random.rs:1-11` carries the one-line reason in place of a tier,
the id is still absent from `hashgate.rs:169`, and `GM_MUTATE_NODE_COUNT` demonstrably turns this
stage red. UNKNOWN = FAIL: an unmeasured "no" is a guess wearing a decision's clothes, and the
whole value of this job is that the negative is real.

Paths you may touch: `crates/graph-core/src/layout/random.rs` (module doc only — no behaviour
change), `docs/measurements/tiers-audit.md` row 5, and any new
`docs/measurements/tier-random.md`. Nothing else.
