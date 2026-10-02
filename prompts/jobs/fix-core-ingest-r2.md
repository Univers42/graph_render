# Job fix-core-ingest, round 2 (agent build: the review's FIX items)

Read `prompts/jobs/fix-common.md` and `prompts/jobs/fix-core-ingest.md` first; same paths, same
done-when. The branch already carries round 1 (`docs/measurements/fix-core-ingest.md`). An independent
review returned FIX. Each item below is RED test first, then GREEN, then its report row updated.

1. **F-15, MAJOR.** `crates/graph-core/src/ingest/roles.rs:68-71` falls back to `group` per record
   (`role_value(..Label).or_else(|| role_value(..Group))`, and `role_value` is None for a missing
   cell), so in a collection declaring both `label` and `group`, a record with no label cell gets its
   group. The doc at :63-67 says label wins where both are declared. Decide the fallback on whether
   the collection declares a `label` role, not on the record's cell. Restore
   `an_absent_value_is_none_and_an_empty_one_is_some_empty` (`tests/roles.rs:165-183`) to
   `one_of_each()`; it must pass unchanged. Correct the report's "every existing test intact" line.
2. **F-107, MAJOR.** The deferral is wrong: `build_topology` already returns
   `Result<(Derived, Topology), BuildError>` (`build.rs:171`), so `BuildError::Capacity` is additive.
   Add it, map `index_model`'s error to it and remove the `.expect("a derived graph indexes")`
   (`build.rs:176`). graph-wasm uses only `Display`/`map_err`; check that it still builds. Flip the
   report row to fixed and cite :176, not :180.
3. **F-103, MINOR.** `build/builder.rs:133-157` dedups within one record (`emitted` is per record).
   A two-way link (A→B in A's record, B→A in B's record) emits two `Derived` edges with one id
   (`make_edge_id` sorts undirected endpoints), and `index_model` drops the second silently. RED: a
   test with that two-way link expecting one edge. GREEN: dedup by edge id across records.
4. **F-108, MINOR.** `strength.rs` keeps each value twice (`STRENGTH_TABLE` :20-26 and the `match`
   :41-53). Keep one source; fix the stale docs at :8 and :28.
5. **Test file.** `tests/fixture_write.rs:45-60`: delete the doubled doc block on `replace_member`;
   its `find("\"graph\":")` matches the first occurrence anywhere, so add a `Ponytail:` line naming
   that failing input.
6. **EXAMPLES.md.** `crates/graph-sdk-js/EXAMPLES.md:140` still shows `relation:Blocks` /
   `label="Blocks"`; `fixtures/ingest/expected-graph.json:215,217` now say `blocks`. Re-copy the line
   from the passing derivation (an added path for this item only).
7. **Report handoff** (no code): `graph-contract`'s `first_with_role` (`collection.rs:37-38`)
   contradicts its "lowest-id" doc and now has no callers; name it under "handed off" to fix-contract.
