# git-plugin measurements

Landing: `examples/plugins/git/`, 2026-10-05. Every number below was produced on this worktree by
the commands quoted with it. Host: 31 GB RAM, `/mnt` NVMe.

> Caveat (from `examples/plugins/git/bench.mjs`): wall clock on a shared host; a median under load
> is an upper bound. Container start-up (node-slim, gr) is outside every bench number and is
> reported apart, below.

## Commands

```sh
mkdir -p target/git-plugin/logs
for r in contributor-stats activitywatch aw-server-rust git; do
  examples/plugins/git/git-log.sh /tmp/gitviz/$r.git > target/git-plugin/logs/$r.log
done
examples/plugins/git/git-log.sh . > target/git-plugin/logs/graph_render.log
/usr/bin/time -f '%e s' examples/plugins/git/git-log.sh /tmp/gitviz/git.git > /dev/null   # three times
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo build -q -p graph-wasm --release --target wasm32-unknown-unknown
flock ~/goinfre/orch/bench.lock scripts/orch/node-slim.sh node --experimental-strip-types \
  examples/plugins/git/bench.mjs target/wasm32-unknown-unknown/release/graph_wasm.wasm \
  target/git-plugin/logs/{contributor-stats,aw-server-rust,activitywatch,graph_render,git}.log
/usr/bin/time -f '%e s' examples/plugins/git/run.sh /tmp/gitviz/git.git                     # three times
```

Preconditions before the `flock` line, checked and met: no `develop-full.rows` gate running
(`pgrep -f develop-full.rows` → none), `free -g` available 20 GB (≥ 12), 1-minute load 2.08 (< 14).

## Bench: the in-process steps (medians of 3 rounds, load printed)

`parse` is `map.mjs`'s `parseLog`; `map` is `toRows` + the SDK's `rowsToIngest`; `stringify` is
`JSON.stringify`; `build` is the wasm motor's `buildContract`, the same derivation as
`graph-cli ingest`. All ms.

| repository | commits | contract bytes | parse | map | stringify | build | `layout.dag.sugiyama` | notes (code: count) |
|---|---|---|---|---|---|---|---|---|
| contributor-stats | 488 | 131,530 | 0.8 | 2.0 | 0.3 | 2.9 | 1.0 | `{}` |
| aw-server-rust | 989 | 274,355 | 1.6 | 1.4 | 0.8 | 5.1 | 3.7 | `{}` |
| activitywatch | 1,271 | 354,609 | 3.0 | 1.0 | 1.0 | 6.3 | 27.8 | `{}` |
| graph_render (this repo) | 2,051 | 500,262 | 2.1 | 2.0 | 0.9 | 9.6 | 196.9 | `{}` |
| git/git | 85,928 | 24,497,318 | 121.6 | 98.7 | 85.3 | 534.1 | 104.0 | `{"4":11310}` |

Load at every row: 1.91 (1-minute average of `/proc/loadavg` in the container).

`layout.dag.lanes` is **not registered** in this build, so the bench ran `layout.dag.sugiyama`
alone. The bench asks `motor.layouts()` and filters, so it picks the lane layout up the day it
lands, with no change here.

## git's own time, and the end-to-end target

| what | rounds (s) | median |
|---|---|---|
| `git log --all --topo-order` on git/git | 0.60, 0.60, 0.61 | **0.60 s** |
| `run.sh /tmp/gitviz/git.git`, end to end | 4.19, 4.44, 3.85 | **4.19 s** |

Load at the `run.sh` rounds: 3.55–3.70.

**Target "git/git end to end ≤ 3 s": MISSED. Median 4.19 s.** Not rounded into a pass.

Where the 4.19 s goes, each part measured once on the warm tree (load 3.55):

| stage | measured | container start-up inside it |
|---|---|---|
| `git-log.sh` (host git) | 0.60 s | 0 |
| `export.mjs` (log → `{"ingest": …}`) | 0.73 s | 0.24 s |
| `graph-cli ingest --out graph.json` | 1.58 s | 0.23 s |
| `decorate.mjs` (graph → studio document) | 1.04 s | 0.24 s |
| shell, `mkdir`, `echo` | ~0.2 s | 0 |

- **Container start-up, apart: 0.71 s** (two `node-slim.sh` at 0.24 s, one `gr` at 0.23 s, medians
  of 3 each). Small against the total; the target is not lost to Docker.
- **In-process sum, apart: 0.84 s** (parse 121.6 + map 98.7 + stringify 85.3 + build 534.1 ms from
  the bench table), plus 104 ms for `layout.dag.sugiyama` if the studio draws it.
- The remaining ~2.5 s is byte movement, not computation: the pipeline writes 14.3 MB of log,
  24.5 MB of contract, 107 MB of graph and 62 MB of studio document, and reads most of it twice.

## Findings

- **The hub cap is crossed by git/git's contract.** 24,497,318 bytes of records against
  `GRAPH_HUB_MAX_PLUGIN_BYTES`' 16 MiB (16,777,216) default: **over by 7,720,102 bytes (46 %)**.
  graph_render is 500,262 bytes, so every smaller history fits with 33× headroom. This is a
  measured fact about git/git's size, not a cap to raise: 86k commits at ~285 bytes each is
  arithmetic, and the spec already names the report as the outcome (2026-10-05 design, "Later: the
  hub").
- **`graph-cli ingest --out` does not write one JSON document.** It writes the canonical JSON, a
  newline, then the readable `describe` rendering
  (`crates/graph-cli/src/ingest_cmd.rs:96-100`). On git/git that is 61,029,255 bytes of JSON plus
  **45,906,103 bytes (43 % of the file) of human-readable rendering** — 106,935,359 bytes for a
  62 MB document. `decorate.mjs` therefore parses the first line only, with the `Caveat:` that
  says why that is sound. A `--json-only` flag, or the rendering behind a second flag, would
  remove the need for that rule and nearly halve the pipeline's largest write. Generic tool gap,
  engine-side.
- **Missing generic tools: the studio opens node/edge JSON, not a contract.** That is the whole
  reason `graph-cli ingest` and `decorate.mjs` sit in the pipeline. A studio feature that opens an
  ingest contract and derives it would delete both steps, and with them 2.6 s of the 4.19 s
  above. Plugin code cannot substitute for it: the plugin must not teach the engine about itself.
- **Note 4 (unrouted arcs) counts.** `layout.dag.sugiyama`: contributor-stats 0, aw-server-rust 0,
  activitywatch 0, graph_render 0, git/git **11,310** — 10.5 % of its 107,694 edges. Sugiyama draws
  every edge up to about 5k commits; past its dummy budget it stays fast by leaving long arcs
  straight. The spec's throwaway probe counted 12,035 on git/git through the SDK's provisional
  `build`; this run goes through the contract, so the two counts are not the same measurement.
- **`graph-cli ingest` is the slowest single step (1.58 s) and is 60 % byte movement**, not
  derivation: the wasm `buildContract` of the same document is 534 ms.

## Gate

`scripts/orch/gate.sh target/rows-git-plugin scripts/orch/rows/git-plugin.rows` → 3 PASS,
2 FAIL. Both FAILs are defects in the rows file, outside this landing's paths; each is reproducible
with no plugin file changed:

- `plugin-test` — `node --experimental-strip-types --test examples/plugins/git/test/` exits 1 with
  `Error: Cannot find module '/w/examples/plugins/git/test'`. Node 22.23.3 does not expand a bare
  directory given to `--test`: it loads the path as a module. The same happens for an existing,
  passing SDK directory (`crates/graph-sdk-js/test`), and the repo's own rows never pass a bare
  directory (`develop-full.rows:146` uses a quoted glob). The identical tests pass as
  `--test "examples/plugins/git/test/**/*.test.mjs"`: 11 tests, 11 pass, exit 0.
- `negctl-converges` — the row's own `node -e` does `JSON.parse(readFileSync("…/small.graph.json"))`,
  which throws `SyntaxError: Unexpected non-whitespace character after JSON at position 4388` on the
  `describe` section described above, so it never writes the broken document and the `--check`
  that follows exits 2 (unreadable) where the row wants 1. This row and `plugin-converges` cannot
  both hold for one file: `--check` compares the whole produced text, canonical JSON *and*
  rendering, so a `small.graph.json` that `JSON.parse` accepts is by construction STALE.
Fixed on develop (800deeab): `plugin-test` takes the quoted glob `"examples/plugins/git/test/*.test.mjs"`
(11 tests, 11 pass), and `negctl-converges` parses the first line only, pops a node, writes the rest
back unchanged and expects `--check` to say `STALE` (exit 1). Rerun on the branch merged with that
develop: `scripts/orch/gate.sh target/rows-git-plugin-merged scripts/orch/rows/git-plugin.rows` →
5 PASS.

## Studio check, by hand (2026-10-05)

The documents `run.sh` wrote, opened in the studio's dev server (`STUDIO_PORT=5175 scripts/studio.sh`,
branch studio-query-fields, which adds `group:` and `version:`) through the Playwright browser MCP
(headless Chromium in `gm-mcp-browser`, WebGL2 on SwiftShader), layout `dag.sugiyama`. Timed from the
file input's `change` to the status line naming the new node count, one run each, host load 2.8–3.3:

| document | nodes · edges | open → first frame | layout | notes |
|---|---|---|---|---|
| contributor-stats.studio.json (347 KB) | 488 · 490 | under 1 s | 1 ms | no console error or warning |
| git.studio.json (62 MB) | 85,928 · 107,694 | 10.65 s, 7.48 s, 7.10 s | 138–148 ms | 4 WebGL `GPU stall due to ReadPixels` warnings, no error |
| contributor-stats, opened right after git | 488 · 490 | **10.1 s** | 1 ms | |

- Queries work on the plugin's members: the Filters panel query `tag:#merge OR tag:#root` keeps
  the 4 nodes the document tags (3 merges, 1 root); `group:"erik bjäreholt" AND version:<1700000000`
  keeps one author's older commits. The search box finds labels only, by design (`ui/Search.tsx`).
- **Finding: the studio, not the plugin, is the slow part on git/git.** The layout is 140 ms; the rest
  of the 7–10 s is the open path. Leaving the big document costs as much again (10.1 s for a 488-node
  file).
- **Finding: the tab crashed** ("Target crashed") 4 times in 6 attempts to open a second document
  after git.studio.json, once with no profiler attached. Not yet separated from the headless
  SwiftShader setup; recorded as unexplained, not as a studio defect.
- **Finding: the legend mislabels counts past its palette.** git/git has 2,486 authors. The legend
  lists ten rows such as "Junio C Hamano 34254", "Ilia K 10348" and "Julia Evans 6774"; the document
  holds 31,716, 1 and 38 commits for those names (`Counter(node["group"])` over git.studio.json). The
  numbers read like per-colour totals labelled with one name each. Not investigated further here.

Caveat: one run per row on a shared host, in the dev server (unminified, React development
build); a production build is faster by an unmeasured factor, so these are upper bounds.

Superseded for the production build by `docs/measurements/studio-open-large.md` (2026-10-05,
`scripts/studio-probe.sh open-document`, app/dist, medians of 5): the same 62 MB document opens in
1.45–1.52 s, switching to the 488-node one takes about 53 ms, and 40 opens gave no crash. The dev
server numbers above are the development build's cost, not the studio's.
