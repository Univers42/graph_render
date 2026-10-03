# Host API v1: what a page may call on `<graph-studio>`

Status: **accepted with conditions, 2026-10-03**. The conditions under "Verdict" bind the code and
override the draft text above them.
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

**PROCEED-WITH-CONDITIONS** (independent review, 2026-10-03). Starting code before conditions 1–5 are in
this contract would be a BLOCK. Scores: blast radius 4, reversibility 4, cost on failure 4, confidence 3.
The worst axis is reversibility, tied with cost on failure. Six claims above are false on develop and are
overridden here: `delta.md`, the file drop, the hover card "as today", `invalidate`, the "same
normaliser", and the interface as written does not compile. Where a condition and the draft above
disagree, the condition wins.

1. **Rename `focus` → `focusNode(id, opts)`.** As written, the interface fails `tsc` with TS2430 against
   `HTMLElement.focus(options?)`, so the native `focus()` stays as it is. Also rename `select` →
   `selectNodes`, `selection` → `selectedIds` and `load` → `loadGraph`. Row `host-api-types` assigns the
   element to `HTMLElement` and calls `el.focus({preventScroll:true})`; its break puts the name `focus`
   back.
2. `focusNode` and `selectNodes` match exact ids in the current frame only. They never fall back to label
   matching. An unknown id returns `false` and moves nothing, including during a load. Row
   `host-api-unit`; its break routes the call through the `view.focus` action.
3. **Host data is never persisted.** A doc loaded through the API is never written to `localStorage`. An
   embedded element reopens `graph-studio.last-source` only when the host opts in. Row `host-api-storage`
   (browser) loads, then reloads, and asserts that no `graph-studio.settings.document:*` key exists and
   that no `graph-load` fires before the host's first `loadGraph`. Its break keeps `remember`.
4. **`url` is dropped from v1's `NodePreview`.** The host opens links itself, from `node-open
   {via:"card"}`. `icon` is plain text, at most 16 code points. It is never fetched and never placed in an
   `<img>` or a CSS `url()`.
5. **Escaping is gated.** The card and the inspector render preview and IngestDoc strings as React text
   only.
   - Row `host-api-escape` runs `renderToStaticMarkup` over hostile labels, paths and previews
     (`<img onerror>`, `javascript:`, `data:`, `"><script>`). It asserts no raw tag, no `on*` attribute and
     no `href`. Its break is `HOST_API_ESCAPE_BREAK=1`, which renders through `dangerouslySetInnerHTML`.
   - Row `lint`: ESLint `no-restricted-syntax` bans `dangerouslySetInnerHTML`, `innerHTML`, `outerHTML`
     and `insertAdjacentHTML` in `packages/`. Its negative control is a fixture that uses one of them.
6. **Preview bounds.**
   - Each field is type-checked with `typeof` and truncated when it enters the cache: `title` at 256 code
     points, `text` at 4096.
   - A rejection or a timeout is never cached.
   - A result that arrives after a newer load is discarded (a load-generation counter).
   - At most one `resolve` is in flight per consumer, and the inspector's call is aborted when the
     selection changes.
   - Row `host-api-unit` has one assertion each for the cap, the generation, the uncached rejection, and
     eviction at entry 257. Each has its own break.
7. **`loadGraph` semantics.**
   - It takes an object, serialises it, then runs `normaliseIngest`.
   - It resolves on `setFrame`, not on the rAF paint, so a hidden tab does not hang it.
   - A superseded call rejects with `CancelledError` and emits no `graph-load`. A call while disconnected
     rejects at once.
   - `notes` is the bounded `firstOf` list.
   - The rejection's `name` equals `graph-error.detail.error`, which equals `ShownError.code`.
   - The byte cap is the motor's `IngestTooLarge` at 1 GiB (`crates/graph-wasm/src/ingest.rs:76`).
   - Row `studio-embed`: two overlapping loads give one `graph-load` and one `CancelledError`.
8. **`applyDeltas` is out of v1.** Hosts feature-test with `"applyDeltas" in el`, and a declared method
   that always rejects defeats that test. When it lands, it is atomic per call, with no coalescing across
   calls, and `DeltaResult` drops the per-id `refused[]`. `delta.md` is cited only once it is on develop.
9. **The interface is complete.** `invalidate(id)` is declared in it. `studio`, `view` and `stopMotor` are
   `@internal` and outside the v1 promise, because they expose dense indices. Row `host-api-types`.
10. **Events.**
    - Every event has `bubbles:true` and `composed:true`.
    - `detail` is a fresh, frozen object of string ids, never a dense index.
    - `node-select` fires only when the set changes, once per gesture end, and never for a `selectNodes`
      that passes the current set.
    - `node-hover` fires at most once per frame.
    - Row `studio-embed`: a listener on `document` receives each event. Its break sets `composed:false`.
    - The fake-DOM `composed` control is dropped from `host-api-unit`: with no DOM library, it passes
      whatever the code does.
11. **Keyboard and focus.**
    - Enter fires `node-open {via:"enter"}` only when the host has focus, exactly one node is selected,
      and the target is not an input (Search already uses Enter).
    - The inspector gets an Open button reachable by Tab.
    - The hover card is new code; none exists today. It is `aria-hidden`, and the inspector mirrors its
      content.
    - Row `studio-embed` has one row per trigger (dblclick, Enter, Open), each with its own break.
12. **Versioning.** `hostApi` bumps only on a breaking change; additions are detected with `in`. Hosts
    `await customElements.whenDefined(tag)`, then check `hostApi`, because `defineGraphStudio` silently
    keeps an older definition (`element.ts:216-217`). A `resolve` set before the element upgrades is
    picked up. Row `studio-embed` sets `resolve` before `define`; its break skips the upgrade step.
13. **Host requirements.**
    - The worker and the wasm are served from the host's own origin: `element.ts:88` builds the worker
      URL from `import.meta.url`, and a Worker must be same-origin. The service's `/embed/` is therefore
      reverse-proxied under the host's origin.
    - The CSP needs `script-src 'wasm-unsafe-eval'` and `worker-src 'self'`.
    - COOP/COEP are optional. Without them the motor runs on one thread.
    - Row `studio-embed` includes one run without the cross-origin-isolation headers.
