# Host API v1: what a page may call on `<graph-studio>`

Status: **accepted with conditions, 2026-10-03**. The conditions under "Verdict" bind the code and
override the draft text above them.
Today a host reaches into `element.studio.store` (`packages/graph-studio/src/element.ts:43-64`); this
contract replaces those reaches. Everything not listed here is internal and may change without notice.

How a host ships the files behind this API — the ESM pack, both wasm artifacts, the CSP and the
COOP/COEP it needs, and what the serial fallback costs — is [`packaging.md`](packaging.md).

## The rule: a node is a reference

A node carries a stable string **id** plus light, fixed metadata: `label`, `kind`, `group`,
`database_id`, `path`, `has_note`, `icon` and `tags` (`IngestNode`,
`packages/graph-studio/src/source/ingest.ts:25`). The **content** a node stands for (a page, a row or a
note body) never enters the element, the worker, the motor or the renderer. The host resolves an id to
content when it needs to (`resolve`, below). Every id that crosses this API is the host's own string;
the dense index never leaves the element.

## Surface (all additive)

As built, 2026-10-04 (`packages/graph-studio/src/host/contract.ts`). The draft's `load`, `focus`,
`select`, `selection`, `applyDeltas` and `NodePreview.url` are gone, per the verdict below.

```ts
interface GraphStudioHost {
  readonly hostApi: 2;                                   // bumped on a breaking change only
  loadGraph(doc: object): Promise<LoadResult>;           // replaces the whole graph
  loadColumns(rows: ColumnRowsLike): Promise<LoadResult>; // the host's own columnar document
  focusNode(id: string): Promise<boolean>;               // centres and selects; false: no node has this exact id
  selectNodes(ids: readonly string[]): Promise<boolean>; // [] clears; false: an id is unknown, nothing changes
  readonly selectedIds: readonly string[];
  resolve: ((id: string, signal: AbortSignal) => Promise<NodePreview>) | null;  // set by the host
  invalidate(id: string): void;                          // forgets id's cached preview and asks again
}
interface GraphStudioElement extends HTMLElement, GraphStudioHost {}  // plus @internal studio, view, stopMotor, watchdogBoundMs
interface NodePreview { title: string; text?: string; icon?: string }
type LoadResult = { nodes: number; edges: number; notes: readonly string[] };  // notes: fills/drops (ingest.ts)
```

- `loadGraph` serialises the object and runs the normaliser behind the `fixtures` attribute and the
  paste action (`source/ingest.ts`), so it refuses what they refuse, with the same typed error. It
  resolves when the new graph's frame is set (verdict 7).
- `loadColumns` takes the host's own columnar document (`docs/contract/ingest-columns.md`) and is
  detected with `"loadColumns" in el`, as every addition here is. The rows are assembled in the
  worker, so the host page never imports the motor's SDK. `ColumnRowsLike` is exported from the one
  file a host imports (`packages/graph-studio/src/element.ts`). Its own clause is condition 7.
- `focusNode` and `selectNodes` answer with a promise, and it settles when the command has run; the
  camera still animates. It takes no options: the draft's `select` and `zoom` had no caller, and an
  options argument can be added later without a break. `hostApi` is 2 for this: at 1 both answered
  with a boolean at once, which a `loadGraph` landing in the same turn could contradict.

## Events (DOM `CustomEvent`, `bubbles: true`, `composed: true`)

| Event | `detail` | When |
|---|---|---|
| `graph-load` | `LoadResult` | a graph finished loading, from any source |
| `node-select` | `{ ids: string[] }` | the selection changed, from a click, a box or `selectNodes()` |
| `node-open` | `{ id: string, via: "dblclick" \| "enter" \| "inspector" }` | the user asked to open a node. The element opens nothing itself |
| `node-hover` | `{ id: string \| null }` | the pointer entered a node, or left all nodes. Throttled to one event per animation frame |
| `graph-error` | `{ error: string, message: string }` | a load, a refused command or the motor failed. `error` is the code the UI shows, else the error's name (`wireError`) |

`detail` holds ids and plain data only, never a live object, so a host can post it straight to a
worker or a backend.

## `resolve` and the preview cache

- The host sets `element.resolve`. The element calls it for the hover card and the inspector only,
  never for layout or drawing.
- The calls are debounced (hover: 150 ms). They are aborted through `signal` when the pointer moves on,
  and cached in a bounded LRU of 256 previews keyed by id. A new graph, from any source, clears it.
- Caveat: the LRU holds what the host returned and never re-validates it. A preview edited on the host
  side shows stale until the next graph or eviction. A host that needs fresher previews calls
  `element.invalidate(id)`.
- When `resolve` is `null`, the card shows `label`, `kind` and `path` from the node's own metadata, as
  today.

## The embed example (`app/embed.html`)

`app/src/embed.ts` is the host page the gate drives, and it streams: its Replay button reads
`fixtures/embed/replay.jsonl` and hands each line to `applyDeltas` one batch at a time, awaiting
every answer before the next call, because the verb is atomic per call and is not coalesced across
calls (condition 8). A refused line pushes the motor's error `name` and the replay goes on; the page
keeps the outcome on `window.__embed.replay` and the gate reads it there. Rows
`embed-replay-applied`, `embed-replay-refused` and `embed-replay-drawn` (`deploy/nav/embedreplay.py`);
`break-replay` serves the file a line short and both of the others must FAIL.

## Gates

As built. The first four run inside `scripts/studio.sh check`.

| Row | Passes when | Negative control |
|---|---|---|
| `host-api-unit` | `node:test` (`tests/host-*.test.ts`): each verb and event, exact ids, the LRU bound, the generation, the uncached rejection, abort on move, `loadGraph` superseded and disconnected, and `loadGraph` re-entered from its own `graph-load` handler | each check also runs on a subject broken the way it exists to catch, and must fail there; `tests/host-supersede.test.ts` was run with the pipeline's token check removed and failed there |
| `host-api-types` | `tests/host-types.test.ts`: the element is an `HTMLElement`, and `focus({preventScroll:true})` still works | `tests/breaks/focus-name.ts` puts the name `focus` back; `tsc` must fail with TS2430 only |
| `host-api-escape` | `renderToStaticMarkup` over hostile labels, paths and previews: no raw tag, no `on*`, no `href` | `HOST_API_ESCAPE_BREAK=1` renders through `dangerouslySetInnerHTML` |
| `lint` | ESLint bans the five markup sinks in `packages/` | `tests/ui/raw-html.tsx` must raise all five |
| `studio-embed` | `scripts/studio-embed.sh` over `app/embed.html`, three runs: `plain` (no COOP/COEP), `isolated`, `csp` (verdict 13's CSP). Rows: the load as the smoke gate judges it, `loadGraph`, a pre-upgrade `resolve`, dblclick, Enter, Open, an overlapping load and one re-entered from its `graph-load` handler, a refused load, `composed` events and storage (row `host-api-storage` is `embed-storage` here), and the Replay button streaming a JSONL of batches through `applyDeltas` | `STUDIO_EMBED_BREAK=1`: sixteen runs, one fault each, injected over CDP or in the bytes served; every targeted row must FAIL for its own reason, and a targeted row that could not be measured at all (`NOT-RUN`) counts as a control that did not bite |

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
   - **`loadColumns`.** Every clause above it, kept: it resolves on `setFrame`; a superseded call
     rejects `CancelledError` and emits no `graph-load` **across both verbs** — a `loadColumns` that
     overtakes a `loadGraph` rejects the loser, and the other way round, because the token is the
     studio's own generation and not the verb's; a call while disconnected rejects at once with
     `InvalidStateError`; `notes` is the bounded `firstOf` list, which is `[]` on this path; and
     **nothing is persisted**, because the columns source carries the same `host: true` marker
     `loadGraph`'s does and `state/persist.ts` reads. `hostApi` stays `2`; hosts detect it with
     `"loadColumns" in el`.
   - **Its cap is the motor's `MAX_INGEST_BYTES`, 1,073,741,824, refused as `IngestTooLarge`**
     (`crates/graph-core/src/ingest.rs:77`) — **not** `loadGraph`'s `MAX_DOCUMENT_CHARS` of 2^28,
     because this path writes no JSON string. A host that applies 2^28 to columns and 1 GiB to a
     document is wrong in both directions.
   - **A refusal names the class, not the code.** The rejection's `name` is `ColumnsRefusedError`
     (`crates/graph-sdk-js/src/errors.ts:86`), the sibling of `BuildRefusedError`; the ABI code
     behind it is `ColumnsInvalid`, and that is what the one `graph-error` carries in
     `detail.error`. `loadGraph` names a refusal by its wire code instead, so on this verb the two
     are deliberately not the same string, as they are for `loadGraph`.
   - A refused *field* — `rows` that is not an object, a column that is not the typed array
     `ColumnRowsLike` names, a cell array that is not a whole number of rows, or an `f64` column
     whose length is not the count `ingest-columns.md:19-21` states — rejects with a `TypeError`
     naming that field, before anything is dispatched and with no `graph-error`, exactly as a
     non-object does for `loadGraph`.
   - Caveat: the typed arrays reach the worker by structured clone — `Port.send` carries no
     transfer list — so the page holds the columns and the worker's copy of them at once, and a
     transfer would have detached the very arrays `Document.nodes` is built from.
8. **`applyDeltas` is outside the v1 promise.** It landed with P4c (`packages/graph-studio/src/element.ts:104`,
   `host/contract.ts:88`); the batch, its refusals and the ABI under it are `docs/contract/delta.md`, and
   the measurement is `docs/measurements/perf-p4c-studio.md`. Hosts feature-test with `"applyDeltas" in el`.
   It is atomic per call, with no coalescing across calls, and resolves with `{ applied }` only: no per-id
   `refused[]`.
9. **The interface is complete.** `invalidate(id)` is declared in it. `studio`, `view` and `stopMotor` are
   `@internal` and outside the v1 promise, because they expose dense indices. Row `host-api-types`.
   The four `@internal` members (`studio`, `view`, `stopMotor`, `watchdogBoundMs`) are present on the
   element at run time, are not part of the promised contract, and a call made after `stopMotor()`
   rejects with `CancelledError`.
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
    - The worker and the wasm are served from the host's own origin: `mount.ts:66` builds the worker
      URL from `import.meta.url`, and a Worker must be same-origin. The service's `/embed/` is therefore
      reverse-proxied under the host's origin.
    - The CSP needs `script-src 'wasm-unsafe-eval'` and `worker-src 'self'`.
    - COOP/COEP are optional. Without them the motor runs on one thread.
    - Row `studio-embed` includes one run without the cross-origin-isolation headers.

## As built: where the code departs from a condition (2026-10-04)

Each item was forced by the implementation or by a measurement. Evidence is in the file named.

- **4.** No `via:"card"`. The hover card is `aria-hidden` and holds no control, so the card's way to
  open a node is the inspector's Open button, `via:"inspector"`.
- **7, the supersede.** Each `apply` carries a generation token (`studio/pipeline.ts`, `Rig.generation`),
  checked at every commit that writes the drawing: after `client.load`, before `draw`, after
  `client.analysis`, and once more before the call reports its outcome. The decision is the token's own,
  not the motor's `busy()`/`cancel()`: `busy()` is false while the worker starts and while the studio
  patches and draws, so a `loadGraph` made from inside a `graph-load` handler re-entered `apply` with a
  graph already on screen and nothing to cancel. Row `host-supersede.test.ts` makes that call.
  - The answer is given up one microtask after the frame is set, because that re-entrant call reaches
    `apply` on the next turn of the queue and not inside the drawing commit.
  - Two `graph-load` events can be heard in that case, one per graph that reached a frame: the studio
    cannot un-announce the frame it had already committed when the handler ran. The superseded call
    still rejects with `CancelledError` and emits no `graph-error`. Row `embed-overlap-reentrant`
    measures both halves; its break is the regression injected as the `supersede` fault.
- **7, the name.** `graph-error.detail.error` is `ShownError.code ?? ShownError.title`
  (`host/contract.ts`, `wireError`), and the rejection's `name` is the same string. `code` is null for
  a loader failure, so the error's name stands in.
  - The SDK's error classes now write their `name` as a literal. With `new.target.name`, the production
    build reported a refused wasm load to its host as `f` (studio-embed break runs, 2026-10-04).
    `crates/graph-sdk-js/test/error-names.test.mjs` renames each class to check this.
- **7, the cap.** The cap is the studio's `MAX_DOCUMENT_CHARS`, 2^28 UTF-16 code units
  (`source/limits.ts`), not the motor's 1 GiB. V8 builds no string past 2^29 characters, so a
  serialised document never reaches 1 GiB.
- **7, other rejections.**
  - A call on a disconnected element rejects with a `DOMException` named `InvalidStateError`.
  - A non-object rejects with a `TypeError`, and so does one that serialises to nothing.
  - A document `JSON.stringify` throws on (a cycle, or a `toJSON` that refuses, or a document past
    V8's string ceiling) rejects as `IngestRefusal` and emits one `graph-error` carrying that same
    name: the studio never saw the document, so nothing else would have been announced.
  - None of these three emits a `graph-error` except the ingest refusal above, and a `CancelledError`
    never does.
  - Loading the document already on screen resolves without a second `graph-load`.
- **3.** An element without `remember` draws nothing until the host's first `loadGraph`. Row
  `embed-storage` is stricter than the condition: it finds no `graph-studio.*` key at all.
- **10.** `graph-error` also fires for a refused command, the same entry the console shows.
- **11.** The Tab check is part of `embed-open` and has no break of its own. Under `break-open`, Tab
  still reached the button after 3 presses.
- **13.** The shadow root's styles are an adopted `CSSStyleSheet` (`mount.ts`, `shadowOf`), not a
  `<style>` element. Under the CSP above, Chromium refused the `<style>` (run `csp`), so a host needs
  no `style-src 'unsafe-inline'`.
- `focusNode` and `selectNodes` return a promise that settles when the command has run, and its answer
  is what the command said: the exact-id check is made against the frame on screen, and if a
  `loadGraph` replaced that frame while the command was in flight, only `entry.ok` counts. While the
  frame is the one the check was made against, the check is the answer. `hostApi` is 2 for this.
- `selectedIds` is a getter and `watchdogBoundMs` is `@internal`.
