# Job host-api-facts (agent build, docs only: what a host can reach in the studio today)

Why: `<graph-studio>` is to become an embeddable service for a Notion-like host
(`~/goinfre/orch/handoff/service-dod.md` step 2). The host contract `docs/contract/host-api.md` will
be written from facts, not from memory. This job gathers those facts. It writes no code.

The contract will cover:
- `load(columns)` and `applyDeltas(batch)`;
- DOM `CustomEvent`s that carry node ids: `node-select`, `node-open` and `node-hover`;
- `focus(id)`;
- an optional host `resolve(id) => Promise<preview>`.

Nodes are references: a string id plus light metadata (label, kind, group, database_id, path,
has_note, icon, tags). Content never enters the motor, the worker or the renderer.

Facts (develop 5fd5d6e9; re-check each on the tree you start from):
- `packages/graph-studio/src/element.ts:43-64` `GraphStudioElement` exposes `studio`, `view`,
  `stopMotor()` and `watchdogBoundMs`. `element.ts:217-218` defines the element once per tag.
- `element.ts` dispatches no event: `git grep -n 'dispatchEvent\|CustomEvent' packages/graph-studio/src`
  finds nothing in `element.ts`.
- `element.ts:32-41` `StudioElementOptions` holds `spawn`, `save`, `backend` and `threads`.
- Every user action is registered once in `packages/graph-studio/src/actions/registry.ts` and called
  through `resolve` (CLAUDE.md "Architecture (studio)").
- The motor runs in a worker behind `packages/graph-studio/src/motor/protocol.ts`.
- The SDK's public surface is `crates/graph-sdk-js/src/index.ts`.

Do, in order. Every row cites `path:line` on the tree you started from, plus the command that found
it (`git grep -n ...`). A row with no `path:line` is not a fact; leave it out or list it under
"Unknown".
1. **Element surface.** List every attribute the element reads and when it reads it. Then every
   property, every method and every lifecycle callback, and what each one touches. Then
   `StudioElementOptions` and the function that registers the tag.
2. **`Studio` surface.** List every member of the `Studio` interface (`src/studio/studio.ts`) with its
   type. Mark each one as state (a store, a getter) or a verb.
3. **Host reaches.** Find every place outside `packages/graph-studio/src` that reads or writes
   `element.studio`, `studio.store`, `element.view` or any studio internal. Search `app/`,
   `harness/`, `scripts/studio-*.sh` and the browser gate scripts they run. Name what each reach
   reads or writes and why. These are the reaches the contract has to replace or bless.
4. **Actions.** Give one row per registered action: id, arguments, what it changes, `path:line`.
5. **Load path.** Trace how a graph gets in today, from the `fixtures` attribute (or whatever starts
   a load) through the worker protocol to the SDK `build*` call. List every message type in
   `protocol.ts` with its direction and payload.
6. **Node identity.** Find where the studio and the renderer go from a dense index to a string id and
   back. Name the column or table that holds the ids. Say whether the renderer ever sees an id.
7. **Selection, hover, open, focus.**
   - Where the selected node(s) and the hovered node are stored; give the store field.
   - What gesture opens a node today, if any (a double-click, Enter).
   - Whether a "move the camera to a node" function exists (`view.ts`, the camera); give its
     `path:line`.
   - What the hover card or tooltip shows, and where its text comes from.
8. **Deltas.** Find every SDK and studio entry point that changes a loaded graph without a full
   rebuild: the force session's methods, `applyDeltas` if present, and a re-ingest. Cite
   `docs/contract/delta.md` and `docs/decisions/delta-abi.md` if they exist on your tree.
9. **SDK exports.** One row per name `crates/graph-sdk-js/src/index.ts` exports (or re-exports):
   kind (class, function, type, error) and `path:line` of the definition.
10. **Packaging today.** How `app/` builds and serves the studio:
    - the vite config;
    - the wasm artifacts and how they are staged (`scripts/studio.sh wasm`);
    - whether a COOP/COEP header is set, and where;
    - what the worker URL resolution assumes.
11. **Unknown.** List every question from steps 1-10 that the code did not answer.

Write it all to `docs/reports/host-api-facts.md`: one section per step above, each a table
`| # | Fact | path:line | Found by |`. No prose beyond one line per section. No recommendations:
the contract is written elsewhere.

You may run `git grep`, `sed -n`, `--help` commands and read any file. Do not run builds, browsers,
benches or gates other than the one below.

Paths you may touch: `docs/reports/host-api-facts.md`. Nothing else.

Done when:
- `scripts/orch/gate.sh target/rows-host-api-facts scripts/orch/rows/docs.rows` writes a
  `summary.txt` with every row PASS.
- Every `path:line` in the report resolves to a non-empty line, checked by this command (expect no
  output):
  `grep -oE '[A-Za-z0-9_./-]+\.(ts|tsx|rs|md|sh|mjs|json):[0-9]+' docs/reports/host-api-facts.md | sort -u | while IFS=: read -r f l; do [ -n "$(sed -n "${l}p" "$f" 2>/dev/null)" ] || echo "BAD $f:$l"; done`
- Negative control: append `packages/graph-studio/src/element.ts:99999` to a scratch copy of the
  report, run the same command on the copy, and confirm that it prints `BAD`. Do not commit the copy.

Return: the branch tip, the row count per section, the Unknown list, and every deviation from this
brief.
