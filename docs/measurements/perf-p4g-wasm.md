# P4g — the last wasm32 gap on the 1M live-growth budget: what was measured, what was taken, and what was not

`docs/measurements/perf-p4f-wasm.md` closed the JS encoder and left two wasm arms **over** the
30 ms budget on the same stream: Barnes-Hut at 33.00 ms and particle mesh at 37.90 ms, medians
of 3 medians at 1M nodes. It named three remaining costs and, for the largest, said what would
have to be measured before anything was changed. This document is that measurement round.

**Both wasm arms are met on a quiet host, and only there.** Against the true `75c885b6`,
re-measured interleaved with the final tree at matching load (1-minute load 4–7), Barnes-Hut goes
**33.42 → 28.86 ms** and particle mesh **38.05 → 28.36 ms**; on the quietest round (load ≤ 2.7)
the final tree measures 28.37 / 27.62 ms. Headroom is 1.1–2.4 ms, and load takes it: Barnes-Hut
measured 31.93 ms in a round at load 5–7, and both arms 41–43 ms at load ≥ 11.

**One of the three named costs was not taken, and the measurement that says so is below.** The
`StringArena` hash is **not** the cost: over a million ids the probe's cache misses are 70 % of
`find` and the hash is 29 %, and a word-at-a-time FNV-1a recovers only 12–13 % of `find`. That is
a measurement and a decision, not an omission.

**Fix 1 is the larger win, and the first draft of this document said the opposite.** Growing the
mesh in place, instead of `Mesh::new` per batch (~80 MB zero-filled at 1M), cuts the particle-mesh
`grow` **10.02 → 3.93 ms** in wasm32 and **6.68 → 3.39 ms** natively; dropping the `px`/`py`
`clear()` (16 MB a batch) cuts Barnes-Hut's 4.71 → 3.58 and 3.89 → 2.77 ms. The first `before`
round ran fix 1's Rust, not `75c885b6`, which is why it showed 0.03 ms — see "The true base".

## Method

Unchanged from P4f: one stream, four arms, 3 rounds, median of 3 medians.
`target/bench/stream-1m.jsonl` — **11 lines, 497 729 524 bytes**, line 0 at 900 000 nodes. Line 0
builds the graph untimed; each of the ten batches after it is timed as `extend` then `grow`, in
two timers that never contain each other, with `tick(1)` outside them.

| arm | command |
|---|---|
| native | `GR_MEM=12g timeout 3000 scripts/orch/gr cargo run -q --release -p graph-cli -- tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl --layout barnes-hut\|particle-mesh --path columns` |
| wasm32, split | `scripts/orch/node-slim.sh node --experimental-strip-types harness/wasm-stream-bench.mjs --from target/bench/stream-1m.jsonl --engine barnes_hut\|particle_mesh --path columns --split` |

The fixed alternating order inside each round is **native BH → wasm BH → native PM → wasm PM**.
One 1M process at a time. Before **every** process the host gate was checked — `free -g`
available ≥ 12 GB and 1-minute load < 14 — and on a failure the process waited 60 s and
re-checked, inside a 45-minute budget for the run. All 36 processes ran and every one exited 0;
**the gate cleared on the first check in all 36**, no wait anywhere. Raw output, one file per
process and one per gate check, under `target/wf/p4g-step/{before,fix1,final}/`. The re-measure
of the true base (24 more processes, same gate, every one exited 0) is under
`target/wf/p4g-step/{base2,final2}/`.

The three source states were three file trees, swapped by copy (`target/wf/p4g-step/swap.sh`),
never by a git state change:

| tree | what it is |
|---|---|
| `base` | the tip this slice started from, **`75c885b6`** — measured only as `base2`; the `before` round ran `fix1`'s Rust |
| `fix1` | `base` + fix 1 (six Rust files), JS at `base` |
| `final` | `fix1` + fix 3 (three JS files) — the tree this slice returns |

### **Caveat: host load moves these numbers by more than the fixes do.**

1-minute load at the process starts: 5.58 → 6.16 for `before`, 2.44 → 2.60 for `fix1`,
1.84 → 2.62 for `final`; in the interleaved re-measure 4.2–7.0 for pair 1, 3.9–11.4 for pair 2
and 11.0–15.4 for pair 3. At load ≥ 11 the wasm arms gain 10–15 ms (`final2` round 3: 41.22 /
43.39 ms). The control is the native `extend` column — no fix touches it and it does not run the
JS encoder — which reads 12.20 / 11.95 ms in `base2` round 1 against 12.52 / 12.11 ms in `final2`
round 1. Native `sum` is not a control across fix 1, which moves native `grow`. Only pairs
measured back to back at matching load are compared.

### Before — the first round, which ran `fix1`'s Rust

| round | arm | encode median | extend median | grow median | sum median | sum p95 | sum max | load start → end |
|---:|---|---:|---:|---:|---:|---:|---:|---|
| 1 | native BH | — | 11.52 | 2.71 | 14.72 | 30.77 | 42.04 | 5.57 5.73 5.25 → 4.87 5.54 5.20 |
| 1 | wasm BH | 18.63 | 28.28 | 3.45 | 32.36 | 81.43 | 119.60 | 4.87 5.54 5.20 → 4.45 5.34 5.15 |
| 1 | native PM | — | 12.10 | 3.32 | 15.93 | 35.65 | 48.99 | 4.45 5.34 5.15 → 4.38 5.29 5.14 |
| 1 | wasm PM | 19.17 | 28.42 | 3.89 | 33.11 | 46.78 | 55.70 | 4.38 5.29 5.14 → 4.37 5.24 5.12 |
| 2 | native BH | — | 11.97 | 2.76 | 15.50 | 30.82 | 41.90 | 4.37 5.24 5.12 → 4.24 5.10 5.08 |
| 2 | wasm BH | 18.84 | 28.36 | 3.60 | 32.51 | 94.60 | 122.72 | 4.24 5.10 5.08 → 4.31 4.99 5.04 |
| 2 | native PM | — | 11.40 | 3.26 | 14.87 | 33.84 | 47.08 | 4.31 4.99 5.04 → 4.34 4.98 5.04 |
| 2 | wasm PM | 20.02 | 30.88 | 4.12 | 34.39 | 49.52 | 55.22 | 4.34 4.98 5.04 → 4.50 4.98 5.04 |
| 3 | native BH | — | 11.74 | 2.81 | 14.67 | 30.86 | 41.47 | 4.50 4.98 5.04 → 4.69 4.97 5.03 |
| 3 | wasm BH | 19.44 | 31.26 | 3.43 | 34.84 | 93.86 | 133.29 | 4.69 4.97 5.03 → 6.11 5.21 5.10 |
| 3 | native PM | — | 11.55 | 3.33 | 15.10 | 33.65 | 47.56 | 6.11 5.21 5.10 → 6.16 5.25 5.11 |
| 3 | wasm PM | 20.06 | 27.93 | 3.87 | 31.76 | 46.36 | 53.53 | 6.16 5.25 5.11 → 5.89 5.24 5.11 |

| arm | encode | extend | grow | **sum** | sum vs 30 ms | verdict |
|---|---:|---:|---:|---:|---:|---|
| native BH columns | — | 11.74 | 2.76 | **14.72** | 0.49× | **met** |
| native PM columns | — | 11.55 | 3.32 | **15.10** | 0.50× | **met** |
| wasm BH columns | 18.84 | 28.36 | 3.45 | **32.51** | 1.08× | **MISSED** — 2.51 ms over |
| wasm PM columns | 20.02 | 28.42 | 3.89 | **33.11** | 1.10× | **MISSED** — 3.11 ms over |

**This round did not measure `75c885b6`.** Mid-job the orchestrator's sync committed the Rust
half of fix 1 as `4ca25aaa`, and the `before` tree was built from `HEAD`, so `before` ran fix 1's
Rust with `base`'s JS (`target/wf/p4g-step/versions.sh`'s comment says otherwise and is wrong).
Its `encode` column is `base`'s JS and valid; its `grow` and `sum` are `fix1`'s, at load 4.2–6.2.
That is why it measured wasm PM `grow` at 3.89 ms where P4f measured 10.18 ms.

### The true base, re-measured interleaved

`target/wf/p4g-step/remeasure.sh` writes every crate and harness file that differs from
`75c885b6` back to that commit (`git show`, restored by a trap), rebuilds the wasm module, runs
one round of the four arms, restores the final tree, rebuilds, and runs one round: three pairs,
`base2` then `final2`, under the same host gate. Pair 1, at matching load, is the comparison:

| arm (pair 1, load 4.2–7.0) | encode | extend | grow | **sum** |
|---|---:|---:|---:|---:|
| native BH columns | — | 12.20 → 12.52 | 3.89 → 2.77 | **16.61 → 15.50** |
| native PM columns | — | 11.95 → 12.11 | 6.68 → 3.39 | **19.02 → 16.00** |
| wasm BH columns | 22.02 → 16.64 | 28.22 → 26.06 | 4.71 → 3.58 | **33.42 → 28.86** |
| wasm PM columns | 18.45 → 13.62 | 27.42 → 24.08 | 10.02 → 3.93 | **38.05 → 28.36** |

| `sum`, base → final | pair 1 | pair 2 | pair 3 |
|---|---:|---:|---:|
| native BH columns | 16.61 → 15.50 | 16.26 → 15.34 | 23.74 → 18.87 |
| native PM columns | 19.02 → 16.00 | 20.26 → 18.40 | 18.62 → 22.72 |
| wasm BH columns | 33.42 → 28.86 | 32.74 → 31.93 | 47.27 → 41.22 |
| wasm PM columns | 38.05 → 28.36 | 40.03 → 42.05 | 38.93 → 43.39 |
| load at start, base / final | 4.2–5.9 / 4.8–7.0 | 3.9–5.7 / 5.3–11.4 | 11.6–15.4 / 11.0–12.1 |

`base2` reproduces P4f's tip: wasm PM `grow` 10.02 / 10.25 / 10.58 ms against P4f's 10.18. The
medians of three are not quoted as results: pair 2's final wasm PM and all of pair 3 ran at
load ≥ 11, and in pair 2 the final tree ran at up to three times the base's load.

## Fix 1 — grow the mesh in place, and stop zeroing `px`/`py`

`ForceSession::grow` ended in `self.mesh = Some(Mesh::new(rows))`. At 1M that is, per batch: a
fresh `Plan`, `density` and `spectrum` (1M `C` each at side 1024), a fresh `Kernel`, `at`, `Rows`
and `Grid::new(1M)` — **~80 MB zero-filled** — and a fresh kernel forces a kernel FFT on the next
tick because `Kernel::refresh` is keyed on `built_for`. Plus `px`/`py` were `clear()` +
`resize(rows, 0.0)`: **16 MB of zero-fill a batch** on both engines.

**What changed.** `Mesh::grow(n)` (new), `Rows::grow`, `Grid::grow`. `plan`, `density`,
`spectrum`, `kernel` and `blocks` are kept when `side_for(n)` is unchanged and rebuilt when it
moves; `at`, `Rows` and `Grid` are resized; `frame` is set to `None`. `px`/`py` are resized
without clearing.

**Why the output cannot move.** The contract is `grow.rs`'s own: the session after `grow` is
bit-for-bit the session a `carry` gives. Three arguments, and one of them was wrong until the
test said so:

1. **Every kept field is fully overwritten before it is read, or validly keyed.** `plan` is a pure
   function of the side. `density` and `spectrum` are written by FFT passes whose every output is
   `side²` and whose `Runner` clears each range before the kernel writes it, and only rows
   `0..cells` are ever read. `blocks` is resized to `n / BLOCK` by `frame::bounds` and every box
   it folds is written. `kernel` is keyed on `(step, reach, dmin2, dmax2)`, and a key hit means
   the next solve would resample and retransform the *same* frame — so the spectrum it keeps is
   the one it would have built. The whole table is the doc comment on `Mesh::grow`.
2. **`px`/`py` are written before they are read, on both engines.** Barnes-Hut's
   `collide::prepare` writes `px[i]`/`py[i]` for `0..x.len()` before the tree is built and before
   any query reads them; particle mesh's `motion::project` does the same into them through the
   `Runner`, and every other use (`center`, `integrate`, `velocities`) is a `Runner` pass that
   clears and refills before reading. The *length* is what matters and is still exactly `rows`,
   because `barnes_hut::step::CollidePass::len` is `sim.px.len()`.
3. **The grid's two permutation columns are put back to the identity.** This one was wrong in my
   first implementation and `grow_equals_carry` caught it. `charge::apply` runs at
   `particle_mesh.rs:136` and `collide::apply` — the only thing that rewrites `grid.order` and
   `grid.slot` — at `:138`. So **the tick's charge reads the *previous* tick's sort.** A `Vec`
   resized with zeros is not a permutation, and a charge deposited against the last sort's
   permutation puts the new rows in the cells of whatever nodes the last sort put in their slots.
   `Grid::grow` restores the identity, which is what `Grid::new` holds.

### The side-boundary test, and its mutation

`session/tests/grow.rs::grow_across_a_side_boundary_equals_carry` grows a particle-mesh session
from **16 000** rows to **17 000** — past `128² = 16 384`, so `side_for` goes 128 → 256
(`ceil(sqrt(16 000)) = 127` rounds up to 128, `ceil(sqrt(17 000)) = 131` to 256) — and compares
the grown session against `carry(previous, &topology)` immediately after the batch and after 20
ticks on, by `to_bits` like `grow_equals_carry`. It asserts `mesh_side()` is 128 before and 256
after, and that it equals the carried session's, so a `grow` that kept the old plan cannot pass by
agreeing with itself.

**Negative control, run and reverted.** With `Mesh::grow`'s rebuild condition weakened to
`if side != self.plan.side() && false`:

```
test layout::force::session::tests::grow::grow_across_a_side_boundary_equals_carry ... FAILED
  assertion `left == right` failed: the crossing batch, 20 ticks on: x
test result: FAILED. 11 passed; 1 failed; 0 ignored
```

Exactly one test goes red — the new one. `grow_equals_carry`, whose 300 rows never cross a
boundary, stays green, which is the point of adding a second test. The mutation is not in the
tree.

### What fix 1 measured

Fix 3 is JS only and cannot move `grow`, so `base2` against `final2` isolates fix 1 in that column:

| `grow`, ms | `base2` r1 / r2 / r3 | `final2` r1 / r2 / r3 |
|---|---:|---:|
| native BH | 3.89 / 3.58 / 6.22 | 2.77 / 2.76 / 3.85 |
| native PM | 6.68 / 6.95 / 6.83 | 3.39 / 3.94 / 5.59 |
| wasm BH | 4.71 / 4.31 / 5.35 | 3.58 / 3.72 / 5.03 |
| wasm PM | 10.02 / 10.25 / 10.58 | 3.93 / 5.39 / 6.05 |

**The particle-mesh `grow` falls 6.1 ms in wasm32 and 3.3 ms natively in pair 1, and falls in
every pair**, including pair 2, where the final tree ran at the higher load. Barnes-Hut's ~1.1 ms
is the `px`/`py` zero fill. The agent's own `before` → `fix1` rounds (3.89 → 3.86 ms) compared fix
1's Rust with itself and measure nothing about it.

**The next cost, named and not measured.** `grow` still climbs with `n` — 2.52 ms at 920 000 rows
to 4.81 ms at 1 000 000, wasm particle mesh, `final` round 1 — with a constant 10 000-row batch.
That trend is *not* the mesh (the mesh is O(batch) now) and not `px`/`py`. It is in
`ForceSession::grow`'s own O(n) work — `absorb`, `relink`'s three column resizes, `place`, and the
amortised reallocation of `at`/`order`/`slot`/`grid.at` — and this slice did not measure which.

## Fix 2 — the `StringArena` hash: measured, and **not** changed

P4f left `StringArena::find` at 9.43 % and `intern` at 5.79 % of the profiled window and said the
next thing to do is measure whether the cost is the hash or the probe's cache misses. Measured,
natively, over a million ids shaped like the studio's (`node-0000000`, a 512-value `Group` set,
`é-node-…`, `🌿`), with `find` run over an arena holding all of them — three runs, `#[ignore]`d
so no gate runs it, then removed:

| | ns/op | share of `find` |
|---|---:|---:|
| `hash_one`, the byte-at-a-time FNV-1a (`arena.rs:47-52`) | **54.0 / 54.3 / 53.9** | **28–30 %** |
| the same FNV-1a eight bytes a step, little-endian, tail after | **30.3 / 30.3 / 31.8** | 17 % |
| `find`, whole | **181.8 / 196.2 / 183.3** | 100 % |
| `find − hash_one`: the probe, its cache misses and the compare | 127.8 / 141.9 / 129.4 | **70–72 %** |

**It is the misses.** Two thirds of `find` is walking a million-entry table's buckets and
comparing spans, and the hash is under a third of it. A word-at-a-time hash — the exact shape the
brief specifies: eight bytes a step, `u64::from_le_bytes`, `wrapping_mul`, the tail after, no
`unsafe`, no dependency, identical on native and wasm32 — **recovers 12–13 % of `find`** and
nothing more. Against a 2.51 / 3.11 ms gap that is well under a millisecond, and the brief's own
rule for this cost is explicit: *if it is the misses, say so with the numbers and do not change
the hash.*

**Not changed, and nothing else in `arena.rs` either.** For the record, had it been taken, the
blocker is `crates/graph-core/src/arena/tests.rs:68`
(`fnv1a_matches_the_published_64_bit_vectors`, guard F-35), which pins the FNV-1a *value* against
the published 64-bit vectors — and that file is **outside this slice's allowed paths**. Changing
`Fnv1a::write` would have required editing a test the brief does not let me touch, which is the
second and independent reason the answer here is the measurement above. `git grep` confirms the
rest of the audit anyway: every `FixedState` collection in graph-core is an `IndexMap`/`IndexSet`
(`index.rs`, `index/extend*`, `neighborhood.rs`, `weights.rs`, `layout/force/mod.rs`,
`yifan_hu/coarsen.rs`), whose iteration order is insertion order and not the hash; the only bare
`HashMap`/`HashSet` sites are membership-only; `arena` is a private module and `Fnv1a`/`FixedState`
are not in `lib.rs`'s re-export list, so no hasher reaches a snapshot, a wire format or a JSON
document.

## Fix 3 — the JS encoder: two walks, and the placement that was 160 000 `subarray` objects

### 3a. `utf8Length` in code units

`utf8Length` walked with `for (const chunk of value)` — a **code-point** iterator, which yields
**one string per code point**. On the 1M stream that is a throwaway string per character, a
megabyte of garbage a batch, to compute a number `charCodeAt` gives directly. It is now a
code-unit walk: the three UTF-8 width classes (`< 0x80` one byte, `< 0x800` two, the rest three)
with the surrogate range as the fourth — a high surrogate and the low one after it are one code
point and four bytes.

**The same number for every input, and the test that says so.**
`"a lone surrogate measures three bytes, the way the code-point walk measured it"` pins
`utf8Length("a\ud800b") === 5`, `"\udc00" === 3`, `"\ud800" === 3`, `"🌿" === 4`, `"" === 0`.
That test earned its place immediately: the first implementation widened the index past the next
code unit after *any* surrogate, so `"a\ud800b"` came out 4 and the test failed. It is written
against the exported function because `exactWidth` refuses a lone surrogate before `utf8Length`
ever sees one — by index, which is what the format needs.

### 3b. One `encodeInto` over the join, not one per entry

`placeBlob` did `out.subarray(cursor, cursor + width)` and `ENCODER.encodeInto` **per entry** — at
1M, 160 000 throwaway `Uint8Array` views and 160 000 `encodeInto` calls a batch. A well-formed
entry can neither open with a low surrogate nor close with a high one, so the joined text encodes
to exactly the concatenation its entries' widths named. `placeTable` places the whole table with
**one** `encodeInto` over the join and checks both halves: `read === text.length` (the whole text
consumed) and `written === blobBytes` (exactly the measured total). So a table that measured and a
table that encoded still cannot disagree — the check is now on the whole blob rather than per
entry.

Micro-benchmark, 160 000 entries and 1.76 MB of blob in the studio's own shape
(`target/wf/p4g-step/place-bench.mjs`, median of 9 runs after 3 warm-ups):

| | median | min |
|---|---:|---:|
| per entry (the shipped form) | **9.98 ms** | 9.94 ms |
| one `encodeInto` over the join | **4.39 ms** | 3.63 ms |
| the join saves | **5.58 ms** | |
| `measureBlob`'s width walk, **code-point** iterator (the shipped form) | **8.37 ms** | |
| `measureBlob`'s width walk, **code-unit** (`charCodeAt`) | **3.75 ms** | |

The cost was never the encoding. It was a `Uint8Array` view object per entry, and a string per
code point.

### Why the GMX1 bytes cannot move

Nothing in the format changed: the same entries, the same offsets (the running sums of the same
widths), the same blob length in the header word at byte 20, the same buffer length and the same
8-byte alignment. Two tests are the proof and both were run:

- **`test/extend-columns.test.mjs` "the GMX1 bytes of a fixed batch are a pinned literal"** — a
  three-node, three-edge batch carrying `id: "z-é🌿"`, `icon: "🌿"`, a `-0` weight, a subnormal,
  an absent optional and an edge onto a node the batch does not carry, pinned as a **hex literal
  and a SHA-256**, so a changed byte is one diff line. It forces the exact measuring path and the
  joined placement, which is the one under test.
- **`test/columns-blob.test.mjs` "an emoji beside a 2-byte and a 3-byte character"** (new) — the
  table `["🌿é", "€", "a", "🌿🌿", "é€", "𝄞", "￿"]`: all three width classes inside one entry and
  beside each other across entries, which is where a code-unit walk and a code-point walk part
  company. It pins the widths `[6, 3, 1, 8, 5, 4, 3]`, the offsets `[0, 6, 9, 10, 18, 23, 27, 30]`,
  the header's blob length, **and** that the blob decodes back to `table.join("")` — with one
  placement for the whole table, a skipped low surrogate or a miscounted class moves a byte and
  fails here rather than in a profile.

The SDK suite is **175 tests** (173 before, two added), green, and `sdk:typecheck` and
`sdk:lint --max-warnings=0` are both clean.

### A test this slice **changed**, and why

`columns-blob.test.mjs` had **"a non-ASCII table is never joined into one string"**, which counted
`Array.prototype.join` calls and asserted `joins === 0` for the exact path. P4f wrote it to pin a
copy that was **made and discarded**; fix 3b makes the join **read**, so that assertion is false
against the better code. It is replaced by **"a non-ASCII table is placed by one `encodeInto` over
the join, never entry by entry"**, which counts `TextEncoder.prototype.encodeInto` calls and
asserts **`placements === 1`** (and `joins === 1`), and which also checks the blob decodes back to
the table. That is a *stricter* pin on the new arrangement — a regression to per-entry views fails
it — but it is a different assertion, and a reviewer should read it as a change and not as a
strengthening of the old one. The sibling ASCII test was given the same `placements === 1` check,
so the two paths are pinned symmetrically.

### What fix 3 measured

| arm | encode before | encode after fix 1 | **encode final** | sum before | sum final |
|---|---:|---:|---:|---:|---:|
| wasm BH | 18.84 | 18.96 | **16.43** | 32.51 | **28.37** |
| wasm PM | 20.02 | 18.26 | **14.40** | 33.11 | **27.62** |
| native BH | — | — | — | 14.72 | 14.86 |
| native PM | — | — | — | 15.10 | 15.21 |

`encodeBatch` fell **2.41 ms (13 %)** on Barnes-Hut and **5.62 ms (28 %)** on particle mesh, and
the native control moved **+1.0 %** and **+0.7 %**. Both `before` and `final` ran fix 1's Rust,
so this table is fix 3 alone plus the host, not `base` → `final`. The micro-benchmark predicted 5.58 ms of
placement and 4.6 ms of measuring; the arm saw 2.4 and 5.6. The micro-benchmark's table is
synthetic and its blob is larger than the stream's, so the two are the same sign and the same
order, not the same number — and the arm is the number that counts.

## Final — all four arms, 3 rounds, the `final` tree

| round | arm | encode median | extend median | grow median | sum median | sum p95 | sum max | load start → end |
|---:|---|---:|---:|---:|---:|---:|---:|---|
| 1 | native BH | — | 11.74 | 2.73 | 14.70 | 29.98 | 41.34 | 2.10 2.29 2.56 → 2.19 2.29 2.55 |
| 1 | wasm BH | 15.63 | 24.53 | 3.54 | 28.37 | 67.83 | 97.87 | 2.19 2.29 2.55 → 2.41 2.34 2.55 |
| 1 | native PM | — | 11.65 | 3.24 | 15.48 | 34.90 | 49.89 | 2.41 2.34 2.55 → 2.42 2.34 2.55 |
| 1 | wasm PM | 14.83 | 24.25 | 4.06 | 28.86 | 54.65 | 55.36 | 2.42 2.34 2.55 → 2.41 2.34 2.55 |
| 2 | native BH | — | 11.60 | 2.70 | 14.86 | 30.20 | 41.83 | 2.41 2.34 2.55 → 2.48 2.36 2.54 |
| 2 | wasm BH | 16.43 | 25.94 | 3.58 | 29.27 | 71.53 | 101.70 | 2.48 2.36 2.54 → 2.28 2.33 2.52 |
| 2 | native PM | — | 11.67 | 3.25 | 15.08 | 33.73 | 47.73 | 2.28 2.33 2.52 → 2.23 2.31 2.52 |
| 2 | wasm PM | 14.31 | 23.64 | 3.89 | 27.62 | 42.91 | 51.71 | 2.23 2.31 2.52 → 2.74 2.42 2.55 |
| 3 | native BH | — | 12.33 | 2.71 | 15.52 | 30.68 | 41.32 | 2.74 2.42 2.55 → 2.72 2.46 2.56 |
| 3 | wasm BH | 16.43 | 24.83 | 3.54 | 27.59 | 68.16 | 99.04 | 2.72 2.46 2.56 → 2.64 2.46 2.55 |
| 3 | native PM | — | 11.53 | 3.26 | 15.21 | 34.26 | 48.89 | 2.64 2.46 2.55 → 2.62 2.46 2.55 |
| 3 | wasm PM | 14.40 | 23.68 | 3.94 | 27.56 | 43.67 | 54.08 | 2.62 2.46 2.55 → 2.56 2.46 2.55 |

### `before` → `fix1` → `final`, median of 3 medians (all three ran fix 1's Rust)

| arm | encode | extend | grow | **sum** | vs 30 ms | change |
|---|---:|---:|---:|---:|---:|---|
| native BH columns | — | 11.74 → 11.66 → 11.74 | 2.76 → 2.75 → 2.71 | **14.72 → 14.59 → 14.86** | 0.49× → 0.50× **met** | +1.0 % (the host) |
| native PM columns | — | 11.55 → 11.52 → 11.65 | 3.32 → 3.32 → 3.25 | **15.10 → 15.01 → 15.21** | 0.50× → 0.51× **met** | +0.7 % (the host) |
| wasm BH columns | **18.84 → 18.96 → 16.43** | 28.36 → 27.70 → 24.83 | 3.45 → 3.53 → 3.54 | **32.51 → 31.75 → 28.37** | 1.08× → **0.95× met** | **−12.7 %** |
| wasm PM columns | **20.02 → 18.26 → 14.40** | 28.42 → 28.67 → 23.68 | 3.89 → 3.86 → 3.94 | **33.11 → 32.67 → 27.62** | 1.10× → **0.92× met** | **−16.6 %** |

### Verdicts

| arm | quiet (`final`, load ≤ 2.7) | matched (`final2` r1, load 4.4–7.0) | under load | verdict |
|---|---:|---:|---|---|
| native, Barnes-Hut, `columns` | 14.86 ms | 15.50 ms | 18.87 ms (load 12–17) | **met** |
| native, particle mesh, `columns` | 15.21 ms | 16.00 ms | 22.72 ms (load 11–13) | **met** |
| wasm32, Barnes-Hut, `columns` | 28.37 ms | 28.86 ms | 31.93 ms (load 5–7), 41.22 ms (load 11–12) | **met on a quiet host**, 1.1–1.6 ms headroom; **missed under load** |
| wasm32, particle mesh, `columns` | 27.62 ms | 28.36 ms | 42.05 ms (load 11–12), 43.39 ms (load 11) | **met on a quiet host**, 1.6–2.4 ms headroom; **missed under load** |

**P4 is met on a quiet host, not robustly.** The wasm margin, 1.1–2.4 ms, is smaller than the
spread one loaded round adds, so a gate run on this host at load ≥ 7 can read red on the same tree.

**Next cost, named.** With `encode` down to 16.43 / 14.40 ms, `extend − encode` is 8.40 / 9.28 ms
and is the largest single item left in either wasm timer. P4f's profile put 28.66 % of the window
in the wasm32 motor and named `StringArena::find` (9.43 %) and `intern` (5.79 %) inside it; this
document has measured that the hash is 29 % of `find` and the probe is 71 %, so **the probe is the
next cost, not the hash** — a million-entry `IndexMap`'s bucket walk and its cache misses, which
nothing in `arena.rs` can fix and which this slice did not attempt. In wasm particle mesh, `grow`
still climbs 2.5 → 4.8 ms across the ten batches with a constant 10 000-row batch, and the O(n)
work behind that is named above and unmeasured.

## Reproducing

```sh
# the module, and the 1M stream (P4e's file; emitted once if absent)
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --n 1000000 --stream 10000 --batches 10 --emit target/bench/stream-1m.jsonl

# a wasm arm, split: `encode` beside `extend` and `grow`. `extend - encode` is (b)+(c).
scripts/orch/node-slim.sh node --experimental-strip-types harness/wasm-stream-bench.mjs \
  --from target/bench/stream-1m.jsonl --engine barnes_hut --path columns --split

# a native arm
GR_MEM=12g timeout 3000 scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl \
  --layout barnes-hut --path columns

# the three source states and the three rounds (file copies; no git state is touched)
target/wf/p4g-step/versions.sh          # base = 75c885b6, fix1, final
target/wf/p4g-step/swap.sh fix1         # or base / final
target/wf/p4g-step/round.sh <label> native-bh wasm-bh native-pm wasm-pm
target/wf/p4g-step/remeasure.sh         # 75c885b6 vs the tip, interleaved: base2/, final2/
scripts/orch/node-slim.sh node target/wf/p4g-step/read-round.mjs <label>

# the placement micro-benchmark and the SDK suite
scripts/orch/node-slim.sh node --experimental-strip-types target/wf/p4g-step/place-bench.mjs
scripts/orch/node-slim.sh node --test --experimental-strip-types \
  'crates/graph-sdk-js/test/**/*.test.mjs'
```

Three rounds are those two commands over the four arms in the fixed alternating order above, each
preceded by `free -g | awk '/^Mem:/ {print $7}'` ≥ 12 and `awk '{print $1}' /proc/loadavg` < 14,
waiting 60 s and re-checking otherwise. `target/wf/p4g-step/` is generated output and is not
committed.

## Caveats

- **Load moves these numbers by more than the fixes do.** At load ≥ 11 the wasm arms gain
  10–15 ms. Only pairs measured back to back at matching load are compared, and `base2`/`final2`'s
  medians of three are not results: pair 2's final wasm PM and all of pair 3 ran at load ≥ 11.
- **The `before` round is not `75c885b6`.** It ran fix 1's Rust (`HEAD` was `4ca25aaa` when its tree
  was built) with `base`'s JS. Its `encode` column is valid; its `grow` and `sum` are fix 1's.
  `base2` is the true base, and it reproduces P4f's wasm PM `grow` (10.02–10.58 ms against 10.18).
- **`before` and `fix1` are the same Rust**, so their 0.03 ms `grow` agreement says nothing about
  fix 1; the `base2`/`final2` `grow` columns do.
- **The hash measurement is native, one shape of id, three runs.** 28–30 % of `find` for the hash
  and 70–72 % for the probe is a consistent three runs, but it is x86-64; wasm32's ratio could
  differ, and nothing here measures it. The conclusion drawn is the conservative one — the hash is
  the minority cost and the word-at-a-time form recovers only 12–13 % — so it is the reading least
  likely to be wrong on the other target.
- **The micro-benchmark is synthetic**: 160 000 entries of one shape and a 1.76 MB blob, not the
  stream's table. Its numbers (5.58 ms of placement, 4.6 ms of measuring) predicted an arm
  movement of 2.4 and 5.6 ms. Use it for the ratio, not the absolute.
- **The split arm does twice the encode work.** Under `--split` each batch runs `encodeBatch` once
  for the `encode` timer and again inside the timed `extend`. The `extend` column is measured
  around its own call and is unaffected; the *process* does twice the GC work, so the split arm is
  not a control for the default arm.
- **A p95 over ten batches is one interpolated value, not a tail**, on both sides —
  `stream/stats.rs`'s `R7` rule in both benches.