# Contract 3D: the devil's verdict on `contract-3d.md`

- Status: accepted, with conditions
- Date: 2026-09-30
- Deciders: the devil review (session graph-render-12); the 3D implementation job has to live with it
- Rules on: `docs/decisions/contract-3d.md` (branch `contract-3d`, 79b55f9). That doc ends with
  its author's own verdict, and this record supersedes it.

## Context

The design is sound in shape. `dim` goes in reserved byte 14, the z column goes after `y`,
and the renderer and studio refuse 3D by name. A 0.3 reader already refuses a 3D snapshot
loudly: `decode` checks the major (`snapshot.rs:157`) and then byte 14 (`snapshot.rs:158`),
before any tag.

Its load-bearing claim does not hold as written: "every existing 2D snapshot keeps
byte-identical output, and no recorded 2D hash changes" (§1, proof step 1: "bytes 0-13 are
untouched"). The reasons:

- Bytes 8-11 of the header are the format minor (`snapshot.rs:11`, written at `snapshot.rs:135`).
- Every layout labels its snapshot `CURRENT_VERSION` (`graph-core/src/layout/mod.rs:54`),
  and so do `snapshot_cmd/dag.rs:70` and `exercise.rs:108`.
- So moving `CURRENT_VERSION` from 0.3 to 0.4 (`version.rs:33`), as §1 proposes, rewrites
  byte 8 of every 2D snapshot. Every 2D hash changes, and `to_json` changes its `"version"`
  member.
- The pinned test says so directly: `binary/tests/pinned.rs:29` asserts `expected[8] == 3`.

U1 ("has anything persisted snapshots?") has an answer: yes. The studio's
`export.snapshot` action saves the snapshot's bytes to a file (`graph-studio/src/actions/export.ts:37-38`).
The doc's recommended check looked only at `session.ts` and the hashgate transport stage, so
it missed this.

It does not force a major bump, though. `version.rs:26-29` defines major as "any change an
older reader would misread" and minor as additive. Under the design:

- a 0.4 reader reads a persisted 0.3 file unchanged, because byte 14 is 0;
- a 0.3 reader refuses a 3D file instead of misreading it.

Both directions are safe, so this is a minor change. The owed 1.0 declaration
(`binary-layout.md:272-276`: "once Phase 4's zero-copy transport has consumed this format")
is a separate, older decision. It is not this change's to make.

The JSON section contradicts itself. §1 says `dim` is "written from 0.4 on", while proof
step 4 says it is "omitted for `dim = 0`". The byte-identity claim needs the second reading.

## Decision

**PROCEED-WITH-CONDITIONS.** The contract change is right. Its version rule is not.

1. **A snapshot is labelled with the lowest version that can express it.** A `dim = 0`
   snapshot keeps its 0.3 label, and only a `dim = 1` snapshot is labelled 0.4. This is
   the notes precedent: "writing follows the snapshot's own version"
   (`binary-layout.md:267-269`).
   - Every producer that writes `CURRENT_VERSION` today switches to that rule, in one
     place, not per call site: `layout/mod.rs:54`, `snapshot_cmd/dag.rs:70` and
     `exercise.rs:108`.
   - `CURRENT_VERSION` (the highest version the crate writes and reads) may still become 0.4.
2. **Byte identity is proven, not argued.** On the tree before the change and on the tree
   after it, record every 2D layout's `hashgate --seeds 8` digest per stage. The two lists
   must be equal. `pinned.rs` stays green unedited, `expected[8] == 3` included.
3. **The JSON rule is one sentence.** `"dim"` is emitted iff the snapshot is labelled 0.4
   or later, which by condition 1 means iff `dim = 1`. It reads as 0 when absent.
4. **The three hand-mirrored column tables move in one commit**: `graph-wasm/src/views.rs`,
   `graph-sdk-js` `types.ts`, and `graph-render` `decode.ts`. This carries over the author's
   condition 3 (the F2/F3 silent mislabels).
5. **The renderer and studio refuse 3D by name** (`"dimension-3d"`), and the studio's
   `export.snapshot` of a 3D run is covered by a test. This carries over the author's
   condition 4.
6. **The author's condition 2 (A3) stands.** No POST or SCALE pass rewrites node columns.
   Today only `post/mod.rs:108-109` and `post/grid_index/build.rs:47-56` read the
   `NodeGeometry` variants, and nothing in `post/` constructs one. The implementation job
   re-checks this on its own tree.

## Alternatives considered

- **Bump every snapshot to 0.4, as written**: every recorded 2D digest and every
  cross-commit golden would be voided. That includes the `sim` branch's 65-digest golden
  against 8e8e93b (`prompts/RESUME.md`, the `sim` row). This loses to condition 1, which
  keeps them at no cost.
- **Major bump (1.0)**: no reader misreads under the design, so `version.rs:26-29` does not
  call for it. Declaring 1.0 is owed on other grounds and is taken separately.

## Consequences

- The implementation job (`orch/prompts/p13-3d.txt`) carries conditions 1-6 as acceptance
  criteria. Condition 2 is its first gate row.
- Two versions are written at once, 0.3 for 2D and 0.4 for 3D. A reader already dispatches
  on the snapshot's own version for notes (`notes.rs:193`, `carries_notes`), so this adds a rule, not a
  mechanism.
- Still open, and not blocking: the 1.0 declaration, since exported snapshots now exist
  (`export.ts:37`).
