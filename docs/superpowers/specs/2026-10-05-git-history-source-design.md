# Git history through graph-render: the git plugin (piece 1 of 3)

Status: draft, 2026-10-05.
- Direction approved by the user on 2026-10-05: "B and A". That means use the existing
  `layout.dag.sugiyama` now (A), and add `layout.dag.lanes` for full histories (B,
  `2026-10-05-dag-lanes-layout-design.md`). The user granted autonomy on the same day.
- Integration points agreed with the hub owner (session graph-render-0e) on 2026-10-05:
  `examples/plugins/git/`, the ingest document first, `sync` once `createPlugin` lands, and no
  studio→hub read.

## What the user asked for

- See the commits, branches and merges of any repository through graph-render.
- Interactive and clickable, with rules users set, and "incredibly fast".
- The test data is ActivityWatch's repositories and this one.

**Hard rule (user, 2026-10-05).** The motor, the SDK and the studio do not know that this plugin
or any other project exists. They give generic tools. The connection is written outside them,
on those tools. If a tool turns out to be missing, the fix is a generic tool, never git-aware
engine code.

Assumptions, open to correction:
- "the diagram of the Japanese guy with the edge routes" is Sugiyama's layered layout.
- "the other repo" is ActivityWatch: contributor-stats, activitywatch, aw-server-rust.
- contributor-stats is prior art for *what* is shown (per-author tables, a Gource video). It is
  not prior art for *how*: it has no commit graph.

## The three pieces

| # | Piece | Where | Depends on |
|---|---|---|---|
| 1 | The git plugin: `git log` → ingest records → a document the studio opens | `examples/plugins/git/` | nothing; hub push once SDK Task 8 lands |
| 2 | `layout.dag.lanes`: one row per vertex, reused lanes, every edge routed | `crates/graph-core` (generic) | nothing |
| 3 | Interaction: commit panel, ancestry highlight, date-range rules, click triggers, each as a generic studio feature | `packages/graph-studio` | 1 |

## Measured baseline (throwaway probe, `target/gitviz-probe/`, not kept)

Each commit DAG went through the SDK's provisional `build` and the wasm motor, under Node 22 in
`node-slim`. Host load was 15–17 because a peer's full gate was running, so the timings are
single runs and only show orders of magnitude.

| repository | commits | edges | build | `dag.sugiyama` | unrouted arcs (note 4) |
|---|---|---|---|---|---|
| ActivityWatch/contributor-stats | 488 | 490 | 8 ms | 4 ms | 0 |
| ActivityWatch/aw-server-rust | 989 | 1,014 | 6 ms | 26 ms | 0 |
| ActivityWatch/activitywatch | 1,271 | 1,356 | 7 ms | 57 ms | 0 |
| Univers42/graph_render | 1,971 | 2,589 | 9 ms | 315 ms | 0 |
| git/git, first 10k | 10,000 | 11,428 | 83 ms | 120 ms | 5 |
| git/git, first 30k | 30,000 | 36,224 | 233 ms | 98 ms | 1,120 |
| git/git | 85,928 | 107,694 | 694 ms | 171 ms | 12,035 |

- **What the table shows.** Sugiyama draws every edge up to about 5k commits. Past its dummy
  budget it stays fast only by leaving long arcs straight. On git/git that is 11% of the edges.
  That is why piece 2 exists.

## Design

### The plugin is a client, nothing more

`examples/plugins/git/` sits beside the hub spec's `examples/plugins/rows-file/` (hub spec §7).
It is outside `crates/`, `packages/` and `app/`, and outside the gate fingerprint.

```
git log (host git) ──► records (one collection: commit) ──► SDK rowsToIngest ──► ingest document
                                                                          │
                       now:   graph-cli ingest --member ingest ──► graph document ──► studio "open document"
                       later: SDK createPlugin + sync ──► graph-hub workspace
```

The plugin uses only public tools:
- the SDK's rows adapter (`crates/graph-sdk-js/src/adapters/rows.ts`), which maps rows to the
  ingest contract;
- the motor's one derivation (`graph-cli ingest`), which turns the contract into the graph;
- the studio's open-document action;
- later, `createPlugin` and `sync` (hub SDK Task 8).

It adds nothing to any of them.

### The `git log` call

`examples/plugins/git/git-log.sh <repo> [<name>]` runs, on the host's git:

```sh
git -C <repo> log --all --topo-order --format='%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s'
```

- One commit per line, six fields separated by `\x1f`. `%s` is the subject's first line, so a
  record never spans two lines.
- `<name>` defaults to the repository directory's basename. It becomes the ingest `source`, so
  two repositories never share an id.

### The mapping: one table, `commit`

| git | rows column (role) | derived by the motor |
|---|---|---|
| `%H` | the row `id` | node id `<name>:commit:<%H>` |
| `%s`, cut at 120 code points (never inside a surrogate pair) | `subject` (`title`) | `label` |
| `%an` | `author` (`group`) | `group`, so colour-by-group and `filter.group` work per author |
| `%at` | the row's `updatedAt` (u32 seconds) | `version`, which `layout.dag.lanes` breaks ties on (newest first) |
| `%P` | `parents` (`link`, `collection: commit`, `cardinality: many`, `symmetric: false`) | one directed `relation` edge per parent, child → parent, in `%P` order |
| `%D` refs, `HEAD -> ` and `tag: ` stripped, plus `merge` (two or more parents) and `root` (none) | `refs` (`scalar`) | nothing in the motor; `decorate.mjs` copies it onto the node's `tags`, so `tag:#develop` and `tag:#merge` queries work |

- **Missing parents.** A parent absent from the log (a shallow clone, a path-limited log) is
  dropped from `parents` before the adapter runs, because the contract refuses a dangling link
  (`check_link_cells`). The drop count is printed on stderr. A silent drop would misstate the
  input.
- **Refused input.** Each refusal names the line and the plugin exits 2:
  - a record without six fields;
  - a hash that is not 40 or 64 hex digits;
  - a non-integer `%at`;
  - a duplicate commit.
- **Why `refs` is not a `tags` column** (amended 2026-10-05, while writing the plan).
  - A `tags` column derives one hub node per distinct value, with one edge per tagged record.
  - In a history drawing every hub is a vertex too. `tag:merge` would join every merge (about
    10k on git/git), and each distinct ref would hold a lane open down to its hub.
  - The studio's `tag:#x` query reads the node's own `tags` member
    (`packages/graph-studio/src/source/ingest.ts`). So the plugin keeps `refs` as a `scalar`
    cell, which the contract carries and derives nothing from, and `decorate.mjs` copies it
    onto each node.

### Opening it, and the layout

- `examples/plugins/git/run.sh <repo> [<name>]` chains the steps:
  - `git log`;
  - `export.mjs` (rows → contract);
  - `graph-cli ingest`;
  - `decorate.mjs`.

  It prints the path of `<name>.studio.json`, which the user opens with the studio's open-file
  action.
- The studio opens node/edge JSON, not a contract document. That is why `graph-cli ingest` and
  `decorate.mjs` sit in the pipeline. A generic studio feature that opens a contract would remove
  both, and it is a piece 3 candidate.
- The plugin names no layout, and nor does the studio (`CLAUDE.md`). The picker lists
  `layout.dag.sugiyama` (and `layout.dag.lanes` after piece 2), and the settings persist the
  choice.

### Rules available on the first day, with no new code

| rule | how |
|---|---|
| colour commits by author | `appearance.colour by group` |
| one author only | `filter.group <author>` |
| one branch's tip, all tags, all merges | `filter.query tag:#develop`, `tag:#merge` (node `tags`, written by `decorate.mjs`) |
| one repository among several | `filter.query db:...` where the derivation sets one, else `id:` prefix |
| branches coloured | `groups.add` with `tag:#<branch>` |
| click → open the commit | `node.open` hands the host the node id. The host, not the studio, maps it to a URL |

### Later: the hub

Once `createPlugin` lands (hub-sdk Task 8):
- The same records go through `sync`.
- The manifest is the `commit` collection above.
- A batch that crosses `GRAPH_HUB_MAX_PLUGIN_BYTES` (16 MiB default) gets a 413, and the plugin
  reports it with the plugin's byte count. git/git's 86k commits are about 17 MB of records, so
  that is a measured finding for `docs/measurements/`, not a limit to change.
- The hub materializes in qualified-id order, which is why piece 2 orders rows from the edges,
  not from admission.

## Speed

| target | measured by |
|---|---|
| git/git (85,928 commits): `git log` + mapping + `rowsToIngest` + `graph-cli ingest` ≤ 3 s end to end | `examples/plugins/git/bench.sh`, three rounds, medians, load printed |
| graph_render's history opened in the studio: first frame ≤ 300 ms after the file is read, with `layout.dag.sugiyama` | a pw MCP session: console, store error, overlay text and a screenshot read. Never the title alone |

Numbers go to `docs/measurements/git-plugin.md`.

> Caveat: the host is shared, so a median under load is an upper bound.

## Testing

- **Unit tests** (`node:test` through `node-slim.sh`). The mapping runs over a hand-written
  `examples/plugins/git/test/small.gitlog` against an expected ingest document. The fixture
  contains:
  - a root;
  - a two-parent merge and an octopus (three-parent) merge;
  - a parent outside the log;
  - refs in the `HEAD ->` and `tag:` forms;
  - a non-ASCII subject.
- **Negative controls.** Every refusal above has a case that must exit 2 and name its line.
- **Convergence.** The expected ingest document passes `graph-cli ingest --check`, so the
  motor's reader accepts what the plugin writes.

## What it does not do

- No diffs, file trees or blame. Only the commit graph and its metadata.
- No live watching. A refresh is a new run.
- No studio code and no SDK code. Missing generic features (date-range queries, an inspector
  panel) are piece 3.
- It does not fix Sugiyama's drawing past about 5k commits. That is piece 2.
