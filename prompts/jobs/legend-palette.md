# Job legend-palette (agent build: one legend row per document group, with its own count)

Why: colouring by the document's own group column assigns a node the slot `group index % 10`
and names that slot after group index `slot`. A document with more than 10 groups therefore draws
groups 0, 10, 20, … in one colour. The legend then shows that colour's total under group 0's name.

Measured on 2026-10-05 with a repository history of 85,928 records and 2,486 authors:
- the legend read "Ilia K 10348";
- the document holds 1 record for that name.

The studio's own header (`look/colourBy.ts:6-8`) says categorical modes give one palette entry per
distinct key, first seen first, up to `MAX_KEYS = 512`, with every further key sharing one MUTED
slot. `kind`, `tag` and `db` do that through `categoricalOf`; the document's group column does not.

The hub owner (session graph-render-0e) confirmed on 2026-10-05 that no job of theirs touches the
studio's look code or `ui/Legend.tsx`.

Facts (develop, 2026-10-05; re-check each on your branch before editing, and stop if one no longer
holds):
- `packages/graph-studio/src/look/groupOverlay.ts:47-51` `documentGroups(meta)`:
  `colours[i] = (meta.group[i] ?? 0) % GROUP_PALETTE.length`, `palette: GROUP_PALETTE`,
  `names: (slot) => meta.groups[slot] ?? \`#${slot}\``. It is called from `look/colourBy.ts:75`
  (`by === "group"`) and `groupOverlay.ts:57` (user groups that match nothing).
- `packages/graph-studio/src/look/categorical.ts` `categoricalOf(keys, labels)`:
  - one slot per distinct key, first seen first, `MAX_KEYS = 512`;
  - slot `i` is coloured `GROUP_PALETTE[i % 10]`, except the empty key, which is MUTED;
  - past 512 keys, one more MUTED slot named `labels.overflow`.
- `packages/graph-studio/src/look/palette.ts`: `GROUP_PALETTE` has 10 entries.
- `packages/graph-studio/src/source/meta.ts`:
  - `groups` holds the names, first seen first, in node order;
  - `group` is a `Uint16Array` index per node;
  - `UNGROUPED = "(no group)"` (`:39`).

  So group index `i` is the `i`-th distinct name in node order, which is the slot `categoricalOf`
  gives it. The first 512 groups keep the colour they have today.
- `packages/graph-studio/src/look/styleOf.ts`:
  - `legendOf` counts nodes per slot;
  - it lists the first `LEGEND_ROWS = 12` slots in palette order.
- Tests: `packages/graph-studio/tests/colour-by.test.ts` (has `makeMeta`) and
  `tests/look.test.ts` (`legendOf`).

Change, exactly:
1. In `groupOverlay.ts`, `documentGroups(meta)` returns
   ```ts
   categoricalOf(Array.from(meta.group, (index) => meta.groups[index] ?? ""), { empty: UNGROUPED, overflow: "(other groups)" })
   ```
   It imports `categoricalOf` from `./categorical.ts` and `UNGROUPED` from `../source/meta.ts`. If
   that import makes a cycle tsc or eslint refuses, use the literal `"(no group)"` and say so.
2. Remove `GROUP_PALETTE` from `groupOverlay.ts`'s imports if nothing else there uses it.
3. Change nothing else in `src/`. The `Ponytail:` paragraph on `MAX_KEYS` in `colourBy.ts:19-22`
   now covers groups too; leave it.

Tests, in the files' own style. Each must fail before the change:
- `colour-by.test.ts`, `by: "group"`:
  - on a meta with 12 groups `G0`…`G11` and one node each, `colours` is `[0, 1, …, 11]`, not
    `[0, …, 9, 0, 1]`;
  - `names(10)` is `"G10"` and `names(11)` is `"G11"`.
- `colour-by.test.ts`, `by: "group"`, on a meta with 514 groups and one node each:
  - nodes 512 and 513 share slot 512;
  - that slot is named `"(other groups)"` and painted MUTED.
- `look.test.ts`, `legendOf` with `colourBy: "group"`, on a meta with 11 groups where group `G10`
  has 3 nodes and `G0` has 1:
  - the legend lists `G0` with count 1 and `G10` with count 3, as separate rows;
  - before the change, `G0`'s row reads 4 and there is no `G10` row.

Rules (beyond `scripts/orch/common.md`):
- Studio code: no type assertions (`as`), no `any`, no `eslint-disable`.
- `scripts/studio.sh` is the only way to type-check, lint, test and build.
- Paths you may touch:
  - `packages/graph-studio/src/look/groupOverlay.ts`;
  - `packages/graph-studio/tests/**`.

  Nothing else. `scripts/orch/rows/legend-palette.rows` is already on develop: do not edit it.
- A test elsewhere that pinned the old `% 10` sharing gets its expectation changed to the new
  slots, and nothing else in it changes. List every such test in the return block.

Done when:
- `scripts/orch/gate.sh target/rows-legend-palette scripts/orch/rows/legend-palette.rows` writes a
  `summary.txt` with every row PASS.

Return:
- the branch tip;
- the files changed, with line counts;
- every test added (name → the assertion that failed before the change);
- every existing test changed, and why;
- each row's result;
- every deviation.
