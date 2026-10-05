# git plugin

A repository's commit graph as a studio document, using only public tools: the SDK's rows
adapter, `graph-cli ingest`, and the studio's open-file action.

```
git-log.sh  ->  map.mjs (parseLog, toRows)  ->  SDK rowsToIngest  ->  {"ingest": contract}
            ->  graph-cli ingest --member ingest (the motor's one derivation)  ->  graph
            ->  decorate.mjs (a contract field copied onto node `tags`)  ->  studio document
```

The engine (motor, SDK, studio) does not know this plugin exists. It lives here, in
`examples/plugins/`, beside `rows-file/`, and adds nothing to any product package.

## Run it

```sh
examples/plugins/git/run.sh /tmp/gitviz/contributor-stats.git
```

It prints the path it wrote:

```
target/git-plugin/contributor-stats/contributor-stats.studio.json
```

The second argument names the ingest source and defaults to the repository directory's name without
`.git`. Work files (`log`, `ingest.json`, `graph.json`) go under `target/git-plugin/<name>/`.

## Open it

In the studio's **Source** section, the **Open a file** action (`source.document`) reads a
`{version, nodes, edges}` JSON document — which is what `run.sh` prints the path of. Point it at
`contributor-stats.studio.json`; the 488 commits appear as nodes, each parent as a directed edge.

## What you can do with no new code

| rule | how |
|---|---|
| colour commits by author | `appearance.colour by group` |
| one author only | `filter.group <author>` |
| one branch's tip, all tags, all merges | `filter.query tag:#develop`, `tag:#merge` (node `tags`, written by `decorate.mjs`) |
| one repository among several | `filter.query db:...` where the derivation sets one, else `id:` prefix |
| branches coloured | `groups.add` with `tag:#<branch>` |
| click → open the commit | `node.open` hands the host the node id. The host, not the studio, maps it to a URL |

The `tag:#x` queries work because `refs` is a `scalar` column in the contract — the motor derives
no hub node from it — and `decorate.mjs` copies the cell onto each node's `tags`, the member the
studio's query reads (`packages/graph-studio/src/source/ingest.ts`).

## What it does not do

- No diffs, file trees or blame. Only the commit graph and its metadata.
- No live watching. A refresh is a new run.
- No studio code and no SDK code. Missing generic features (date-range queries, an inspector
  panel) are piece 3.
- It does not fix Sugiyama's drawing past about 5k commits. That is piece 2, `layout.dag.lanes`.

## Caveats the code carries

- A parent outside the log (a shallow clone, `--first-parent`, a path-limited log) is dropped from
  `parents` and counted on stderr, because the contract refuses a dangling link. A silent drop would
  misstate the input.
- A ref literally named `merge` or `root` is indistinguishable from the synthetic tag of the same
  name; the set keeps one.
- `decorate.mjs` parses the first line of the file `graph-cli ingest --out` wrote, because that
  file is the canonical JSON followed by the CLI's readable rendering. See
  `docs/measurements/git-plugin.md` for the byte cost of that rendering.

## Tests and measurements

```sh
scripts/orch/node-slim.sh node --experimental-strip-types --test "examples/plugins/git/test/**/*.test.mjs"
scripts/orch/gate.sh target/rows-git-plugin scripts/orch/rows/git-plugin.rows
```

Numbers, the ≤ 3 s end-to-end target on git/git and the findings are in
`docs/measurements/git-plugin.md`.