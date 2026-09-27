# H9 — the node group is a `u32`, not the oracle's `Uint8Array` byte

Status: **decided** (Phase 1). Scope: the `group` column of `graph_core::Topology`.

## Context

The layout bridge gives each node the index of its `source`, in the order the sources
are first seen (`src/core/layout/layoutBridge.ts:76-87`). It stores that index in a
`Uint8Array`:

```ts
const nodeGroups = new Uint8Array(this.idList.length);
// ...
nodeGroups[i] = g & 0xff;
```

A graph with 257 sources therefore puts source 256 in group 0. Its nodes are pulled to
source 0's cluster centre. Nothing detects this, and the result looks plausible but is
wrong.

## Decision

graph-core stores the group as a `u32`: the first-seen index of the node's source among
the deduplicated nodes, with no truncation. Dropping the `& 0xff` is a bug fix and a
deliberate deviation from the oracle.

## How the differential holds us to it

This is not a function the oracle exports. It is the body of
`LayoutController.rebuild`. So `harness/oracle-h9.mjs` holds a **transcription** of
lines 76-87, and `harness/oracle-diff.mjs` runs it under the name `layoutGroups`:

- it runs over the model `rebuild` receives: `indexModel`'s de-duplicated nodes, with
  `idList` set to their ids, as `rebuild` sets it;
- it is refused (exit 2) unless its twelve lines are still one contiguous block of
  `layoutBridge.ts`, verbatim.

The cases come from `graph-cli emit-fixtures`. Every 10th seed has `1 + seed % 400`
sources over as many nodes, plus 5 repeats, so seeds 260-390 cross 255 groups. A last
node repeats the first id under a new source. `indexModel` drops it, so an arm that
grouped the raw nodes would see one group too many and fail.

The harness also runs the same loop with neither the `Uint8Array` nor the mask
(`widenedGroups`). A mismatch is accepted as H9 only if graph-core's line is exactly
those widened groups **and** the oracle's line is exactly them `& 0xff`. Any other
difference fails the run. That includes a wrong group that happens to agree in its low
byte (300 where 44 is right). The cli test corrupts one group by +256 and expects red.
The first version of this rule accepted any graph-core group ≥ 256 whose low byte
matched the oracle's, and it passed that corruption. The review caught it.

Result at 1000 seeds, on the final Phase 1 tree: 100 cases, 72 equal, 28 declared H9
divergences (every case that crossed 255 groups), 0 unexplained.

## What it does not change

`src/` is read-only, so `layoutBridge.ts` still aliases above 255 sources. Only
graph-core's `group` column is fixed.
