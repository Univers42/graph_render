# Handoff — state of graph-motor at the end of this session

Read this first. It lists what is done, what is half-done and what is left, in the order to
do it. The standing rules are in `CLAUDE.md`, `prompt.md`, `prompts/ONBOARDING.md` and
the `.claude` house rules (read-only rules repo `univers42/claude-deal-with-the-devil`).

## Toolchain (how every result below was produced)

- Everything runs in Docker. The `ge-rust` image is built from this repo's `Dockerfile`
  (Rust 1.98, edition 2024, `wasm32-unknown-unknown`, node). Prebuilt vendor language
  images (`FROM rust:*`, playwright, …) are not allowed.
- Command form: `docker run --rm -v "$PWD:/w" -w /w ge-rust cargo …`. In this session the
  runs also used `--network host` and a cargo registry volume. Behind a TLS proxy, mount
  the host CA bundle and pass its proxy variables.
- House limits: ≤ 40 lines per function, ≤ 4 parameters, ≤ 300 lines per file.
  `rules/ponytail.md` governs the `Ponytail:` markers.
- Git (user rules): author `LESdylan <dev.pro.photo@gmail.com>`, message exactly
  `updated`, push straight to `develop`, no PR, no co-author trailer.

## Done

- **Phase 0**: foundation, contract header, 4-way hashgate, ledger, D1 probe, guard.
  Report: `docs/reports/phase-00.md`.
- **Phase 1**: the topology layer and the 17 oracle functions. `oracle:diff` is green.
  Report: `docs/reports/phase-01.md`.
- **Phase 2**: all the code is in and tested. The gate has not been run, and its report
  has not been written.
  - `graph-contract`
    - `version.rs`: `FormatVersion`, current **0.2**. Anything with a newer major is
      refused.
    - `geometry.rs`: `NodeGeometry` (Point, Circle, Box) and `EdgeGeometry` (Line,
      Polyline, Curve) over `Paths` (CSR offsets plus interleaved pts).
    - `binary.rs` and `binary/decode.rs`: the word-aligned binary face (`Snapshot`,
      `StringTable`). The decoder refuses every malformed column at its exact offset.
    - `canonical_json.rs` with `parse.rs` and `read.rs`: a strict RFC 8259 parser and
      the canonical writer (sorted keys, shortest f32).
    - `schema.rs`: the serde mirror used for the generated
      `docs/contract/snapshot-schema.json`.
  - `graph-core`
    - `stage.rs`: the `Stage` trait, `run_pipeline` and `run_with`, `PipelineRun`, with
      each stage hashed separately.
    - `stage/topology.rs`: `seeded_model` and `gate_node_count`.
    - `layout/grid.rs`: the grid. Conventions: cells are `spacing` apart, the lattice is
      centred on the origin, `cols = ceil(sqrt n)`. It carries the Ponytail for the
      ragged last row.
    - `registry.rs`: `LAYOUTS` and `find`, with non-optional `Metadata`.
      `scale_ceiling` is 4.6 M nodes, measured by `tests/memory.rs`
      `grid_pipeline_memory_per_node` at 919 B per node (an estimate).
  - `graph-wasm`: the exports are now `gm_topology` and `gm_layout_grid`.
    `gm_synthetic` was removed. `harness/wasm-run.mjs` was updated to match.
  - `graph-cli`
    - `hashgate`: its stages are `topology` and `layout.grid`. There are two negative
      controls, `GM_MUTATE_REFERENCE_DEGREE` (red on topology) and
      `GM_MUTATE_GRID_SPACING` (red on layout.grid). Each writes its own record,
      `hashgate-control-<knob>.json`, and only one may be set at a time.
    - `snapshot --seed S [--nodes N] --layout grid --out-bin x.bin --out-json x.json`.
      `-` means stdout, and the summary goes to stderr.
    - `roundtrip --seeds N`: checks that binary↔JSON is byte-exact in both directions,
      plus the JS f64 read path, over the grid and a contract exercise per seed. It also
      checks the grid against a hand oracle written in f64, and writes
      `target/gates/roundtrip.json`.
    - The capabilities ledger now has 9 rows (`layout.grid` added). The verdict accepts
      any current control that went red on the row's stage, and reads
      `oracle_record = oracle-diff | roundtrip`.
  - `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings` and
    `cargo test --workspace` are all green. The `graph-core` wasm32 build is green.

## User decisions already taken (do not re-ask)

- **Q1: `child_of` orientation → option (c).** The direction is decided per type.
  - `child_of` edges are flipped when the index is built, so the hierarchy CSR always
    reads parent → children.
  - `parent`, `parent_of` and `*hierarchy*` keep source = parent.
  - This needs a direction flag kept at classification time, because
    `edge_kind_from_type` currently throws the type away.
  - Do this **first in Phase 3**, before any tree layout reads `Topology::hierarchy`.
    Remove the Ponytail on `topology.csr` and `Topology::hierarchy` once it is fixed.
- **Q2: snapshot format → option (b).** Stay on 0.x (0.2); do not declare 1.0 until
  Phase 4's zero-copy transport has consumed the format. Caveat to keep in the docs:
  under 0.x a breaking change is a minor bump, which a 0.2 reader does not refuse. That
  is acceptable only while nothing saves snapshots.

## Remaining — Phase 2 (finish before Phase 3)

1. **House-limit fixes, not done yet.** Found with a scan of every file changed since
   commit `c307300`:
   - `crates/graph-cli/src/capabilities/tests.rs` is 306 lines; trim or split it.
   - `crates/graph-cli/tests/cli.rs` is ~356 lines. Move the `snapshot` and `roundtrip`
     tests into a new `crates/graph-cli/tests/snapshot.rs`, duplicating the small
     helpers. Split `hashgate_passes_and_each_negative_control_goes_red_on_its_own_stage`
     (54 lines) into an honest test and a controls test.
   - `crates/graph-contract/src/snapshot/tests.rs`:
     `every_snapshot_refusal_names_its_column_and_position` is 77 lines; split it.
   - `crates/graph-cli/src/hashgate.rs`: `record` takes 4 parameters plus a tuple.
     Pass `&Tally` instead.
   - `canonical_json/tests.rs` is already split, into `tests/shape.rs` and
     `tests/syntax.rs`.
   - `seeded_model` is flagged with "5 params", but that is a scanner false positive
     caused by the tuple return type.
2. **`docs/contract/binary-layout.md` is not written yet.** It is authoritative per the
   Phase 2 prompt §3. It must cover:
   - the byte table, from the 28-byte header (`GMSN`, version major/minor, kinds,
     stage count, n, m);
   - the node id table (u32 offsets n+1, UTF-8 bytes, zero padding to 4);
   - the edge id table, `source` u32×m and `target` u32×m;
   - the node columns in order (x, y[, r | w, h]) as f32 LE;
   - the edge geometry: Line has none; Polyline has offsets m+1 then pts; Curve has the
     degree u32 first;
   - the refusal rules and the version policy (Q2);
   - the JSON face's shape.
   The pinned 80-byte example in `binary/tests.rs` is the ground truth.
3. **Run the Phase 2 gate** (`prompts/phase-02-contract-registry-grid.md` §Gate) and keep
   the logs. Run these rows in addition, and report them as deviations:
   - `GM_MUTATE_GRID_SPACING=2 … hashgate --seeds 8` must exit 1; it backs the
     `layout.grid` row;
   - `graph-cli codegen --check` must exit 0.
   Then run `capabilities --check`, which needs `hashgate --seeds 1000`, both controls,
   `roundtrip --seeds 1000` and `oracle-diff` on the current tree.
   **UNKNOWN = FAIL: none of these has been run on the final tree.**
4. Run `cargo-mutants` over `git diff c307300 HEAD` using the `ge-mutants` image
   (built from this repo).
5. Write `docs/reports/phase-02.md` in the shape of `prompt.md` §12, using `phase-01.md`
   as the template. It must include:
   - the `snapshot.json` for seed 1 at N=50, from
     `graph-cli snapshot --seed 1 --nodes 50 --layout grid --out-json -`;
   - every deviation (see the list below);
   - Q1 and Q2 recorded as resolved.

### Phase 2 deviations to report

- New child files forced by the 300-line limit or a name clash:
  - `geometry/tests.rs`;
  - `binary/{decode,tests}.rs`;
  - `canonical_json/{parse,read,schema,tests}.rs`;
  - `canonical_json/tests/{shape,syntax}.rs`;
  - `stage/topology.rs` (moved from `stage.rs`);
  - `snapshot_cmd/{exercise,tests}.rs`.
- Files modified outside the envelope:
  - `graph-cli/src/codegen.rs` (three outputs, workspace-relative paths);
  - `graph-wasm/src/lib.rs`;
  - `capabilities/{registry,verdict,tests}.rs`;
  - `hashgate/tests.rs`;
  - `graph-cli/tests/cli.rs`;
  - `graph-core/tests/memory.rs` (the grid ceiling measurement);
  - `generated/snapshot-header.*` (regenerated: `deny_unknown_fields` on the version);
  - `graph-contract/src/snapshot/tests.rs`;
  - `docs/reports/HANDOFF.md` (this file).
- The Phase 0 `synthetic_snapshot` stub and the `gm_synthetic` export were removed, and
  the hashgate stage `synthetic` was replaced by `layout.grid`.
- A second negative control, `GM_MUTATE_GRID_SPACING`, was added, because the grid
  ignores weights and the reference-degree control cannot reach `layout.grid`.
  Control record names changed from `hashgate-control.json` to
  `hashgate-control-<knob>.json`.

## Remaining — Phases 3 to 10

Follow `prompts/phase-03-*.md` through `phase-10-*.md`, one phase at a time, each with its
gate, a commit and push, and a report.

- **Phase 3**: tidy tree, treemap, circular, circle packing. Start with Q1 (c) above.
- **Phase 4**: wasm ABI and JS SDK. Revisit Q2 (1.0) there.
- **Phase 5**: Sugiyama.
- **Phase 6**: iterative, spectral and MDS layouts.
- **Phase 7**: analysis (petgraph is allowed from here on).
- **Phase 8**: post-processing, routing, bundling.
- **Phase 9**: scale and bench. The memory budget is 442 B/node, against §5.1's
  33 B/node.
- **Phase 10**: ingest, SDK, publish.

## Still open from Phase 1 (need the user or the osionos host)

- Ratify the Phase 1 deviations (`phase-01.md` §1).
- osionos host items: the baseline and the guard rows; whether osionos persists
  `makeEdgeId` output (H1 id migration); running the literal `ge-check` on a host without
  TLS interception. osionos is READ ONLY.
- Documentation corrections F1–F8 (`phase-01.md` §8 item 4).
