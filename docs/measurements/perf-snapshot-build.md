# `perf-snapshot-build` — building a 1M-node snapshot in O(n), same bytes

Measured 2026-10-03 on branch `perf-snapshot-build` (from `501b6b08`, develop `b2cbbcd9`). Host:
dlesieur42, i5-13600KF, 20 cores, 31 GB. Native, `--release`, x86_64. The probe is committed as
`crates/graph-core/tests/snapshot_build.rs`, so anyone can re-run it:

```sh
scripts/orch/gr cargo test --release -p graph-core --test snapshot_build -- --ignored --nocapture
```

**Every test in that file is `#[ignore]`d and asserts no number.** It is a measurement, not a gate
row: a busier host changes its output and cannot turn the suite red.

Why: the browser profile in `docs/measurements/perf-live-cadence.md` (`origin/perf-live-cadence`,
table row "`Snapshot::new` + `StringTable::from_strs` (once, at `force.start`)") charged **441 ms +
338 ms** to the two of them for one 1M-node snapshot. Together 5.1% of that profile, and the only
row on the list that is not inside the motor's tick.

## What changed

| Piece | Where | What changed |
|---|---:|---|
| the id a snapshot keeps | `graph-core/src/index/view.rs:62-73` | `Topology::node_id` / `edge_id`: one arena lookup each, beside `node`/`edge` |
| the caller | `graph-core/src/layout/mod.rs:154-157` | `snapshot()` asks for the id, not the ten-field view it discarded |
| both buffers reserved | `graph-contract/src/binary.rs:57-70` | `from_strs` reserves `offsets` and `text` from the iterator's `size_hint` before writing anything |
| the duplicate-id check | `graph-contract/src/binary.rs:118-124` | a `HashSet` membership test instead of a `BTreeSet` |
| the output buffer | `graph-contract/src/binary/write.rs` | `byte_len` counts the exact length first, then `Vec::with_capacity` |

`binary.rs` was 264 lines and would have passed 300 with all of that in it, so the writers moved
into the child module `binary/write.rs` — which is also where the length count lives, next to the
writes it has to agree with. `debug_assert_eq!` at the end of `write::bytes` is the check on that
agreement.

## Native: `layout::snapshot` and `to_bytes`, 1M nodes / 1.55M edges

Arms alternated, 3 rounds each, `uptime` printed with every round; medians of 3 inner repetitions
inside each run.

| round | load before/after | `snapshot` before | `snapshot` after | `to_bytes` before | `to_bytes` after | bytes |
|---|---|---:|---:|---:|---:|---:|
| 1 | 15.6 / 15.6 | 559.6 ms | 70.8 ms | 27.8 ms | 23.1 ms | 69 625 908 |
| 2 | 16.0 / 12.0 | 551.6 ms | 71.5 ms | 27.4 ms | 23.8 ms | 69 625 908 |
| 3 | 12.1 / 11.5 | 558.8 ms | 71.8 ms | 27.2 ms | 23.9 ms | 69 625 908 |
| **median** | | **558.8 ms** | **71.5 ms** | **27.4 ms** | **23.8 ms** | **69 625 908** |

`snapshot` **7.8× faster (−87%)**, `to_bytes` −13%. The byte count is identical in all six rows,
and `binary::tests::pinned::the_layout_is_pinned_byte_for_byte_for_a_tiny_snapshot` plus
`every_kind_round_trips_to_the_same_bytes_and_every_column_is_word_aligned` (both in
`crates/graph-contract/src/binary/tests.rs`) pass unchanged: the bytes are the same bytes.

`to_bytes` gains much less than expected, and the reason is in the number: at 69.6 MB it was
already moving bytes at about 2.5 GB/s, so it was never allocation-bound — the doubling growth cost
roughly the 4 ms of `memcpy` it now skips. The count is what makes it *bounded* rather than faster.

## Step 4: the two `first_repeat` candidates, measured on 1M ids

Both were run in the same process, alternating, 3 rounds, on one table of 1 000 000 distinct
15.9-byte ids (15 888 890 bytes of text), release build, load 12.5–12.6. The sort candidate sorts a
`Vec<u32>` of indices by `(string bytes, index)`, takes each run of equal strings' second index as a
candidate, and answers the smallest of those.

| candidate | 1 | 2 | 3 | median | what it is |
|---|---:|---:|---:|---:|---|
| `BTreeSet<&str>` (develop) | 271.7 | 394.2 | 336.9 | **336.9 ms** | one tree node per id, O(n log n) string compares |
| sort a `Vec<u32>` of indices | 244.1 | 300.7 | 295.0 | **295.0 ms** | 4 bytes per id, O(n log n) string compares |
| `HashSet<&str, BuildHasherDefault<DefaultHasher>>` | 64.6 | 81.0 | 80.1 | **80.1 ms** | membership only |

The sort candidate was the one the brief expected to win and it did not: sorting `u32` keys whose
comparison has to chase into `offsets` and then into `text` is cache-hostile, and it pays the same
`n log n` compares as the tree without the tree's sequential inserts. The hash set wins by 4× on the
same comparison count and by dropping the per-id allocation.

D4 is cited on the set (`binary.rs:118-120`): it answers membership only and is **never iterated**,
so its internal order cannot reach the output — the answer is a position in the table's own order,
read from `iter()`. `BuildHasherDefault<DefaultHasher>` is the fixed, unseeded SipHash, so it is
identical on every run and every host (no randomness, D-rules) and is still keyed enough that a
crafted payload cannot pick its own collisions — which matters because the decoder runs this same
check on bytes it did not produce.

The sort candidate was measured and dropped; it is not in the tree.

## Negative control (step 4)

`first_repeat` temporarily made to `return None`:

```
---- binary::tests::repeat::a_repeated_id_is_refused_at_the_index_of_the_repeat stdout ----
thread '...' panicked at crates/graph-contract/src/binary/tests/repeat.rs:20:5:
assertion `left == right` failed: the repeat's own index, not 4 321
  left: None
 right: Some(10000)

---- binary::tests::repeat::of_two_repeats_the_earliest_position_is_the_answer stdout ----
thread '...' panicked at crates/graph-contract/src/binary/tests/repeat.rs:45:5:
assertion `left == right` failed: 7 repeats 3, 9 000 repeats 2, and 7 comes first in the table's own order
  left: None
 right: Some(7)

failures:
    binary::tests::a_string_table_is_csr_shaped_and_keeps_empty_strings
    binary::tests::construction_refuses_repeated_ids_stray_endpoints_and_short_ends
    binary::tests::repeat::a_repeated_id_is_refused_at_the_index_of_the_repeat
    binary::tests::repeat::of_two_repeats_the_earliest_position_is_the_answer

test result: FAILED. 20 passed; 4 failed; 0 ignored; 0 measured; 101 filtered out
```

Reverted; the same command then reports `24 passed; 0 failed`. The pre-existing
`construction_refuses_repeated_ids_stray_endpoints_and_short_ends` is the control that was already
there, and it goes red too.

## Browser

**browser not measured.** `app/dist` does not exist in this worktree, so
`scripts/studio-probe.sh` refuses before it reaches a browser (`scripts/studio-probe.sh:35-38`).
The probe that would carry the figure, `deploy/perf/settle.py`, does not report it either: it prints
`open`, `settle` and console error counts (`deploy/perf/settle.py:83,86`), and `fullMs`
(`deploy/perf/probes/settle.js:29`) starts at the probe's first poll *after* the open returned. And
`force.start` is a worker request message, not a mark
(`packages/graph-studio/src/motor/protocol.ts:66`, handled at `motor/liveLoop.ts:185`) — there is no
`performance.mark` anywhere in `packages/` or `app/` to read. Building the studio and inventing a
mark is out of this job's paths, so the browser arm is absent rather than approximated.

## What this does not do

- **It does not touch the canonical JSON face.** `canonical_json` is unchanged: no parser, no
  writer, no shape. The 1331 graph-contract tests, including
  `canonical_json::tests::binary_to_json_to_binary_is_byte_exact_for_every_kind`, pass untouched.
- **It does not change one output byte.** The wire format, `SnapshotParts`, the header and the
  decoder are all as they were; `from_strs` reads the same iterator once and produces the same CSR
  shape, and `first_repeat` returns the same index.
- **It does not make `snapshot` O(n).** It removes a log factor and the doubling, from 558.8 ms to
  71.5 ms. What is left is 1M id reads through the arena plus one hash-set pass, which is still
  linear and still ~70 ms at this size.
- **It does not make `from_strs`'s text reservation exact.** `offsets` is exact
  (`try_reserve_exact(count + 1)`); `text` is `ID_BYTES` (16) per item, a guess, because an exact
  answer needs the ids read twice and the iterator is not `Clone`. Caveat: ids longer than 16 bytes
  cost one reallocation; ids shorter leave up to 16 bytes per id reserved and never filled — 16 MB at
  a million six-byte ids, against 6 MB of text. It sizes the first allocation only; `text` still
  grows to whatever the ids are, and `try_reserve` reports a host that cannot give it even this much
  as `SnapshotError::Capacity`, the same refusal the `u32` overflow gives, at the same item.
- `Caveat:` the host was loaded for the whole session (load average 10.9–16.8 on 20 cores, other
  jobs alongside). The arms alternated so both saw the same load, and the three rounds of each arm
  spread by under 2%, but only the medians are claimed.
- `Caveat:` a first alternation run was thrown away. Two runs of the driver overlapped and overwrote
  each other's source tree mid-run, which put one arm's binary in the other arm's round (before:
  391.3–398.7 ms, where the clean arm reads 551.6–612.9 ms). The table above is from one clean
  re-run with nothing else in flight.
- **It does not fix `roundtrip --seeds 100`.** That command exits 1 on this branch, on
  `layout.dag.sugiyama`'s structural invariants — seeds 66, 75, 77, 78, 96, 97, all "reversed=true"
  edge notes. It fails identically with this job's three files reverted to develop's, so it is
  pre-existing and belongs to the DAG layout, not here. Recorded rather than hidden.