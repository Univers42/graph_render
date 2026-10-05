# Job studio-open-large (agent build: find and cut where opening a large document goes)

Why: opening a node/edge document of 85,928 nodes and 107,694 edges (62 MB) in the studio took
7.1–10.65 s to first frame. Its `layout.dag.sugiyama` took only 140 ms of that. Opening a 488-node
file right after it took 10.1 s. The tab also crashed ("Target crashed") 4 times in 6 attempts to
open a second document after the large one.

These are one-run numbers, taken by hand on 2026-10-05 in the dev server, through the Playwright
MCP browser (SwiftShader WebGL2). See `docs/measurements/git-plugin.md`, "Studio check". This job
makes them reproducible, finds where the time goes, and cuts the largest cost it can without
changing behaviour.

The hub owner (session graph-render-0e) confirmed on 2026-10-05 that no job of theirs touches the
studio's open path or the legend. Their slice "buildContract in the worker" (the studio opening an
ingest contract) is theirs: do not start it.

Facts (develop, 2026-10-05; re-check each on your branch before editing, and stop if one no longer
holds):
- The document: `examples/plugins/git/run.sh /tmp/gitviz/git.git` writes
  `target/git-plugin/git/git.studio.json`, `{version, nodes, edges}` JSON, in about 5 s.
  - `examples/plugins/git/README.md` is its manual.
  - `/tmp/gitviz/contributor-stats.git` gives a 488-node one the same way.
  - If `/tmp/gitviz` is missing (a reboot), stop and say so.
- The open action is `packages/graph-studio/src/actions/source.ts:48-60`, `source.document`, with
  params `name` and `text`. The full document text is a settings value
  (`state.settings.source.text`, `:57-58`), bounded by `MAX_DOCUMENT_CHARS`
  (`src/source/limits.ts`).
- `packages/graph-studio/src/source/ingest.ts` `normaliseIngest(text, source)` (`:288`) parses it.
  `src/source/meta.ts` `metaOf` builds the columns.
- A new source starts in a new worker (`src/motor/client.ts`, `loadFresh`).
- `deploy/perf/open.py` is the pattern for a probe:
  - CPU profiles of the page and every worker, over `deploy/perf/cdp.py`;
  - first-frame timing;
  - it drives the studio through `deploy/perf/drivers/hook.js`, which dispatches the studio's
    own actions (`window.__perf`).
- `scripts/studio-probe.sh NAME [ARGS]` runs `deploy/perf/NAME.py` against `app/dist` in
  headless Chromium in Docker. Build first with `scripts/studio.sh build`.

Steps:
1. **The probe.** Write `deploy/perf/open-document.py`, modelled on `open.py`. Args:
   `<document path under the repo> <backend> [<second document path>]`.
   - It opens `app/dist`, selects `layout.dag.sugiyama`, and dispatches `source.document` with the
     document's text. Serving the document is your choice; say how.
   - It prints:
     - the open time;
     - the milliseconds to the first frame after it;
     - `performance.memory.usedJSHeapSize` after it;
     - the profiles' top self and inclusive rows for the page and each worker.
   - With the second document, it then opens that one and prints the same.
   - A crash or a page error is printed and the probe exits 1.
   - Header: what it measures, and a `Caveat:` naming what it gets wrong.
2. **Baseline.** On develop's build, 3 rounds of
   `scripts/studio-probe.sh open-document target/git-plugin/git/git.studio.json webgl2 target/git-plugin/contributor-stats/contributor-stats.studio.json`,
   and the same with `canvas2d`. Print the load before each round. Count the crashes.
3. **Find.** From the profiles, name the three largest costs on the open path and the three on the
   switch to the second document, each with its function and file:line.
4. **Cut.** Fix the largest costs you can inside `packages/graph-studio/src/`. Behaviour stays the
   same: every studio test passes unchanged, except a test that pinned an internal you replaced
   (list it).
   - Library-first: a primitive that exists is reused.
   - No O(n²).
   - A hot loop allocates nothing per node.
   - **Stop before code** if the fix changes a message shape in `src/motor/protocol.ts`, the
     worker lifecycle in `src/motor/client.ts`, or what `settings.source` holds or persists.
     Instead, write the change you propose, its risk and its measured gain under "decisions
     needed". Those need a risk verdict before code (house rule).
5. **Crash.** If step 2 reproduces a crash, find its cause: memory per process, the
   `performance.memory` trend across the two opens, and what still holds the first document.
   Fix it if it falls under step 4's paths and rules; otherwise report it.
6. **After.** The same 3 + 3 rounds on your branch's build, alternating with develop's where you
   can. Claim medians only, and say the host was loaded.
7. **Report.** Write `docs/measurements/studio-open-large.md` with:
   - the commands;
   - the before/after table (open ms, first frame ms, switch ms, heap MB, crashes; per backend);
   - the profile rows that justify each cut;
   - what you did not cut, and why;
   - a `Caveat:` line.

Rules (beyond `scripts/orch/common.md`):
- Studio code: no type assertions (`as`), no `any`, no `eslint-disable`.
- `scripts/studio.sh` is the only way to type-check, lint, test and build. Node only through
  `scripts/orch/node-slim.sh`.
- Never take `~/goinfre/orch/timed.lock`. Probes never take the host gate lock. Run one probe at a
  time.
- Paths you may touch:
  - `packages/graph-studio/src/**` and `packages/graph-studio/tests/**`;
  - `deploy/perf/open-document.py`;
  - `docs/measurements/studio-open-large.md`.

  Nothing else: not `packages/graph-render`, `crates/`, `app/`, `server/`, `src/` or
  `examples/`. `scripts/orch/rows/studio-open-large.rows` is already on develop: do not edit it.

Done when:
- `scripts/orch/gate.sh target/rows-studio-open-large scripts/orch/rows/studio-open-large.rows`
  writes a `summary.txt` with every row PASS;
- `docs/measurements/studio-open-large.md` holds the before/after table.

Return:
- the branch tip;
- the files changed, with line counts;
- the before/after medians per backend;
- the three costs found for the open and the three for the switch, with what you did to each;
- the crash finding;
- each row's result;
- decisions needed;
- every deviation.
