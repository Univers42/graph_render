# Job perf-snapshot-build (agent build: build a 1M snapshot in O(n), same bytes)

Goal: at 1M nodes the browser spends 441 ms in `Snapshot::new` and 338 ms in `StringTable::from_strs`
for one snapshot (`origin/perf-live-cadence:docs/measurements/perf-live-cadence.md:91-102`). Make
both linear and allocation-bounded without changing one output byte.

Facts (verified on develop b2cbbcd9):
- `crates/graph-core/src/layout/mod.rs:152-176` `snapshot()`:
  - `:154-155` reads ids through `topology.node(i).id` and `topology.edge(e).id`;
  - `:162-163` passes them to `StringTable::from_strs`;
  - `:176` calls `Snapshot::new`.
- `crates/graph-core/src/index/view.rs:45-60` `node(index)` builds a whole `NodeView`: ten fields,
  five string lookups. `:63-75` `edge` does the same with nine fields, including two endpoint
  lookups. `snapshot()` keeps only `.id`.
- `crates/graph-contract/src/binary.rs`:
  - `:36` `StringTable::from_strs` starts from `Self::default()` and grows `text` and `offsets` one
    `push_str`/`push` at a time, with no reserve.
  - `:87` `first_repeat` is `let mut seen = BTreeSet::new(); self.iter().position(|s| !seen.insert(s))`.
    That is O(n log n) string compares and one tree node per id.
  - `:139` `Snapshot::new` calls `first_repeat` on `node.id` and on `edge.id`.
  - `:189-191` `to_bytes` starts from `Vec::new()`, so it grows by doubling up to the final size.
- `first_repeat` only answers whether an id repeats, and which index repeats first. The index it
  returns is the first position whose id was already seen. Iteration order is the table's own
  order, so the result does not depend on the set's internal order.
- The negative control already exists:
  `crates/graph-contract/src/binary/tests.rs:136-143`
  `construction_refuses_repeated_ids_stray_endpoints_and_short_ends` asserts
  `DuplicateId { column, index: 1 }` for both columns.
- `graph-contract` dependencies are `serde`, `schemars` and `serde_json`, all optional
  (`crates/graph-contract/Cargo.toml:16-19`). No new dependency is allowed.

Do, in order:
1. Write a test first in `crates/graph-contract/src/binary/tests.rs`: a table of 10 000 distinct
   ids followed by a repeat of id 4 321 reports `DuplicateId { index: 10_000 }`, and a table whose
   ids repeat at index 7 and again at 9 000 reports index 7. Run it on develop: it must pass there
   too (it pins behaviour, it is not a red test). Keep the existing test untouched.
2. Add `Topology::node_id(index) -> &str` and `Topology::edge_id(index) -> &str` next to `node` and
   `edge` in `index/view.rs`, one line each. Use them in `snapshot()` instead of the full views.
3. Reserve in `StringTable::from_strs`:
   - Take `items: impl IntoIterator<Item = &str>` as it does now, but collect nothing.
   - Reserve `offsets` from the iterator's `size_hint().0`.
   - For `text`, use whatever exact length is cheap: if the iterator is `Clone`, a first pass
     summing `len()` is O(n) and allocation-free. Otherwise reserve from the hint times a fixed
     guess, with a `Caveat:` line on the guess.
   - The `Capacity` refusal must still fire at the same item. Keep its check.
4. Replace the `BTreeSet` in `first_repeat` with an O(n) membership test:
   - Sort a `Vec<u32>` of indices by `(string bytes, index)`. Then, for each run of equal strings,
     the repeat is the run's second index, and the answer is the smallest such second index over
     all runs. That is O(n log n) compares but one allocation of 4 bytes per id, with no tree nodes.
   - Or use `std::collections::HashSet<&str>` with a fixed `BuildHasherDefault` hasher. D4 forbids
     `HashMap` where iteration order is observable; membership only is not iteration. If you take
     this route, put one line above it citing D4 and saying the set is never iterated.
   - Measure both on 1M ids and keep the faster. Record both numbers.
5. Presize `to_bytes`: compute the exact output length from the header, the tables and the columns
   before writing, then `Vec::with_capacity(len)`. Add a `debug_assert_eq!(out.len(), len)` at the
   end.
6. Measure `snapshot()` at 1M nodes and 1.5M edges, release build, native:
   - Use an `#[ignore]`d timing test in `crates/graph-core` (it is a bench, not a gate; say so in
     its doc line) or a `graph-cli` subcommand if one already times stages. Do not add a new
     subcommand.
   - Run before (develop) and after (branch) alternated, 3 rounds each. Report medians only, and
     print `uptime` with each round.
   - Then build `graph-wasm` and repeat once in the studio at 1M: the `force.start` and first load
     times from the worker's performance marks, if you can reach them headless. If not, write
     "browser not measured".

Out of bounds: the wire format, `SnapshotParts`, the JSON face, anything under `packages/` or `app/`.
No `unsafe`, no new dependency. House limits: 40 lines a function, 4 parameters, 300 lines a file,
nesting ≤ 3. Split `binary.rs` into a child module if it would exceed 300 lines.

Paths you may edit: `crates/graph-contract/src/binary.rs` and its child modules,
`crates/graph-contract/src/binary/tests.rs`, `crates/graph-core/src/index/view.rs`,
`crates/graph-core/src/layout/mod.rs`, one timing test file under `crates/graph-core/tests/`,
`docs/measurements/perf-snapshot-build.md`.

Done when:
- `scripts/orch/gr cargo fmt --all --check` exits 0.
- `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- `scripts/orch/gr cargo test --workspace --no-fail-fast` exits 0, including the step-1 test and the
  existing duplicate-id test.
- `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` exits 0.
- `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` exits 0, and the same run with
  `-e GM_MUTATE_REFERENCE_DEGREE=9` exits non-zero.
- `scripts/orch/gr cargo run -q -p graph-cli -- roundtrip --seeds 100` exits 0.
- Negative control for step 4: temporarily make `first_repeat` return `None`; the duplicate-id test
  must fail. Revert, and paste both outputs into the measurement doc.
- `docs/measurements/perf-snapshot-build.md` holds the before/after medians with load, both step-4
  candidates' numbers, and what the change does not do (the canonical JSON face is not touched).
