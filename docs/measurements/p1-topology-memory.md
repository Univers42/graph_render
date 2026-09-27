# Phase 1 — topology memory, and the `topology.index` scale ceiling

Measured on the Phase 1 tree in `ge-rust` (release build). A counting global allocator
wraps `index_model` on `synthetic_records(n)`, after `apply_degree_weights`:

- **held** is the net heap that `index_model` owns when it returns;
- **peak** is the highest net heap reached during the call.

The inputs are allocated before the counter starts, so neither number includes them.
The harness is a throwaway binary outside the repo. Its source is kept with the phase
evidence (`memprobe/src/main.rs`).

| n | m | arena bytes | arena strings | node columns | edge columns | 3 CSRs | held | peak | held / node | arena / node |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 000 | 1 541 | 39 320 | 3 572 | 50 000 | 46 230 | 24 340 | 395 770 | 395 870 | 395.8 B | 39.3 B |
| 10 000 | 15 474 | 427 990 | 35 505 | 500 000 | 464 220 | 243 804 | 5 222 872 | 5 222 972 | 522.3 B | 42.8 B |
| 100 000 | 154 978 | 4 636 520 | 355 009 | 5 000 000 | 4 649 340 | 2 439 836 | 44 202 584 | 44 202 684 | 442.0 B | 46.4 B |

## Two numbers, never one (§5.1)

At 100 000 nodes:

| Part | Size | Per node |
|---|---|---|
| String arena (data-dependent) | **4.6 MB** | 46.4 B |
| Everything else (columns, CSRs, indices, arena spans and lookup) | **39.6 MB** | 395.6 B |

The "everything else" half breaks down as follows:

- node columns: 5.0 MB;
- edge columns: 4.6 MB;
- 3 CSRs: 2.4 MB;
- the rest: 27.5 MB. This is the insertion-ordered id sets for nodes and edges, the
  arena's spans and lookup table, and `by_database`, all including growth slack.

This is more than the 33 B/node that §5.1 budgets. Two reasons:

- `weight` and `version` are `f64` here (the oracle's `number`), not `f32`;
- the hash indices cost more than the columns do.

Phase 9 owns the budget, and these numbers are its baseline.

The 10 000 row is higher per node than the 100 000 row. The measurement catches the
hash tables and vectors just after a capacity doubling.

## The ceiling

| Constraint | Binds at | Derivation |
|---|---|---|
| wasm32 linear memory, 4 GiB | **~9.7 M nodes** | 4 294 967 296 B / 442 B per node |
| String arena, `u32` offsets (2^32 − 1 bytes) | ~92 M nodes | 4 294 967 295 B / 46.4 B per node |
| Dense `u32` index space | 4.29 G nodes | `u32::MAX` |

`scale_ceiling` for `topology.index` is **9 700 000**: the first constraint to bind,
rounded down to two figures.

The ceiling depends on the data. The arena half grows with id and label length. Real ids
such as `postgresql:<uuid>:<uuid>` are several times longer than the synthetic
`bench:db-0:12`, and a longer id lowers the ceiling.

Past the ceiling the two targets fail differently:

- **wasm32:** the allocation fails and the module traps. That is an abort with no
  partial result.
- **Native:** the host's memory usually runs out first (about 41 GB at 92 M nodes). If
  it does not, the arena reaches 2^32 − 1 bytes and `index_model` returns
  `CapacityError`. That is a refusal. It never wraps or truncates.
