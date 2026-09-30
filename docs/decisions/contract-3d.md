# D-3D — dim once per snapshot, a z column, 2D bytes unchanged

Status: **proposed** — written for the devil verdict (`prompts/RESUME.md:247`: "an opus devil
verdict on contract-3d, then the 3D implementation"). No code. Scope: the snapshot contract
(`crates/graph-contract`, `docs/contract/`), its consumers (`graph-wasm`, `graph-sdk-js`,
`graph-render`, the studio), and the first 3D layouts. Decides the byte layout, the JSON
shape, the version bump, and the consumer obligations. Leaves open: the exact 3D layout
set, the random-3D oracle, and whether edge paths ever go 3D.

## Context

The user approved 3D layouts on 2026-09-29, subject to a devil verdict on the contract
change (`prompts/RESUME.md:240`): "It needs a contract change (dim once per snapshot, a z
column, 2D bytes unchanged), so the design goes through a devil verdict before any code."
The job is `prompts/jobs/contract-3d.md` (design doc only; `docs.rows` = fmt).

Today the contract is 2D-only. The 28-byte header (`crates/graph-contract/src/snapshot.rs:7-19`,
`docs/contract/binary-layout.md:15-31`) carries a node geometry tag (byte 12), an edge
geometry tag (byte 13), and a **z channel** (byte 14) that is reserved and refused when
nonzero (`snapshot.rs:107` `ReservedZChannel(u8)`, `snapshot.rs:170-178` `check_reserved`,
`docs/contract/binary-layout.md:24`). Node columns are SoA: `x`, `y`, then `r` (Circle) or
`w`, `h` (Box) (`geometry.rs:174-181`, `docs/contract/binary-layout.md:47`). There is no
`dim` field anywhere in the contract. The hash is SHA-256 over the binary face's bytes
(`docs/contract/binary-layout.md:307-317`). The JSON face round-trips byte-exact through the
binary face (`graph-cli roundtrip`, `crates/graph-cli/src/snapshot_cmd.rs:140-181`).

SciGraphs computes 3D natively (`SciGraphs/core/scigraphs_core/mesh/layouts/`); the motor
deliberately stayed 2D (`docs/decisions/eigensolver.md:27-32` — spectral ported with
`dims = 2` where the reference passes 3; `docs/measurements/phase08-routing.md:211-212` —
the reference's 3D PCA `_frame` is absent). igraph FR-3D is deferred
(`prompts/REFERENCES.md:120`).

## Decision

### 1. The change

**`dim` lives in the header, once per snapshot, at byte 14.** The reserved z-channel byte
(`snapshot.rs:14`, `docs/contract/binary-layout.md:24`) is repurposed as `dim: u8`:
`0` = 2D (no z column), `1` = 3D (z column present). Values `2..=255` are refused as a new
`ReadError::ReservedDim(u8)` (renaming `ReservedZChannel`, `snapshot.rs:107`; Display:
"dim {v} is reserved and not implemented"). No new header byte: the header is pinned at 28
bytes (`snapshot.rs:29` `HEADER_LEN = 28`, `docs/contract/binary-layout.md:15`), and adding
one would break every pinned example (`binary/tests/pinned.rs:26,54,69,88`). The check
order is unchanged — `check_reserved` runs before the geometry tags
(`snapshot.rs:158`, pinned by `reserved_fields_are_checked_before_the_geometry_tag`,
`snapshot/tests.rs:76`) — so a 3D snapshot is refused by a 2D reader at byte 14, ahead of
any tag.

**The z column sits immediately after `y`, present only when `dim = 1`.** Wire order
becomes: `x, y, z` (Point 3D), `x, y, z, r` (Circle 3D), `x, y, z, w, h` (Box 3D). For
`dim = 0` the order is exactly today's `x, y` / `x, y, r` / `x, y, w, h`
(`geometry.rs:174-181`). Coordinates-first keeps `x, y, z` contiguous and matches SciGraphs'
`(n, 3)` position arrays (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:5-9`). The
reader computes column positions from `dim` in the header, never from fixed offsets, so
the `r`/`w`/`h` shift when `dim = 1` is safe. Edge paths stay 2D: `Paths` is still
`2 × offsets[m]` (`geometry.rs:153-162`, `docs/contract/binary-layout.md:94`); a 3D edge
path is a second breaking change, out of scope.

**Canonical JSON gains a top-level `"dim"` member and a conditional `z` column.** Shape
from 0.4 on: `{"dim", "edges", "geometry", "nodes", "notes", "version"}` (keys sorted by
UTF-8 bytes, `canonical_json.rs:140-153`, so `"dim"` sorts first). `dim` is `0` or `1`,
written from 0.4 on; absent below 0.4, reading as `0` — the same optional-member pattern
as `notes` (`canonical_json/read.rs:205-217`, `docs/contract/binary-layout.md:290-292`).
`geometry.nodes` gains a `"z"` array iff `dim = 1`; for `dim = 0` the object is unchanged.
The schema (`docs/contract/snapshot-schema.json`) marks `dim` with a default of `0` and
enumerates `0, 1`.

**Version bump: 0.3 → 0.4 (minor).** Under `0.x` a minor bump licenses a breaking payload
change (`docs/contract/binary-layout.md:272-276`). The breaking part is exactly one thing: a
0.3 reader refuses a 0.4 snapshot at byte 14 (`ReservedDim(1)`), which is the desired
2D-only-consumer behaviour. A 0.4 reader reads 0.3 snapshots (byte 14 = 0) unchanged. The
bump is minor — not major — because 2D bytes are unchanged (proof below), so no persisted
2D snapshot becomes unreadable and no recorded 2D hash changes. Precedent: turning on a
reserved note code is a minor bump "because a 0.3 reader refuses those codes"
(`docs/contract/binary-layout.md:269-270`); the notes section itself was 0.2 → 0.3
(`docs/contract/binary-layout.md:262-269`). `CURRENT_VERSION` moves from `0.3`
(`version.rs:33`) to `0.4`.

**Proof sketch: every existing 2D snapshot keeps byte-identical output.** The change
touches four code paths; none alters the `dim = 0` output:

1. *Header encode* (`snapshot.rs:132-140`): byte 14 is written as `self.dim`. For 2D,
   `dim = 0`, so the written byte is `0` — identical to today's literal `0`
   (`snapshot.rs:136`). Bytes 0-13 and 15-27 are untouched. The 28 header bytes are
   identical.
2. *Header decode* (`snapshot.rs:142-167`): `check_reserved` (`snapshot.rs:158`, `:170-178`)
   changes from "byte 14 must be 0" to "byte 14 must be 0 or 1". A 2D snapshot has
   byte 14 = 0, which passes both checks. The decoded header is identical.
3. *Node columns* (`geometry.rs:174-181`): `columns()` returns `x, y` / `x, y, r` /
   `x, y, w, h`. The z column is appended only when `dim = 1`. For `dim = 0` the list is
   unchanged, and `binary.rs:178-180` writes exactly that list. The node section is
   byte-identical.
4. *JSON* (`canonical_json.rs:97-130`): the `dim` member and `z` column are emitted only
   when `dim = 1`. For `dim = 0` the emitted text is unchanged (the `dim` member is
   omitted, like `notes` below 0.3). The pinned JSON text for 2D snapshots
   (`canonical_json/tests.rs:65`) is unchanged.

Edge geometry (`geometry.rs:153-162`), notes, and string tables are untouched. Therefore
`to_bytes()` and `to_json()` produce identical output for every 2D snapshot. Since the hash
is over `to_bytes()` (`docs/contract/binary-layout.md:307-317`), every recorded 2D hash
stays valid. The empirical confirmation is the existing hashgate over 2D snapshots (already
green) plus the pinned byte tests (`binary/tests/pinned.rs:26,54,69,88`), which must stay
green unchanged.

### 2. What each consumer must do

The wasm ABI and the SDK are **transport**: they carry bytes and must be
dimension-agnostic. The renderer and the studio are **presentation**: they are 2D-only for
now and must refuse 3D, never silently drop z.

**wasm ABI** (`crates/graph-wasm`):
- Append `NODE_Z: u32 = 12` to the column id table (`views.rs:32-57`; append-only, never
  renumber — `views.rs:31`). Today the next free id is `11` (`EDGE_CURVE_DEGREE`).
- Add a `"z"` arm to `node_column`'s match (`views.rs:80-86`). The current `_ => "h"`
  fallthrough is a live bug risk: a `NODE_Z` id would silently map to `"h"` and return the
  height column (or `Absent` for Point/Circle) — a silent mislabel. The new arm makes the
  fallthrough unreachable for `NODE_Z`.
- Add a `gm_dim(handle) -> u32` export returning `0` or `1`, reading the snapshot header's
  `dim`. The ABI already returns raw snapshot bytes (`gm_snapshot_bytes`,
  `exports/columns.rs:67`), but a consumer should not have to parse byte 14 itself; the
  export is the clean surface. The `Code` error enum (`errors.rs:16-61`) is append-only
  (`errors.rs:134-166`); no new error is needed here because the wasm layer does not
  refuse 3D — it carries it.
- The wasm layer links the contract crate, so it inherits the 0.4 reader. No refusal.

**SDK** (`crates/graph-sdk-js`):
- Add `NodeZ: 12` to the `ColumnId` table (`types.ts:15-31`).
- Add `NodeZ` to `F32_COLUMNS` (`views.ts:19-26`). Without it, `z` falls through to
  `Uint32Array` (`views.ts:120`) — a silent mislabel of f32 bytes as u32. This is the SDK's
  "silently drop z" failure mode.
- Add `dim: 0 | 1` to `RunResult` and `PostResult` (`types.ts:49-70`).
- Make `nodeColumnApplies` dim-aware for `z` (`views.ts:29-42`): `z` applies when
  `dim = 1`, regardless of `nodeKind`. Today it keys on `nodeKind` only.
- The SDK is transport; it carries 3D transparently. No refusal.

**Renderer** (`packages/graph-render`) — 2D-only, must refuse:
- Replace `reserved("z", bytes[14], 0)` (`decode.ts:190`) with a dim check that refuses
  `dim = 1` with a **named error**: `SnapshotRefusal("dimension-3d", "dim", "found 1, only
  0 is read")`. The `RefusalCode` union (`decode.ts:16`) gains `"dimension-3d"`. This is
  the renderer's "refuse with a named error, never silently drop z" behaviour: it does not
  read a z column (it cannot render 3D), and it does not pretend a 3D snapshot is 2D.
- `HEADER_BYTES = 28` (`decode.ts:70`) is unchanged. The renderer's check order already
  mirrors the contract's (`decode.ts:178`), so the refusal fires at byte 14, ahead of the
  geometry tags — same as the Rust reader.

**Studio** (`packages/graph-studio`) — 2D-only, must refuse:
- The studio consumes the renderer's `decodeSnapshot` (`session.ts:115`, `pipeline.ts:142-143`),
  so a renderer refusal propagates. The studio must surface it as a named error in the UI,
  not a blank canvas or a silent 2D projection.
- The parity scene reads `screen[0]` / `screen[1]` (`parity/scene.ts:84-91`, `:117-118`) —
  a 2D projection. A 3D snapshot must be refused before it reaches the parity gate.
- `RunSummary` (`pipeline.ts:136`) may carry `dim` for diagnostics, but the studio's
  rendering surface is one 2D canvas (`element.ts:96-102`); 3D rendering is out of scope.

### 3. The first 3D layouts

Easiest first. Each names its oracle. All are LAYOUT-stage only; POST and SCALE stay 2D
(see below).

1. **Sphere / shell** — SciGraphs `_sphere_layout`
   (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:22-34`): closed-form Fibonacci
   sphere, no RNG, no graph input. **Oracle: bit-identical** to SciGraphs' `_sphere_layout`
   for the same `n` and `scale` (same formula: `y = 1 − 2(i+0.5)/n`, `r = sqrt(1−y²)`,
   `θ = π(3−√5)i`). Easiest possible 3D layout: deterministic, closed-form, no eigensolve.
2. **Random 3D** — SciGraphs `_random_layout` (`basic.py:5-9`): `rng.rand(n, 3) * scale`,
   seeded. The house's version uses its own seeded generator (D1-D10). **Oracle:
   determinism** (hashgate: same seed → same bytes) **plus statistical parity** with
   SciGraphs' `_random_layout` (same `n`, `scale`: per-axis mean ≈ `scale/2`, variance
   ≈ `scale²/12`). Bit-identical parity would require porting numpy's `RandomState`; that
   is deferred (U4).
3. **Spectral 3D** — SciGraphs `_spectral_layout_3d`
   (`SciGraphs/core/scigraphs_core/mesh/layouts/networkx_layouts.py:249-269`):
   `dims = 3` on the existing eigensolve path. The house's 2D spectral is already ported
   (`docs/decisions/eigensolver.md:27-32`, dense tier at `:49-68`); 3D is a `dims`
   change, not a new solver. **Oracle: SciGraphs' `_spectral_layout_3d` / scipy LOBPCG**
   (`$GM_SCRATCH/refs/scipy-1.16.2/lobpcg.py`), 3 eigenvectors, compared up to sign after
   the house's sign-fixing (`_fix_eigenvector_signs`, `networkx_layouts.py:66-72`, ported
   per `docs/decisions/eigensolver.md:15-19`). The `n < 4` guard
   (`networkx_layouts.py:257-258`) is not ported (`docs/decisions/eigensolver.md:238`).

Next, if the verdict wants more: helix (`basic.py:65-81`, closed-form), spiral 3D
(`basic.py:36-63`, closed-form with arc reparam), MDS 3D (`networkx_layouts.py:271-291`,
`dims = 3` on the pivot path). The `_generate_z_component` helper
(`networkx_layouts.py:304-335`) — deriving z from graph structure for a 2D layout — is the
cheapest 3D after sphere, but it is a 2D layout with a synthetic third axis, not a native
3D layout; the design notes it as an option, not a first.

**POST and SCALE are 2D-only for now.** POST's `centres()`
(`crates/graph-core/src/post/mod.rs:103-111`) destructures every node kind to `(x, y)`; all
POST passes (routed `post/routed.rs:123,209`, fdeb `post/fdeb.rs:207`, mingle
`post/mingle.rs:236`, ink `post/ink.rs:44`, styles `post/styles.rs:196`, grid index
`post/grid_index.rs:132-134,170-171`) are 2D. SCALE's `hints(t, x, y, params)`
(`crates/graph-core/src/scale/lod.rs:152`) takes `x, y` only; its viewport test
(`scale/lod.rs:169-178`) is a 2D rectangle, documented as "2D, no projection matrix, no
perspective" (`scale/lod.rs:9-12`). Neither stage modifies node columns — POST adds edge
geometry, SCALE produces `Hints` (`scale/lod.rs:36-48`) — so z survives them unchanged.
That is an assumption (A3) to verify in the implementation job: if any POST pass ever
rewrites node columns, it must preserve z or refuse.

### 4. The gates

1. **hashgate over 3D snapshots** — already covered by the existing registry-driven gate:
   `hashgate/stages.rs:4-7` builds the stage list from `graph_core::registry::LAYOUTS`, and
   `hashgate/stages.rs:153-165` hashes `Snapshot::to_bytes()` without inspecting a column,
   so a new 3D layout joins with no change. Gate row `hashgate-8`
   (`scripts/orch/rows/quick.rows:5`). **Negative control:** a knob that perturbs the z
   column (e.g. `GM_MUTATE_NODE_Z`, following `crates/graph-cli/src/hashgate/knob.rs` and
   the `GM_MUTATE_*` pattern at `crates/graph-cli/tests/common/mod.rs:9-16`) must turn the
   row red.
2. **roundtrip over 3D JSON** — already covered by the sweep: `roundtrip.rs:86-103` runs
   every registered layout, `roundtrip.rs:144-152` calls `faces_agree`
   (`snapshot_cmd.rs:140-181`), which checks binary→JSON→binary and JSON→binary→JSON
   byte-exactness. A 3D layout joins automatically. **Negative control:** a 3D JSON with
   `dim = 1` but no `z` column (or `dim = 0` with one) must be refused — the schema marks
   `dim` as `0 | 1` and the z column is required iff `dim = 1`.
3. **A contract negative control** — new row `negctl-dim-z-mismatch` (pattern:
   `negctl-degree`, `scripts/orch/rows/quick.rows:6`): a snapshot with `dim = 1` but a
   z column of the wrong length must be refused as `Length { column: "node.z" }`
   (`geometry.rs:183-199` `check`, `geometry.rs:257-259` `check_len`). A snapshot with
   `dim = 0` but a z column must be refused as `TrailingBytes`
   (`decode.ts:252-254`, `docs/contract/binary-layout.md:229`).
4. **codegen** — `graph-cli codegen --check` (`codegen.rs:26-46`) compares the committed
   generated files against fresh output. The schema
   (`crates/graph-contract/generated/snapshot-header.schema.json`) and TS declarations
   (`crates/graph-contract/generated/snapshot-header.d.ts`) must be regenerated to include
   `dim`; `docs/contract/snapshot-schema.json` gains the `dim` member and the conditional
   `z` column. **Negative control:** a stale generated file fails `--check` (the existing
   mechanism, `codegen.rs:34-38`).

## 5. Assumptions, failure modes, unknowns

Written out for the verdict (`.claude/rules/risk.md:36-40`: "Externalize before you rule").

### Blast radius — which crates and files change

- `crates/graph-contract/src/snapshot.rs` — header encode/decode, byte 14 semantics,
  `ReadError::ReservedDim` (renaming `ReservedZChannel`).
- `crates/graph-contract/src/geometry.rs` — `NodeGeometry` gains a `z` field per variant,
  `columns()` appends `z` iff `dim = 1`, `check()` validates it, `node_column()` gains a
  `"z"` arm (`geometry.rs:246-254`).
- `crates/graph-contract/src/binary.rs` / `binary/decode.rs` — `to_bytes`/`from_bytes` read
  and write the z column.
- `crates/graph-contract/src/canonical_json.rs` / `canonical_json/read.rs` — JSON `dim`
  member and conditional `z` column.
- `crates/graph-contract/src/version.rs` — `CURRENT_VERSION` 0.3 → 0.4.
- `crates/graph-contract/generated/snapshot-header.schema.json` / `.d.ts` — regenerated.
- `docs/contract/binary-layout.md` — normative doc update (header table, node columns,
  JSON shape, version policy).
- `docs/contract/snapshot-schema.json` — regenerated.
- `crates/graph-wasm/src/views.rs` — `NODE_Z` id, `node_column` match.
- `crates/graph-wasm/src/exports/` — `gm_dim` export.
- `crates/graph-sdk-js/src/types.ts` / `views.ts` — `ColumnId`, `F32_COLUMNS`,
  `RunResult.dim`, `nodeColumnApplies`.
- `packages/graph-render/src/snapshot/decode.ts` — dim refusal (`decode.ts:190`).
- `packages/graph-studio/src/...` — 3D refusal surfacing.
- Tests: `binary/tests/pinned.rs`, `snapshot/tests.rs`, `geometry/tests.rs`,
  `canonical_json/tests.rs`, `roundtrip/tests.rs`, `cli.rs`.

### Reversibility
- 2D bytes are unchanged (proof sketch, §1), so no persisted 2D snapshot is affected and
  no data migration is needed.
- The bump is minor; a 0.3 reader refuses 0.4 at byte 14, so old code cannot misread new
  bytes — it fails loudly.
- Reverting the code reverts the contract. The only irreversible act is declaring 0.4:
  once 0.4 is written, reverting to 0.3 would refuse 0.4 snapshots that may exist. Because
  2D bytes are unchanged, a revert is safe **as long as no 3D snapshot has been persisted**
  — the same condition the 0.x rule already states (`docs/contract/binary-layout.md:272-276`).

### Assumptions
- **A1** — The 0.x minor-bump rule still applies: nothing persists snapshots in a way that
  a breaking minor bump would corrupt. *Unverified* — see U1.
- **A2** — The z column is added only when `dim = 1`; 2D snapshots are completely
  unchanged. (Proof sketch, §1; empirical confirmation via hashgate + pinned tests.)
- **A3** — POST and SCALE do not modify node columns, so z survives them. (Assumed from
  code reading: POST adds edge geometry, SCALE produces `Hints`. To verify in
  implementation.)
- **A4** — The renderer and studio are the only 2D-only consumers; the wasm ABI and SDK
  are transport and carry 3D transparently.
- **A5** — Byte 14 is the right home for `dim` (no new header byte; the header is pinned
  at 28 bytes).
- **A6** — The SciGraphs submodule is the oracle for 3D layouts; scipy/jama/networkx refs
  are pinned (`AGENTS.md`, `scripts/orch/scratch.sh`).

**Failure modes:**
- **F1** — A 2D-only consumer (renderer, studio) receives a 3D snapshot and silently drops
  z. *Mitigation:* named refusal (`"dimension-3d"`, §2).
- **F2** — The SDK's `F32_COLUMNS` table mislabels z as `Uint32Array` (`views.ts:120`
  fallthrough). *Mitigation:* add `NodeZ` to `F32_COLUMNS` (§2).
- **F3** — The wasm `views.rs` `_ => "h"` fallthrough (`views.rs:80-86`) mislabels z as h.
  *Mitigation:* add a `"z"` arm (§2).
- **F4** — A 0.3 reader refuses a 0.4 snapshot — desired, but a consumer that does not
  handle the refusal crashes. *Mitigation:* the refusal is a named error
  (`ReservedDim(1)`); consumers surface it.
- **F5** — Wrong version bump (major vs minor): a major bump would break old readers
  unnecessarily; a minor bump lets a 0.4 reader read 0.3. *Mitigation:* minor, justified
  by unchanged 2D bytes (§1).
- **F6** — The z column placement (after y) shifts `r`/`w`/`h` positions; a reader assuming
  fixed positions misreads. *Mitigation:* the reader computes positions from `dim` in the
  header (§1).
- **F7** — A 0.3 producer omits the JSON `dim` member; a 0.4 reader misreads. *Mitigation:*
  default to `0`, like `notes` (§1).
- **F8** — The hashgate transport stage (`hashgate/stages.rs:37-44`) uses `layout.grid`, a
  2D layout. If 3D layouts are to be gated over the real ABI (not just natively), the
  transport stage must be extended. *Mitigation:* extend the transport stage or accept that
  3D is gated natively only.

**Unknowns:**
- **U1** — Has Phase 4's zero-copy transport persisted snapshots? `docs/contract/binary-layout.md:274-275`
  says "once Phase 4's zero-copy transport has consumed this format, 1.0 is to be declared."
  If snapshots have been persisted, the 0.x minor-bump rule may no longer apply and a major
  bump (1.0) is needed. **This determines the version bump.** *Recommended answer:* check
  whether any snapshot bytes have been written to disk (the transport stage,
  `hashgate/stages.rs:37-44`, and the studio's `toBytes` path, `session.ts:34,188`); if
  none have, the minor bump stands.
- **U2** — Does the studio need to render 3D, or just refuse it? (Product decision.)
  *Recommended answer:* refuse for now; 3D rendering is a separate design.
- **U3** — Will the renderer ever render 3D, or will a separate 3D renderer be built?
  *Recommended answer:* the renderer's refusal is temporary if 3D rendering is coming;
  permanent if a separate renderer is built.
- **U4** — The exact oracle for random 3D: bit-identical (requires porting numpy's
  `RandomState`) or statistical? *Recommended answer:* statistical for the first
  implementation; bit-identical only if the parity gate demands it.
- **U5** — Does edge geometry (`Paths`) need a 3D variant? The job says "a z column" for
  nodes; edge paths are 2D (`geometry.rs:153-162`). A 3D edge path is a second breaking
  change. *Recommended answer:* edge paths stay 2D for now.
- **U6** — Is `_generate_z_component` (`networkx_layouts.py:304-335`) in scope? It derives
  z from graph structure for a 2D layout. *Recommended answer:* note it as an option (§3),
  not a first.

## Alternatives considered

- **New header byte for `dim`** — rejected: the header is pinned at 28 bytes
  (`snapshot.rs:29`); adding one breaks every pinned example (`binary/tests/pinned.rs:26`)
  and every hand-mirrored constant (`decode.ts:70` `HEADER_BYTES = 28`,
  `docs/contract/binary-layout.md:15`).
- **`dim` as the actual dimension count (2 or 3)** — rejected: a 0.3 snapshot has
  byte 14 = 0, which a `{2, 3}` reader would refuse, breaking backward compatibility. The
  `0`/`1` encoding (0 = 2D, 1 = 3D) keeps 0.3 snapshots readable.
- **z column at the end (after `r`/`w`/`h`)** — rejected: it splits the coordinates from
  the sizes, so `x, y` are no longer contiguous with `z`, and it breaks the "coordinates
  first" convention SciGraphs uses (`basic.py:5-9`). The after-y placement keeps `x, y, z`
  contiguous.
- **A `dim` export folded into `gm_geometry_kind`** — rejected: `dim` is orthogonal to
  `node_kind` (a 3D snapshot can be Point, Circle, or Box); folding them would conflate two
  independent discriminants.
- **A major bump (1.0)** — rejected: 2D bytes are unchanged, so no persisted 2D snapshot
  becomes unreadable; a major bump would break old readers for no benefit. A major bump
  is warranted only if U1 shows snapshots have been persisted.

## Pinned by

- `docs/contract/binary-layout.md` — the normative binary layout (authoritative; the code
  follows it, `binary.rs:1-4`, `snapshot.rs:1-6`).
- `crates/graph-contract/src/snapshot.rs:7-19` — the header table.
- `crates/graph-contract/src/geometry.rs:174-181` — node column wire order.
- `crates/graph-contract/src/binary.rs:178-180` — columns written in wire order.
- `crates/graph-contract/src/canonical_json.rs:97-130` — the JSON emitter.
- `crates/graph-cli/src/snapshot_cmd.rs:140-181` — the roundtrip checker.
- `scripts/orch/rows/quick.rows:1-6` — the gate rows and their negative-control pattern.
- `SciGraphs/core/scigraphs_core/mesh/layouts/` — the 3D layout references.
- `$GM_SCRATCH/refs/scipy-1.16.2/lobpcg.py` — the eigensolve oracle.

## Verdict

**PROCEED-WITH-CONDITIONS** — the design is sound, subject to:

1. **U1 is resolved before any code:** confirm no snapshot bytes have been persisted. If
   they have, the bump becomes 1.0 (major), not 0.4 (minor).
2. **A3 is verified in the implementation job:** POST and SCALE do not modify node columns.
   If any does, it must preserve z or refuse.
3. **The three hand-mirrored column tables move together** (`views.rs:32-57`,
   `types.ts:15-31`, `decode.ts:198-207`) — a `NODE_Z` id added to one and not the
   others is the F2/F3 silent-mislabel bug.
4. **The renderer and studio refuse 3D with a named error** (`"dimension-3d"`), never a
   silent 2D projection.
