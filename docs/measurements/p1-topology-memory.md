# Phase 1 — topology memory, and the `topology.index` scale ceiling

Measured on the Phase 1 tree in `ge-rust` (release build, x86_64). The probe is
committed as `crates/graph-core/tests/memory.rs`, so anyone can re-run it:

```sh
docker run --rm -v "$PWD:/w" ge-rust \
  cargo test --release -p graph-core --test memory -- --ignored --nocapture
```

A counting global allocator wraps `build_synthetic_model(n)`:

- **held** is the net heap that the returned topology owns. The input records are freed
  before the call returns, so they are not in it;
- **peak** is the highest net heap reached during the call, input records included.

| n | m | arena bytes | arena strings | node columns | edge columns | 3 CSRs | held | peak | held / node | arena / node |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 000 | 1 541 | 39 320 | 3 572 | 50 000 | 46 230 | 24 340 | 395 770 | 947 787 | 395.8 B | 39.3 B |
| 10 000 | 15 474 | 427 990 | 35 505 | 500 000 | 464 220 | 243 804 | 5 222 872 | 10 901 147 | 522.3 B | 42.8 B |
| 100 000 | 154 978 | 4 636 520 | 355 009 | 5 000 000 | 4 649 340 | 2 439 836 | 44 202 584 | 101 430 310 | 442.0 B | 46.4 B |

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

## The ceiling — an estimate

| Constraint | Binds at | Derivation |
|---|---|---|
| wasm32 linear memory, 4 GiB | **~9.7 M nodes** | 4 294 967 296 B / 442 B per node |
| String arena, `u32` offsets (2^32 − 1 bytes) | ~92 M nodes | 4 294 967 295 B / 46.4 B per node |
| Dense `u32` index space | 4.29 G nodes | `u32::MAX` |

`scale_ceiling` for `topology.index` is **9 700 000**: the first constraint to bind,
rounded down to two figures.

This is an **estimate**, not a measurement on the target, and the ledger row carries a
Ponytail marker saying so. What it misses:

- **The target.** The 442 B/node was measured on a 64-bit host and projected onto
  wasm32's 4 GiB. On wasm32, pointers and `usize` are 4 bytes, so the vector headers and
  the index tables are smaller and the real per-node cost is probably lower.
- **The data.** The arena half grows with id and label length. Real ids such as
  `postgresql:<uuid>:<uuid>` are several times longer than the synthetic
  `bench:db-0:12`, and a longer id lowers the ceiling.
- **The input.** The caller's own records are not counted. During the build they more
  than double the peak (101 MB against 44 MB held, at 100 000 nodes).

Past the ceiling the two targets fail differently:

- **wasm32:** the allocation fails and the module traps. That is an abort with no
  partial result.
- **Native:** the host's memory usually runs out first (about 41 GB at 92 M nodes). If
  it does not, the arena reaches 2^32 − 1 bytes and `index_model` returns
  `CapacityError`. That is a refusal. It never wraps or truncates.
