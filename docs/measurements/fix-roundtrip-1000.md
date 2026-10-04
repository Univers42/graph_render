# fix-roundtrip-1000 — why `roundtrip --seeds 1000` ran 2 h and printed nothing

## Symptom

`develop-full.rows:42` `roundtrip-1000` ran 2 h at 100% of one core, 12.9 MiB RSS, an
empty log, and was killed. `roundtrip --seeds 4` takes 6 s warm. Nobody had run 1000
seeds since the row was written.

## Two independent causes, not one

**1. The row ran a debug build.** `cargo run -q -p graph-cli` with no `--release`. The
whole sweep is float arithmetic in tight loops — exactly the code a debug build runs 10x
to 30x slower on. The row was asking a debug build for a number the release build had
already been producing all along.

**2. Per-seed cost grows with the seed, because the node count does.**
`graph_core::gate_node_count(seed) = 2 + seed % 600`
(`crates/graph-core/src/stage/topology.rs:20`), so seed 0 gets 2 nodes and seed 599 gets
601. Cost per seed therefore rises across the sweep, and `roundtrip` runs all 40
registered layouts on every seed (`swept_layouts`, `roundtrip.rs:54`). The sum is not
linear in `--seeds`, which is why `--seeds 4` in 6 s said nothing about 1000.

No seed hangs. The stall was the tail of a superlinear sweep.

## Where the superlinear term is

Measured per layout, release build, `n=601`, seed 0, one `snapshot` run each
(`--layout <name> --nodes 601`), wall seconds. 0.22 s is process startup: 35 of the 40
layouts finish within noise of it.

| layout | n=601 wall (s) |
| --- | --- |
| `force.davidson_harel` | **7.51** |
| `force.fdp` | 1.43 |
| `circular.circo` | 1.22 |
| `packing.circle` | 0.91 |
| `force.neato` | 0.80 |
| `force.drl` | 0.62 |
| all other 34 | 0.22–0.53 |

`force.davidson_harel` alone is ~50% of the sweep's per-seed work at the largest size.
Its own complexity claim (`crates/graph-core/src/registry/igraph.rs:82`,
`DH_CEILING = 500` at `davidson_harel.rs:65`) is
`O(rounds * 30 * n * (n + deg(v) * m))` with 10 rounds and 30 candidate moves per node —
the gate's largest size, 601, is past that ceiling, and the note says past it there is no
refusal, only slowness. That is honest, and it is why the gate is slow.

## The root-cause fix (shared function, both callers)

`energy::delta` computed, for each of the 30 candidate moves of node `v`, the full energy
of the position `v` is *already* at:

- `crossings` re-ran `cross(p, pu, pa, pb)` — the same segment intersection, for the same
  unchanged geometry — once per candidate. That half is
  `O(deg(v) * m)` per candidate, and it is **the** term: `deg(v) * m` segment tests, 30
  candidates, `n` nodes, 10 rounds.
- `node_dist` re-ran `1.0 / d2(p, pu)` for all `n` nodes, per candidate, for the same
  reason.

Only `pos[v]` moves while `v` is being probed, and every edge incident to `v` is skipped
by the crossings term, so neither half depends on the candidate. `energy::resting`
(`crates/graph-core/src/layout/force/davidson_harel/energy.rs:56`) records both once per
position — the crossing flags in the exact order the reduction read them, the reciprocal
distances indexed by node — and `Probe` carries the record into `delta`. The record is
rebuilt only after a move is accepted.

`roundtrip` is the caller that made this expensive rather than the only one: `snapshot`,
`hashgate` and `bench` all reach the same `energy::delta`, so the fix is in the shared
function and every caller gets it.

This is a cache, not an approximation: same values, same order, same sums. Verified as
bit-identical output, not argued:

| n | before sha256 | after sha256 | before (s) | after (s) |
| --- | --- | --- | --- | --- |
| 101 | `6916a9de…` | `6916a9de…` | 0.43 | 0.45 |
| 201 | `de72fbde…` | `de72fbde…` | 1.31 | 0.95 |
| 301 | `d5b57d71…` | `d5b57d71…` | 2.32 | 1.61 |
| 401 | `26ace198…` | `26ace198…` | 3.75 | 2.69 |
| 601 | `ffe50444…` | `ffe50444…` | 10.82 | 5.71 |

Same hash at every size, ~1.9x faster at the largest.

## The visibility fix

`roundtrip` buffered every finding into one `String` and printed it in a single `print!`
at the very end (`roundtrip.rs:217` before this change), so a killed run left an empty
log with no way to name the seed it died in. `sweep_with` now takes a progress sink and
writes one line per seed to standard error, **before** working that seed, so the last line
in a log is the seed a stalled run is sitting in. Standard output keeps its single block,
so a reader can still diff it. Pinned by
`roundtrip::tests::every_seed_is_reported_before_it_is_worked` and
`progress_is_one_line_per_seed_not_a_sample` (the second is the negative control: a
sampled line would pass the first and fail this one).

The row also moved to `--release` (`scripts/orch/rows/develop-full.rows:42`), and the
sweep loop was split so `sweep` keeps its test-only signature.

## Before / after

`scripts/orch/gr ./target/release/graph-cli roundtrip --seeds N`, seconds, same host. The
"before" column is the release build of the pre-fix tree; the "after" column is this tree.

| N | release before | release after | debug after | exit |
| --- | --- | --- | --- | --- |
| 8 | 0.61 | 0.76 | — | 0 |
| 32 | 2.21 | 2.62 | 30.27 | 0 |
| 128 | 42.82 | 31.05 | 333.12 | 0 |
| 256 | 292.34 | 252.41 | — | 0 |
| 1000 | (2 h, killed, debug) | **2822.51** | — | **0** |

The 1000-seed row, measured exactly as `scripts/orch/rows/develop-full.rows:42` now runs
it (`/usr/bin/time -f %e`, under `scripts/orch/timed`, no concurrent build or edit in the
tree — the first attempt at this measurement exited 2 because a `cargo fmt` mid-run moved
the tree fingerprint out from under the evidence stamp, which is the stamp working):

```
scripts/orch/timed bash -c "scripts/orch/gr cargo build -q --release -p graph-cli && \
  /usr/bin/time -f %e scripts/orch/gr ./target/release/graph-cli roundtrip --seeds 1000 \
  > target/rt1000.out 2> target/rt1000.err"
-> 2822.51 s, exit 0, PASS
```

`target/rt1000.out`:

```
roundtrip: seeds=1000 snapshots=41000 (every registered layout + contract exercise)
  binary <-> JSON byte-exact on 41000/41000 snapshots
  layout.grid on its stated conventions on 1000/1000 seeds
  layout.circular.radial on its stated conventions on 1000/1000 seeds
  layout.packing.circle on its stated conventions on 1000/1000 seeds
  layout.dag.sugiyama on its structural invariants on 1000/1000 seeds
  3D exercise snapshots (dim 1, z column) round-tripped: 333
  notes cases drawn (exercise, each needed): 0.2-labelled 133, 0.3 k=0 324, code 1 342, code 2 343, code 3 200
PASS
```

`target/rt1000.err` holds 1000 progress lines plus the `/usr/bin/time` figure, one per
seed from `seed 0/1000 nodes 2` to `seed 999/1000 nodes 401`. That is the shape the log
should have had all along: had the old code printed them, the killed 2 h run would have
said which seed it was sitting in rather than nothing at all.

2822.51 s is still 47 minutes, and it is still the slow row — `hashgate-1000` next to it
under `# slow:` is recorded at 3052 s (`docs/reports/phase-03.md:104`), so this is now the
same order as the gate's other slow arm rather than an outlier that gets killed.

N=8 and N=32 are inside process-startup noise (~0.2 s of it) and the layout work at those
sizes is sub-second, so those two rows are not a measurement of the fix. The rows that
are: 128 seeds, 42.8 s → 31.1 s, and 256 seeds, 292.3 s → 252.4 s.

Debug is 10.7x release at 128 seeds (333.12 s vs 31.05 s) — the whole reason the row was
killed. That ratio alone accounts for the missing factor of ten; the crossing-cache change
accounts for the rest.

Growth is not linear in N: with per-seed node count capped at 601 the sweep cannot keep
scaling past 600 seeds, and the 128 → 256 step (nodes 2…129 vs 2…257) is the steepest in
the table. Extrapolating the `O(n^2)`-ish layout terms over 1000 seeds lands where the
1000-seed measurement is at.

**`roundtrip --seeds 1000` (release), the row as committed: 2822.51 s, exit 0.** The full
measurement and its output are in the table above.

## Commands

```
scripts/orch/gr cargo run -q --release -p graph-cli -- roundtrip --seeds 8    # before: 0.61 s
scripts/orch/gr cargo run -q --release -p graph-cli -- roundtrip --seeds 32   # before: 2.21 s
scripts/orch/gr cargo run -q --release -p graph-cli -- roundtrip --seeds 128  # before: 42.82 s
scripts/orch/gr cargo run -q --release -p graph-cli -- roundtrip --seeds 256  # before: 292.3 s
scripts/orch/gr ./target/release/graph-cli snapshot --seed 0 --nodes 601 --layout force.davidson_harel --out-bin /w/target/dh.bin
scripts/orch/timed bash -c "scripts/orch/gr cargo build -q --release -p graph-cli && \
  /usr/bin/time -f %e scripts/orch/gr ./target/release/graph-cli roundtrip --seeds 1000 \
  > target/rt1000.out 2> target/rt1000.err"                                    # 2822.51 s, exit 0
scripts/orch/gr -e GM_MUTATE_NODE_Z=1 cargo run -q -p graph-cli -- roundtrip --seeds 8   # expect non-zero
scripts/orch/gr cargo fmt --all --check
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings
scripts/orch/gr cargo test -q -p graph-core davidson
scripts/orch/gr cargo test -q -p graph-cli --bin graph-cli roundtrip::
```

## The negative control

`GM_MUTATE_NODE_Z=1` puts one value too many in the 3D z column, so the reader must refuse
it and the row must go non-zero. It does, and the progress lines say exactly where:

```
roundtrip: seed 0/8 nodes 2
roundtrip: seed 1/8 nodes 3
roundtrip: seed 2/8 nodes 4
roundtrip: could not run: perturbed exercise seed 2: node.z: 4 values, need 3
-> exit 2
```

Seed 2 is the first 3D one (`seed % 3 == 2`), so the control bites on the first seed it
can and the log names it.

## Files

| file:line | what |
| --- | --- |
| `crates/graph-core/src/layout/force/davidson_harel/energy.rs:52` | `resting`: records the position's own crossings and reciprocal distances once |
| `crates/graph-core/src/layout/force/davidson_harel/energy.rs:108` | `node_dist` reads the record instead of recomputing the `p` half |
| `crates/graph-core/src/layout/force/davidson_harel/energy.rs:144` | `crossings` reads the recorded flags instead of recomputing them |
| `crates/graph-core/src/layout/force/davidson_harel.rs:209` | `try_node` builds one record per position, not per candidate |
| `crates/graph-cli/src/snapshot_cmd/roundtrip.rs:159` | `sweep_with`: one progress line per seed, before the seed is worked |
| `scripts/orch/rows/develop-full.rows:42` | the row builds `--release` |