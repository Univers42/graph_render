# p13-3d-seam — the layout side of the 3D contract

## What the gap was

p13-3d landed the contract: `Dim::D3` at header byte 14, `SnapshotParts.z: Option<Vec<f32>>`,
`gm_dim`, `label_for`. `graph_core::layout::Geometry` had `nodes`/`edges`/`notes` only, and
`snapshot()` hard-wired `version: label_for(Dim::D2)` and `z: None`. So no layout could emit a
3D snapshot no matter what it computed: the seam did not exist. p12-t3 (sphere, helix, cube, 3D
spring, hierarchy) and p12-t4a/t4b (3D arms of existing layouts) were blocked on this.

## (a) One constructor, one z column

`Geometry` gains `pub z: Option<Vec<f32>>`: one z per node in node order, `None` for 2D. Two
constructors, `Geometry::planar(nodes, edges, notes)` and `Geometry::in_space(nodes, edges,
notes, z)`, plus `with_edges(edges)` for the re-bundling path. Every construction site in
`graph-core/src/layout/**`, `graph-core/src/post/**` and `graph-wasm/src/post/**` now calls
`planar`; `git grep -n 'Geometry {' -- crates` finds no struct literal outside `layout/mod.rs`
itself (the remaining hits are `NodeGeometry`/`EdgeGeometry`, return types, and the `impl`
block). The point of the constructor is that the next field does not touch 27 files again.

The seam's own tests moved to `layout/tests.rs` when `layout/mod.rs` passed 300 lines; the
public surface of the module is unchanged.

`Geometry::dim()` is the single place that reads "is this 3D", so the header's label and the
byte payload cannot disagree.

## (b) `snapshot()` follows the geometry

`version: label_for(geometry.dim())`, `z: geometry.z`. A z whose length is not the node count is
refused as `SnapshotError::Length { column: "node.z", .. }`; a non-finite z is refused as
`SnapshotError::NonFinite { column: "node.z", index }`. Those come from the contract's existing
column checks — no new rule, no new error variant, so D9 has one enforcement point rather than
two.

## (c) Every 2D byte is unchanged

`z: None` for every layout in the tree, so the version label stays 0.3 and the payload is
byte-identical. `binary/tests/pinned.rs` green unedited; `hashgate --seeds 8` PASS.

## (d) Tests

In `layout/mod.rs`: a 3D geometry produces a `label_for(Dim::D3)` snapshot whose z round-trips
through both faces (`to_bytes`/`from_bytes` and `to_json`/`from_json`); a wrong-length z is
refused under `node.z`; an infinite z is refused under `node.z`. `re_edging_carries_the_z_column`
covers `with_edges`.

In `post/tests.rs` and `graph-wasm/src/post/tests/composability.rs`: every registered capability
over a 3D geometry returns the same z, plus a `z` equality assertion inside the existing
composability matrix's per-capability check. Without these the matrix would stay green forever,
because every layout it runs is 2D.

Negative control: `GM_MUTATE_NODE_Z=1 roundtrip --seeds 8` still fails (`node.z: 4 values, need
3`), exit 2. `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` still fails, exit 1.

## (e) Post passes over a 3D geometry — per pass

None of them refuse; all carry z through untouched. The reason is structural rather than
per-pass discipline: `Bundled.geometry` is rebuilt only through `Geometry::with_edges`, which
hands on `nodes`, `notes` and `z` and only replaces `edges`. `graph-core`'s `post.route.grid`
and the four styles are reached through the `graph-wasm` adapters on wasm32 and through the
same `style_edges`/`routed::route` core functions on native. `POSTS` in `graph-core` registers
the two bundlers; routing and the four styles are registered on the wasm32 side
(`graph-wasm/src/post/registry.rs`), and their adapters are the only other place a `Geometry`
is rebuilt in the workspace.

- `post.bundle.fdeb` (`post/fdeb.rs`): carries z; replaces edges with the bundled polylines.
- `post.bundle.mingle` (`post/mingle.rs`): carries z; same.
- `post.route.grid` (`graph-wasm/src/post/routed.rs`): carries z; the routed paths become the edges.
- `post.style.straight` / `orthogonal` / `quadratic` / `bezier` (`graph-wasm/src/post/styles.rs`):
  all four through one `style_run`: carries z; the styled edges become the edges.
- `post.ink` and `post.measure` (`post/ink.rs`, `post/mod.rs`): read-only, no geometry out.
- `post.grid_index` (`post/grid_index.rs`): reads `&NodeGeometry` only; no geometry out.

The reason z is out of a pass's reach in the first place is in `post/mod.rs:110`: a pass is
handed `&NodeGeometry`, and `z` lives on the `Geometry`, not on the node columns. The pass's
real projection of a 3D drawing is the 2D one it routes and styles — a bundle in the xy plane.
That is the pass's own limitation, stated at the seam, and it is not a dropped column: the
snapshot keeps its z and the label keeps it at 0.4.

Ponytail: a pass that dropped z would be a silent 3D→2D downgrade, so the assertion is in the
matrix rather than in a comment. If a future pass needs to refuse 3D (a pass that genuinely
cannot honour a third coordinate), it must return a named `StageError` — a dropped column is
the one outcome that is not allowed.

## Checks

| check | result |
|---|---|
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo test --workspace --no-fail-fast` | exit 0, 0 failures (1330 passed across 19 binaries) |
| wasm32 build, graph-core | 0 |
| wasm32 build, graph-wasm | 0 |
| `hashgate --seeds 8` | PASS, exit 0 |
| `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` | FAIL as required, exit 1 |
| `roundtrip --seeds 100` | PASS, exit 0 |
| `GM_MUTATE_NODE_Z=1 roundtrip --seeds 8` | refused as required, exit 2 |
| `codegen --check` | all 4 artifacts up to date, exit 0 |

## Decisions taken

`with_edges` was added as a third constructor for the re-bundling path rather than requiring
each pass to rebuild the struct by hand. It is inside the job's path list (`crates/graph-core/**`)
and it is what makes (e) true by construction rather than by five separate correct edits.

No contract change was needed: `SnapshotParts.z` and `Dim::D3` already existed and already
refused a misfit z.