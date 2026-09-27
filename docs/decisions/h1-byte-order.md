# H1 — undirected edge ids are ordered by UTF-8 bytes, not `localeCompare`

Status: **decided** (Phase 1). Scope: `graph_core::make_edge_id`.

## Context

The oracle builds an undirected edge id with `src/core/model/ids.ts:78-87`:

```ts
[source, target].sort((a, b) => a.localeCompare(b)).join("--")
```

`localeCompare` depends on the runtime's ICU data and default locale. The same two ids
can therefore get a different edge id on another machine, another Node build or another
browser. That breaks the property the function exists for: A–B and B–A collapse to one
id. The motor has to produce the same bytes on native and on wasm32 (D7), and it has no
ICU at all.

## Measurement

Measured in `node:22-slim`: Node v22.23.3, ICU 78.3, default locale `en-US`. The pairs
are the ones in `fixtures/adversarial-ids.json`. "Oracle first" means the endpoint that
`localeCompare` puts first. "Bytes first" means the endpoint that UTF-8 byte order puts
first.

| pair | `localeCompare` sign | byte-order sign | oracle and bytes agree? |
|---|---|---|---|
| `"a"`, `"A"` | −1 | +1 | **no** |
| `"Z"`, `"a"` | +1 | −1 | **no** |
| `"note:1"`, `"NOTE:1"` | −1 | +1 | **no** |
| `"_x"`, `"ax"` | −1 | −1 | yes |
| `"é"`, `"e"` | +1 | +1 | yes |
| `"10"`, `"9"` | −1 | −1 | yes |
| `"a-b"`, `"ab"` | −1 | −1 | yes |
| `"a_b"`, `"a-b"` | −1 | +1 | **no** |
| `"é"`, `"f"` | −1 | +1 | **no** |
| `"x"`, `"🚀"` | +1 | −1 | **no** |
| `"Ａ"` (U+FF21), `"🚀"` | +1 | −1 | **no**. JavaScript's `<` (UTF-16 code units) also says +1 here, so UTF-16 order is not the contract either. |
| `"é"` (NFC), `"é"` (NFD) | **0** | +1 | **no**. A 0 keeps argument order under a stable sort, so the oracle gives A–B and B–A **two different ids**. |

The first seven rows are the pairs that `prompt.md` §7.3 measured: 3 of the 7 disagree,
as it says. The fixture holds 20 pairs: 13 that diverge and 7 controls.

## Decision

`make_edge_id` puts the endpoint that is smaller in **UTF-8 byte order** (`str::cmp`)
first. Two equal ids give the same result either way. The rule is total, needs no locale,
and gives the same answer on every target. It also always collapses A–B and B–A,
including the NFC/NFD case where the oracle does not.

## How the differential holds us to it

`harness/oracle-diff.mjs` accepts a `makeEdgeId` mismatch only if all of these hold:

- the edge is undirected;
- `localeCompare` and byte order pick different first endpoints;
- graph-core's id is exactly the byte-ordered id;
- the oracle's id is exactly the locale-ordered id.

Any other difference is unexplained and fails the run. The harness also checks each
adversarial pair's `diverges` flag against what it observed. It fails if it observes
fewer than 3 H1 divergences. Result at 1000 seeds, on the final Phase 1 tree: 1080
`makeEdgeId` cases, 931 byte-equal, 149 declared H1 divergences over 148 distinct pairs,
0 unexplained.

## Blast radius — re-verified by grep, not copied

The following was run in this repo on the Phase 1 tree:

```sh
grep -rn "makeEdgeId" src      # ids.ts (definition), index.ts:31 (re-export). No caller in src/.
grep -rn "EdgeId" src          # types.ts; model.ts:41-42; neighborhood.ts:23,25,64; diff.ts:66-82
grep -rn "localStorage\|sessionStorage\|indexedDB\|postMessage\|fetch(" src
```

- An `EdgeId` exists only as an in-memory key or value: the `edgeById` and `adjacency`
  maps (`model.ts:41-42`), the BFS set (`neighborhood.ts:23,25,64`) and a patch's
  `removedEdgeIds` (`diff.ts:66-71`).
- The only thing persisted is `Controls`, through `localStorage` (`useControls.ts:43,60`).
  Its fields hold database names, node kinds, tags, colours, physics, visual settings
  and a search query (`core/state/controls.ts:13-52`). None of them is an edge id.
- The layout worker gets endpoint **indices**, not ids (`layoutBridge.ts:58-62`).
- Exports are PNG and SVG images (`SearchExportPanel.tsx:47-53`).

**Nothing in this repo stores an edge id, so there is no data here to migrate.**

**Open:** a host that calls the public `makeEdgeId` and persists the result would see its
ids change for the diverging pairs. The one known host, osionos, is not in this container
and is read-only. Whether it persists edge ids is **not verified here**. It is a host item
in the phase report.
