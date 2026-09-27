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
`LayoutController.rebuild`. So `harness/oracle-diff.mjs` runs a **transcription** of
lines 76-87 and names it `layoutGroups`. The transcription is refused (exit 2) unless
each of its lines is still in `layoutBridge.ts`, verbatim.

The cases come from `graph-cli emit-fixtures`: every 10th seed has `1 + seed % 400`
sources over as many nodes, plus 5 repeats. Seeds 260-390 therefore cross 255 groups.

A mismatch is accepted only where graph-core's group is ≥ 256 and the oracle's is that
value `& 0xff`. Any other difference fails the run. At 1000 seeds there were 100 cases:
72 equal, 28 declared H9 divergences (every case that crossed 255), and 0 unexplained.

## What it does not change

`src/` is read-only, so `layoutBridge.ts` still aliases above 255 sources. Only
graph-core's `group` column is fixed.
