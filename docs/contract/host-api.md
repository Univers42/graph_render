# Host API v1: what a page may call on `<graph-studio>`

Status: **draft, 2026-10-03**. It needs a devil verdict, because it is a public surface. The verdict's
conditions are appended under "Verdict" and bind the code.
Today a host reaches into `element.studio.store` (`packages/graph-studio/src/element.ts:43-64`); this
contract replaces those reaches. Everything not listed here is internal and may change without notice.

## The rule: a node is a reference

A node carries a stable string **id** plus light, fixed metadata: `label`, `kind`, `group`,
`database_id`, `path`, `has_note`, `icon` and `tags` (`IngestNode`,
`packages/graph-studio/src/source/ingest.ts:25`). The **content** a node stands for (a page, a row or a
note body) never enters the element, the worker, the motor or the renderer. The host resolves an id to
content when it needs to (`resolve`, below). Every id that crosses this API is the host's own string;
the dense index never leaves the element.

## Surface (all additive; `studio`, `view` and `stopMotor` stay as they are)

```ts
interface GraphStudioElement extends HTMLElement {
  readonly hostApi: 1;                                    // bumped on any breaking change
  load(doc: IngestDoc): Promise<LoadResult>;              // replaces the whole graph
  applyDeltas(batch: DeltaBatch): Promise<DeltaResult>;   // docs/contract/delta.md; coalesced per frame
  focus(id: string, opts?: { select?: boolean; zoom?: number }): boolean; // false: unknown id
  select(ids: readonly string[]): void;                   // [] clears
  readonly selection: readonly string[];
  resolve: ((id: string, signal: AbortSignal) => Promise<NodePreview>) | null;  // set by the host
}
interface NodePreview { title: string; text?: string; url?: string; icon?: string }
type LoadResult  = { nodes: number; edges: number; notes: readonly string[] };   // notes: fills/drops (ingest.ts)
type DeltaResult = { applied: number; refused: readonly { id: string; error: string }[] };
```

- `load` is the documented path for what the `fixtures` attribute and a file drop do today. It runs the
  same normaliser (`source/ingest.ts`), so it refuses exactly what a drop refuses, with the same typed
  error. It resolves once the first frame of the new graph is drawn.
- `applyDeltas` belongs to service-dod step 3 (live growth). Until that lands, it rejects with
  `NotYetSupported`, and the element still declares it, so a host can feature-test.
- `focus` moves the camera to the node and returns at once; the camera animates. An unknown id
  returns `false` and moves nothing.

## Events (DOM `CustomEvent`, `bubbles: true`, `composed: true`)

| Event | `detail` | When |
|---|---|---|
| `graph-load` | `LoadResult` | a graph finished loading, from any source |
| `node-select` | `{ ids: string[] }` | the selection changed, from a click, a box or `select()` |
| `node-open` | `{ id: string, via: "dblclick" \| "enter" \| "inspector" }` | the user asked to open a node. The element opens nothing itself |
| `node-hover` | `{ id: string \| null }` | the pointer entered a node, or left all nodes. Throttled to one event per animation frame |
| `graph-error` | `{ error: string, message: string }` | a load, a delta or the motor failed; the same code the UI shows |

`detail` holds ids and plain data only, never a live object, so a host can post it straight to a
worker or a backend.

## `resolve` and the preview cache

- The host sets `element.resolve`. The element calls it for the hover card and the inspector only,
  never for layout or drawing.
- The calls are debounced (hover: 150 ms). They are aborted through `signal` when the pointer moves on,
  and cached in a bounded LRU of 256 previews keyed by id. `load` clears it, and so does a delta that
  touches the id.
- Caveat: the LRU holds what the host returned and never re-validates it. A preview edited on the host
  side shows stale until the next `load`, a delta on that id, or eviction. A host that needs fresher
  previews calls `element.invalidate(id)`.
- When `resolve` is `null`, the card shows `label`, `kind` and `path` from the node's own metadata, as
  today.

## Gates

| Row | Passes when | Negative control |
|---|---|---|
| `host-api-unit` | `node:test` over a fake DOM: each method and event, the LRU bound, abort on move, `focus` on an unknown id | drop the `composed` flag: the shadow-crossing test is red |
| `studio-embed` | browser gate over `app/embed.html`: loads columns through `load`, gets `node-open` with the clicked id, shows a `resolve`d preview, and streams deltas when step 3 has landed | `STUDIO_EMBED_BREAK=1` stops dispatching `node-open`: red |

## Verdict

(pending)
