# fix-ingest-scale: the 1M-node document builds on wasm32

Job brief: `prompts/jobs/fix-ingest-scale.md` over `prompts/jobs/fix-common.md`. Source:
`docs/decisions/wasm-ingest-limits.md` F-16 step 5, which fired on the 1M-node degree-4 and
degree-5 documents and classified the trap as a scale defect rather than a ceiling.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| F-16-s | BLOCKER | fixed: `read_records` validates the document with a walk that builds no `Value` tree and reads each record out of the element's own text, so the 1M-node documents at degrees 3, 4 and 5 build on the artifact and the ceiling could be raised to `2^30` | `the_reader_agrees_with_the_frozen_one_on_every_accepted_fixture`; `the_reader_agrees_with_the_frozen_one_on_seeded_mutations`; the eight in `ingest/tests/scan.rs`; `the_ceiling_is_the_largest_power_of_two_at_or_below_the_largest_that_built` | `crates/graph-wasm/src/ingest/scan.rs:163`, `ingest/element.rs:73`, `ingest.rs:67` |
| F-16-s | MAJOR | fixed: the trap was not in the arena at all — it was the `canonical_json::Value` tree, which measured 3.2x the document's text at 1M nodes. `index_model` was the phase the trap was *attributed* to because it is where the tree's memory was still resident | `the_counted_length_is_the_length_the_records_come_out_at` | `crates/graph-wasm/src/ingest/scan.rs:24` |
| F-16-s | MINOR | fixed: two refusal divergences the frozen-reader corpus could not reach, both found by the new `ingest/tests/scan.rs` and both fixed — a non-object root said `missing member \`version\`` instead of `expected an object`, and a member that may be `null` said `expected a string` instead of `expected a string or null` | `a_root_that_is_not_an_object_is_refused_after_the_syntax_has_been_read`; `an_array_where_a_scalar_member_belongs_is_refused_as_that_members_type` | `crates/graph-wasm/src/ingest.rs:170`, `ingest/element.rs:186` |

## Step 1 — the per-phase table, before

`harness/ingest-ceiling.mjs --phases`, on a `--features probe` artifact (the marks and a
lifted ceiling; see "Decisions taken"), reading `memory.buffer.byteLength` at each phase. The
marks live in a `static` at a fixed linear-memory address and are read out of
`memory.buffer`, **not** by calling the module — a document that traps leaves its marks
behind, and an instance that hit `unreachable` cannot be called again.

`copy` is `gm_build`'s entry, with the caller's buffer resident. `parse` is the whole
document validated. `records` is every `NodeRecord`/`EdgeRecord` built. `index_model` is
`index_model` returned — arena, columns and the three CSRs all live. `arena_text`,
`columns` and `csrs` are that topology's own component byte counts, recorded at the same
point. `returned` is `gm_build` about to return.

| 1M nodes, degree | document bytes | copy | parse | records | index_model | returned | outcome |
|---|---:|---:|---:|---:|---:|---:|---|
| 3 | 678,016,813 | 746,455,040 | 2,898,001,920 | 3,210,018,816 | 3,763,732,480 | 3,763,732,480 | **built** |
| 4 | 842,132,644 | 910,557,184 | 3,663,986,688 | — | — | — | **trapped** between `parse` and `records` |
| 5 | 992,910,579 | 1,061,355,520 | — | — | — | — | **trapped** inside `parse` |

Degree 3's components at `index_model`: `arena_text` 50,241,696, `columns` 142,999,721,
`csrs` 37,198,348 — 230,439,765 of the 553,713,664 the phase added over `records`, the rest
being the arena's span table and lookup hash table plus the two id `IndexSet`s.

**The phase that holds the bytes is the parse.** At degree 3 the generic tree costs
2,151,546,880 bytes over a 678,016,813-byte document: **3.17x the input**, one heap block per
object member and one per string and per number. Degree 4 traps while the records are being
read *alongside* the tree, and degree 5 traps before `parse` even returns. `index_model`'s
own share — the arena, the columns and the CSRs — is 0.55 GB at degree 3 and would have fit
several times over.

## Step 2 — the rung, and why it is that one

The job's rungs are pre-size, free-what-is-dead, borrow-instead-of-copy, parse-into-typed-
records. The first three cannot touch this phase: the tree *is* the allocation, so there is
nothing to pre-size or free early, and borrowing ids would shrink the record copy (312 MB at
degree 3) rather than the 2.15 GB tree. So the fourth rung is the one that applies, and it is
the one taken:

- `crates/graph-wasm/src/ingest/scan.rs` (+ `scan/walk.rs`, `scan/text.rs`) — a validating
  walk of the document that keeps only the root's members and each array's element *count*.
  It is a line-for-line mirror of `graph_contract::canonical_json::parse`: same acceptance,
  same `JsonError::Syntax` offsets, same messages, same order (a member's value read before
  its key is checked for repetition; a repeated key reported at the second key's quote).
- `crates/graph-wasm/src/ingest/element.rs` — one record's members as spans in a fixed
  `[Option<Span>; 10]` on the stack, read in the shape's field order afterwards.
- `read_records` is two walks over the text and then one per array, with no per-element or
  per-member allocation anywhere: both record `Vec`s are sized from the count the first walk
  took, so neither grows by doubling.

The two walks are split on purpose. The reader this replaces parses the *entire* text before
it looks at the root, so a syntax fault in `edges[4000000]` outranks a missing `version`. One
walk that located as it validated would report them the other way round — which the
differential test would rightly call a change of refusal order.

## Step 3 — the per-phase table, after

| 1M nodes, degree | document bytes | copy | parse | records | index_model | returned | outcome |
|---|---:|---:|---:|---:|---:|---:|---|
| 3 | 678,016,813 | 746,455,040 | 746,455,040 | 1,327,366,144 | 1,990,328,320 | 1,990,328,320 | **built** |
| 4 | 842,132,644 | 910,557,184 | 910,557,184 | 1,630,732,288 | 2,331,770,880 | 2,331,770,880 | **built** |
| 5 | 992,910,579 | 1,061,355,520 | 1,061,355,520 | 1,920,794,624 | 2,611,675,136 | 2,611,675,136 | **built** |

Degree 3's components: `arena_text` 50,241,696, `columns` 142,999,721, `csrs` 37,198,348 —
byte-identical to before, which is the point: the topology is the same graph, held in less
memory.

| 1M nodes, degree | peak before | peak after | cut |
|---|---:|---:|---:|
| 3 | 3,763,732,480 | 1,990,328,320 | **−47.1%** |
| 4 | trapped at 3,663,986,688 | 2,331,770,880 | — |
| 5 | trapped inside `parse` | 2,611,675,136 | — |

`parse` now costs **zero** bytes over the copy mark: the whole document is validated twice
and neither walk allocates past the root's three members.

## Step 4 — the sweep upward, and the new ceiling

The studio's generator clamps `nodeCount` to `MAX_NODES = 1_000_000`
(`packages/graph-studio/src/source/synthetic.ts:28`), so degree is the only lever upward from
the scale target. Sweeping it on the probe artifact until a document traps:

| 1M nodes, degree | document bytes | edges | `index_model` mark | outcome |
|---:|---:|---:|---:|---|
| 5 | 992,910,579 | 4,999,975 | 2,611,675,136 | built |
| 6 | 1,170,734,439 | 5,999,964 | 3,198,812,160 | built |
| 7 | 1,335,097,629 | 6,999,951 | 3,643,801,600 | built |
| **8** | **1,499,403,588** | **7,999,936** | **4,117,561,344** | **built — the largest that built** |
| 9 | 1,663,576,802 | 8,999,919 | — | **trapped** while the records were being read |

`2^30` = 1,073,741,824 is the largest power of two at or below 1,499,403,588, so
**`MAX_INGEST_BYTES = 1_073_741_824`**. It refuses nothing that built before this job: the
previous ceiling was 774,568,785 (itself the largest that built then), and the studio's own
1M-node degree-3 document is 678,016,813 bytes. Re-run under the new number, all of it:

| 1M nodes, degree | document bytes | outcome on the default artifact | exit |
|---:|---:|---|---:|
| 3 | 678,016,813 | `1000000 2999991 3 678016813 built code=0` | 0 |
| 4 | 842,132,644 | `1000000 3999984 4 842132644 built code=0` | 0 |
| 5 | 992,910,579 | `1000000 4999975 5 992910579 built code=0` | 0 |

and the boundary, on the default artifact:

| document bytes | outcome | exit |
|---:|---|---:|
| 1,073,741,824 (exactly the ceiling) | `1000000 4999975 5 1073741824 built code=0` | 0 |
| 1,073,741,825 (one past) | `1000000 4999975 5 1073741825 refused code=19` | 1 |

Documents `fix-wasm-ingest` recorded as trapping, re-run under the new ceiling: 950,000 nodes
at degree 4 (799,922,860 B) → `built code=0`; 1,000,000 at degree 4 (842,132,644 B) → `built
code=0`; and the old ceiling's own document, 920,000 at degree 4 (774,568,785 B) → `built
code=0`.

### What the rounding costs

The ceiling refuses the 425,661,764 bytes between `2^30` and 1,499,403,588 — documents this
build accepts. That is the trade `docs/decisions/wasm-ingest-limits.md` step 3 asks for now
that the arena no longer traps, and it is stated in the constant's `Ponytail:`, in
`docs/contract/wasm-abi.md` and in the decision record rather than papered over. The ceiling
still bounds bytes and not the work they imply: the degree-9 document at 1,663,576,802 bytes
is 589,834,978 *above* the ceiling and traps, while the largest that built is 425,661,764
*below* it — the two bands do not meet.

## Commands

| command | exit | last lines |
|---|---|---|
| `scripts/orch/gr cargo test -p graph-wasm --lib -- ingest::differential` (RED, four rounds) | 101 → 0 | `2 passed; 1 failed` → `3 passed; 0 failed`. The four divergences it caught, in order: `Shape("nodes[0].source: expected a string")` vs `Shape("nodes[0]: unknown member \`sourc9\`")` (edge field offsets); `Syntax { at: 263, what: "an unknown escape" }` vs `at: 265` (the escape offset); `Syntax { at: 394, what: "a fraction needs digits" }` vs `at: 396` (the number's fault offsets); `unknown member \`str\n\`` vs `unknown member \`str\ngth\`` (the escaped run's tail was dropped) |
| `scripts/orch/gr cargo test -p graph-wasm --lib -- ingest::tests::scan` (RED, two rounds) | 101 → 0 | `: missing member \`version\`` vs `: expected an object`; `nodes[0].icon: expected a string` vs `expected a string or null` |
| `scripts/orch/gr cargo test -p graph-wasm --lib` | **0** | `160 passed; 0 failed; 1 ignored` |
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | **0** | `Finished dev profile` |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | **0** | 1795 passed, 0 failed, 12 ignored across 21 suites |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | `Finished dev profile` |
| `scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown` | 0 | `Finished release profile`, no warning from any code |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | **0** | `4-way equal on 8/8 seeds` / `PASS` |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | **1** (control, as required) | `4-way equal on 0/8 seeds` / `FAIL: 8 of 8 seeds diverge` |
| `scripts/scigraphs-conformance.sh` | **0** | `scigraphs-conformance: 32/32 rows reached a reference` / `PASS` |
| the sweep, degrees 5-9 at 1M nodes, probe artifact | 0,0,0,0,3 | `built code=0` ×4, then `trapped code=0` |
| degrees 3/4/5 at 1M nodes, default artifact | 0,0,0 | `built code=0` ×3 |
| the boundary at `2^30`, default artifact | 0,1 | `1073741824 built code=0` / `1073741825 refused code=19` |

## Decisions taken

1. **The probe artifact lifts the byte ceiling.** `--features probe` builds
  `read_records`'s length check as `usize::MAX`, so the sweep can watch a document the
  ceiling would *refuse* trap instead — the phases that trap are exactly the ones no
  refusing run reaches. Only the length check moves; every byte after it and every refusal
  below it is the same code. The build/refuse rows above all come from the **default**
  artifact; only the mark tables come from the probe one. `Ponytail:`-marked on `ceiling()`.
2. **The marks are a `static` read through `memory.buffer`, not a returned `u32` per phase.**
   A returned value is lost exactly where the measurement needs it: the documents that trap.
   `Table::base()` is a `usize`, not a `u32`, because this module is also unit-tested natively
   where a position-independent binary's static lives above 4 GiB; only `gm_probe_base`
   narrows it, and there `usize` *is* `u32`.
3. **The probe's unit tests each own a leaked `Table`.** A `Table`'s whole point is that its
   address does not move, and a local one moves when it is returned — so the tests leak 256
   bytes each rather than have the bug they exist to catch. They also cannot share the static:
   every `read_records` in the binary writes it.
4. **`element.rs` replaces `record.rs` rather than sitting beside it.** The old module read a
   `canonical_json::Value`; with no tree there is nothing for it to read. Its
   `NODE_FIELDS`/`EDGE_FIELDS` moved into the new module, which is where they are used.
5. **The field offsets are named per shape.** `kind` is index 1 in a node and 3 in an edge,
   and the differential test's first failure was a reader that assumed otherwise — so
   `field::NODE_KIND` and `field::EDGE_KIND` are separate constants, and a comment says why.
6. **`Scan::array`'s depth for a root member's value is a named constant, not a stored
   field.** The only arrays the walk re-walks are root members', whose values are at depth 1
   by construction (the root object is depth 0).

## Deviations

- `crates/graph-wasm/src/ingest/record.rs` is **deleted**, and
  `crates/graph-wasm/src/ingest/{scan.rs,scan/walk.rs,scan/text.rs,element.rs,phases.rs,tests/scan.rs}`
  are **new**. All are inside this job's `ingest/**` path.
- `docs/measurements/fix-wasm-ingest.md` is **not** edited: it is the record of the sweep
  that set the previous number, and that number is what this job's ceiling had to clear.
  Overwriting it would erase the measurement the new one is measured against.

## Findings not fixed here

- **`memory_measure.rs`'s counting allocator still measures the tree it no longer builds.**
  `provisional_ingest_pipeline_memory_per_node` is `#[ignore]`d and its numbers are now the
  post-fix ones; nothing reads them in this tree. It is outside this job's paths.
- **`crates/graph-wasm/src/ingest/scan.rs`'s duplicate-key check keeps the reference's
  quadratic-over-a-wide-object caveat verbatim**, including the `WIDE_OBJECT` threshold. The
  decision is identical either way (a repeated key is refused at the same offset), so the
  threshold is a performance choice inherited rather than re-measured here.
- **`docs/measurements/p1-topology-memory.md` and the `transport.wasm.columnar` ledger row's
  `scale_ceiling` figure** were measured with the tree-building reader and are now pessimistic
  by ~2.15 GB per 1M nodes. Both are outside this job's paths; the row should be re-measured
  with `memory_measure`'s test now that it is no longer measuring the same pipeline.
