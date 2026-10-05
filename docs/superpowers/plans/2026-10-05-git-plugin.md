# Git plugin (`examples/plugins/git`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn any repository's `git log` into a file the studio opens:
- commits as nodes;
- parent links as directed edges;
- authors as groups;
- refs as node tags.

It is built only on public tools: the SDK's rows adapter, `graph-cli ingest` and the studio's
document format. Its speed is measured on real histories.

**Architecture:** A client outside every product package. The pipeline is:

```
git-log.sh  ->  map.mjs (parseLog, toRows)  ->  SDK rowsToIngest  ->  {"ingest": contract}
            ->  graph-cli ingest --member ingest (the motor's one derivation)  ->  graph
            ->  decorate.mjs (a contract field copied onto node `tags`)  ->  studio document
```

`run.sh` chains the steps, and `bench.mjs` times the in-process steps through the wasm SDK.

**Tech Stack:**
- Node 22 with `--experimental-strip-types`, through `scripts/orch/node-slim.sh`;
- `node:test`;
- host `git`;
- `scripts/orch/gr` for `graph-cli`.

**Spec:** `docs/superpowers/specs/2026-10-05-git-history-source-design.md`. Read it first. This
plan amends one row of it: `refs` is a `scalar` column, not `tags`. The spec is updated in the
same landing as this plan, and gives the reason:
- tag hubs would add one lane per distinct ref, and a hub whose degree is the number of merges;
- the studio's `tag:#x` reads node `tags`, which `decorate.mjs` writes.

## Global Constraints

- **Nothing outside `examples/plugins/git/` and `docs/measurements/git-plugin.md` changes.** No file under `crates/`, `packages/`, `app/`,
  `src/`, `server/`, `harness/` or `fixtures/`. The rows file's `no-engine` row checks it.
  - The engine must not learn this plugin exists (user rule, 2026-10-05).
  - A missing generic tool is a finding written in the measurement doc, never a workaround in
    the engine.
- Node only through `scripts/orch/node-slim.sh` (it mounts the git top-level at `/w`, so every
  path you pass is relative to the worktree root). No host `node`/`npm`. No new npm dependency.
- `graph-cli` only through `CARGO_BUILD_JOBS=3 scripts/orch/gr cargo run -q --release -p graph-cli -- ...`.
- House limits:
  - ≤ 40 lines a function, ≤ 300 lines a file, ≤ 4 parameters;
  - every heuristic carries a `Caveat:` line;
  - comments say why, not what.
- The bench may run only under `flock ~/goinfre/orch/bench.lock`, and only once
  `pgrep -f develop-full.rows` finds no gate (apart from opencode), `free -g` shows ≥ 12 GB
  available and the 1-minute load is < 14. Medians of 3 rounds, with load printed.
- Exit codes follow `graph-cli`'s: 0 done, 1 ran and failed, 2 refused or could not run.

## Review Focus

1. **A subject cut at 120 code points with an astral character at the boundary.** The pair
   must stay whole. Pinned by `a_subject_is_cut_at_120_code_points_never_inside_a_pair`.
2. **A parent outside the log** (shallow clone, `--first-parent`, path-limited log). It is
   dropped and counted, and nothing dangles. Pinned by `a_parent_outside_the_log_is_dropped_and_counted`.
3. **An empty subject or author.** The cell is absent, never an empty string, so the motor
   labels the node with its record id. Pinned by `an_empty_subject_or_author_is_an_absent_cell`.
4. **Malformed input:**
   - a line with five or seven fields;
   - an abbreviated hash;
   - a negative or fractional time;
   - a commit printed twice (`--all` with grafts).

   Each exits 2 and names the line. Pinned by `every_refusal_names_its_line` and the
   `negctl-refused` row.
5. **A ref named `merge` or `root`.** It is kept once, because the set dedups. Pinned by
   `synthetic_tags_never_duplicate_a_ref`. The ambiguity carries a `Caveat:`.

---

### Task 1: the mapping (`map.mjs`) and its tests

**Files:**
- Create: `examples/plugins/git/map.mjs`, `examples/plugins/git/test/small.mjs`,
  `examples/plugins/git/test/map.test.mjs`, `examples/plugins/git/package.json`

**Interfaces:**
- Produces:
  - `FORMAT: string`
  - `SUBJECT_MAX: 120`
  - `class LogRefusal extends Error { line: number }`
  - `parseLog(text: string): Commit[]`
  - `refsOf(decoration: string, parentCount: number): string[]`
  - `toRows(commits: Commit[], name: string): { rows: RowsSource, dropped: number }`
  - `COLUMNS`
- `Commit` is `{ hash, parents: string[], author: string | undefined, time: number, refs: string[], subject: string | undefined }`.

- [ ] **Step 1: Write `examples/plugins/git/package.json`**

```json
{
  "name": "@graph-motor/example-git",
  "private": true,
  "type": "module",
  "description": "A repository's commit graph as a studio document, built on the SDK's rows adapter and graph-cli ingest. Knows git; the engine does not.",
  "scripts": {
    "test": "node --experimental-strip-types --test test/"
  }
}
```

- [ ] **Step 2: Write the fixture module `examples/plugins/git/test/small.mjs`**

```js
// A six-commit history, children first as `git log --topo-order` prints it. Covered:
// - an octopus merge (f) and a two-parent merge (e);
// - a parent outside the log (b's second parent, 9…9);
// - a root (a);
// - `HEAD -> ` and `tag: ` refs;
// - a non-ASCII subject (d);
// - a subject past 120 code points whose 120th is astral (c).
export const H = (c) => c.repeat(40);
export const LONG = `${"x".repeat(119)}🚀tail`;
const LINES = [
  [H("f"), `${H("e")} ${H("d")} ${H("c")}`, "Bo", "1700000006", "tag: v1.0", "Octopus merge"],
  [H("e"), `${H("c")} ${H("d")}`, "Ana", "1700000005", "HEAD -> main, origin/main", "Merge branch 'feature'"],
  [H("d"), H("b"), "Bo", "1700000004", "feature", "Ajouté l'accès café ☕"],
  [H("c"), H("b"), "Ana", "1700000003", "", LONG],
  [H("b"), `${H("a")} ${H("9")}`, "Ana", "1700000002", "", "Second"],
  [H("a"), "", "Ana", "1700000001", "", "Initial"],
];
export const SMALL_LOG = `${LINES.map((fields) => fields.join("\x1f")).join("\n")}\n`;
```

- [ ] **Step 3: Write the failing tests `examples/plugins/git/test/map.test.mjs`**

```js
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { COLUMNS, LogRefusal, parseLog, refsOf, toRows } from "../map.mjs";
import { H, LONG, SMALL_LOG } from "./small.mjs";

const row = (rows, hash) => rows.tables[0].rows.find((r) => r.id === hash);

test("the committed small.gitlog is the fixture module's text", () => {
  assert.equal(readFileSync(new URL("small.gitlog", import.meta.url), "utf8"), SMALL_LOG);
});

test("every commit becomes one row, in log order, with its time as updatedAt", () => {
  const { rows } = toRows(parseLog(SMALL_LOG), "small");
  assert.equal(rows.source, "small");
  assert.deepEqual(rows.tables[0].columns, COLUMNS);
  assert.deepEqual(rows.tables[0].rows.map((r) => r.id), ["f", "e", "d", "c", "b", "a"].map(H));
  assert.deepEqual(rows.tables[0].rows.map((r) => r.updatedAt), [6, 5, 4, 3, 2, 1].map((s) => 1700000000 + s));
});

test("merges keep every parent in %P order and carry the merge tag", () => {
  const { rows } = toRows(parseLog(SMALL_LOG), "small");
  assert.deepEqual(row(rows, H("f")).values.parents, [H("e"), H("d"), H("c")]);
  assert.deepEqual(row(rows, H("f")).values.refs, ["v1.0", "merge"]);
  assert.deepEqual(row(rows, H("e")).values.refs, ["main", "origin/main", "merge"]);
  assert.deepEqual(row(rows, H("a")).values.refs, ["root"]);
});

test("a parent outside the log is dropped and counted", () => {
  const { rows, dropped } = toRows(parseLog(SMALL_LOG), "small");
  assert.equal(dropped, 1);
  assert.deepEqual(row(rows, H("b")).values.parents, [H("a")]);
  assert.deepEqual(row(rows, H("b")).values.refs, ["merge"], "git printed two parents");
});

test("a subject is cut at 120 code points, never inside a pair", () => {
  const [, , , c] = parseLog(SMALL_LOG);
  assert.equal(Array.from(c.subject).length, 120);
  assert.ok(c.subject.endsWith("🚀"));
  assert.equal(c.subject, LONG.slice(0, -4));
  assert.equal(parseLog(SMALL_LOG)[2].subject, "Ajouté l'accès café ☕");
});

test("an empty subject or author is an absent cell", () => {
  const log = `${[H("a"), "", "", "1", "", ""].join("\x1f")}\n`;
  const values = toRows(parseLog(log), "s").rows.tables[0].rows[0].values;
  assert.equal(values.subject, undefined);
  assert.equal(values.author, undefined);
});

test("synthetic tags never duplicate a ref", () => {
  assert.deepEqual(refsOf("merge, tag: merge", 2), ["merge"]);
  assert.deepEqual(refsOf("HEAD", 0), ["HEAD", "root"]);
  assert.deepEqual(refsOf("", 1), []);
});

test("every refusal names its line", () => {
  const good = [H("a"), "", "Ana", "1", "", "ok"];
  const cases = [
    [good.slice(0, 5), "expected 6 fields"],
    [[...good, "extra"], "expected 6 fields"],
    [[H("a").slice(0, 12), ...good.slice(1)], "hex hash"],
    [[good[0], "beef", ...good.slice(2)], "hex hash"],
    [[...good.slice(0, 3), "-1", ...good.slice(4)], "u32"],
    [[...good.slice(0, 3), "1.5", ...good.slice(4)], "u32"],
    [[...good.slice(0, 3), "4294967296", ...good.slice(4)], "u32"],
  ];
  for (const [fields, why] of cases) {
    const log = `${good.join("\x1f")}\n${fields.join("\x1f")}\n`.replace(H("a"), H("b"));
    assert.throws(() => parseLog(log), (e) => e instanceof LogRefusal && e.line === 2 && e.message.includes(why), why);
  }
  const twice = `${good.join("\x1f")}\n${good.join("\x1f")}\n`;
  assert.throws(() => parseLog(twice), (e) => e instanceof LogRefusal && e.line === 2 && /twice/.test(e.message));
});
```

The `.replace(H("a"), H("b"))` makes line 1 commit `b…b`, so line 2 fails only for its own
reason and never as a duplicate.

- [ ] **Step 4: Run them to verify they fail**

Run: `scripts/orch/node-slim.sh node --experimental-strip-types --test examples/plugins/git/test/`
Expected: FAIL. `../map.mjs` is not found, and `small.gitlog` is missing.

- [ ] **Step 5: Write `examples/plugins/git/map.mjs`**

```js
// `git log` text (FORMAT) to the rows the SDK's rows adapter maps onto the ingest contract.
// Pure: no I/O, so the tests run it on a string and the bench times it alone.

export const FORMAT = "%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s";
export const SUBJECT_MAX = 120;
const HASH = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/;
const SECONDS = /^\d{1,10}$/;
const U32_MAX = 0xffff_ffff;

// `refs` is a scalar column: tag hubs would add a lane per distinct ref to a history drawing.
// `decorate.mjs` copies it onto node `tags` for the studio's `tag:#x` query instead.
export const COLUMNS = [
  { name: "subject", role: "title" },
  { name: "author", role: "group" },
  { name: "parents", label: "parent", role: "link", link: { collection: "commit", cardinality: "many", symmetric: false } },
  { name: "refs", role: "scalar" },
];

export class LogRefusal extends Error {
  constructor(line, why) {
    super(`line ${line}: ${why}`);
    this.name = "LogRefusal";
    this.line = line;
  }
}

/** Every commit, in log order. A record without six fields, a hash that is not 40 or 64 hex
 *  digits, a time that is not a u32, or a commit seen twice is refused with its line. */
export function parseLog(text) {
  const commits = [];
  const seen = new Set();
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i += 1) {
    if (lines[i] === "") continue;
    const commit = parseLine(lines[i], i + 1);
    if (seen.has(commit.hash)) throw new LogRefusal(i + 1, `commit ${commit.hash} appears twice`);
    seen.add(commit.hash);
    commits.push(commit);
  }
  return commits;
}

function parseLine(line, at) {
  const fields = line.split("\x1f");
  if (fields.length !== 6) throw new LogRefusal(at, `expected 6 fields separated by \\x1f, found ${fields.length}`);
  const [hash, parentText, author, time, decoration, subject] = fields;
  const parents = parentText === "" ? [] : parentText.split(" ");
  for (const id of [hash, ...parents]) {
    if (!HASH.test(id)) throw new LogRefusal(at, `not a 40 or 64 digit hex hash: ${JSON.stringify(id)}`);
  }
  if (!SECONDS.test(time) || Number(time) > U32_MAX) {
    throw new LogRefusal(at, `time is not a u32 count of seconds: ${JSON.stringify(time)}`);
  }
  return {
    hash, parents, author: author === "" ? undefined : author, time: Number(time),
    refs: refsOf(decoration, parents.length), subject: cut(subject),
  };
}

/** `%D` as tag values, `HEAD -> ` and `tag: ` stripped, then `merge` for two or more parents
 *  and `root` for none, counted on `%P` as git printed it (before missing parents drop).
 *  Caveat: a ref literally named `merge` or `root` is indistinguishable from the synthetic
 *  tag; the set keeps one. */
export function refsOf(decoration, parentCount) {
  const refs = decoration === "" ? [] : decoration.split(", ").map(strip);
  if (parentCount >= 2) refs.push("merge");
  if (parentCount === 0) refs.push("root");
  return [...new Set(refs)];
}

function strip(ref) {
  if (ref.startsWith("HEAD -> ")) return ref.slice("HEAD -> ".length);
  if (ref.startsWith("tag: ")) return ref.slice("tag: ".length);
  return ref;
}

// Array.from walks code points, so a surrogate pair is never split. An empty subject is an
// absent cell: the motor then labels the node with its record id.
function cut(subject) {
  if (subject === "") return undefined;
  const points = Array.from(subject);
  return points.length <= SUBJECT_MAX ? subject : points.slice(0, SUBJECT_MAX).join("");
}

/** The rows source named `name`. A parent absent from the log is dropped, because the
 *  contract refuses a dangling link; `dropped` counts them so the caller can say so. */
export function toRows(commits, name) {
  const known = new Set(commits.map((c) => c.hash));
  let dropped = 0;
  const rows = commits.map((c) => {
    const parents = c.parents.filter((p) => known.has(p));
    dropped += c.parents.length - parents.length;
    return { id: c.hash, updatedAt: c.time, values: { subject: c.subject, author: c.author, parents, refs: c.refs } };
  });
  const table = { id: "commit", name: "Commit", titleColumn: "subject", columns: COLUMNS, rows };
  return { rows: { source: name, tables: [table] }, dropped };
}
```

- [ ] **Step 6: Write the committed fixture file**, then run the tests

Run:
- `scripts/orch/node-slim.sh node --input-type=module -e 'import { SMALL_LOG } from "./examples/plugins/git/test/small.mjs"; process.stdout.write(SMALL_LOG)' > examples/plugins/git/test/small.gitlog`
- `scripts/orch/node-slim.sh node --experimental-strip-types --test examples/plugins/git/test/`

Expected: all 8 tests pass.

- [ ] **Step 7: Commit**

`git add examples/plugins/git && git -c user.name=LESdylan -c user.email=dev.pro.photo@gmail.com commit -qm updated`
(If the job's permission layer denies `git commit`, leave the change staged and say so.)

### Task 2: export, derive, decorate, run

**Files:**
- Create: `examples/plugins/git/git-log.sh`, `examples/plugins/git/export.mjs`,
  `examples/plugins/git/decorate.mjs`, `examples/plugins/git/run.sh`,
  `examples/plugins/git/test/small.ingest.json` (generated), `examples/plugins/git/test/small.graph.json`
  (generated), `examples/plugins/git/test/bad.gitlog`, `examples/plugins/git/test/decorate.test.mjs`

**Interfaces:**
- Consumes:
  - Task 1's `parseLog`, `toRows`, `LogRefusal`;
  - the SDK's `rowsToIngest` and `RowsAdapterError` (`crates/graph-sdk-js/src/adapters/rows.ts`);
  - `graph-cli ingest --from F --member ingest (--out G | --check G)`.
- Produces:
  - `export.mjs <name> <log> <out>`, which writes `{"ingest": <contract>}`;
  - `decorate.mjs <ingest.json> <graph.json> <field> <out>`, which writes
    `{"version":1,"nodes":[…+tags],"edges":[…]}`;
  - `run.sh <repo> [<name>]`, which writes `target/git-plugin/<name>/<name>.studio.json` and
    prints that path.

- [ ] **Step 1: Write `examples/plugins/git/git-log.sh`**

```sh
#!/usr/bin/env sh
# git-log.sh <repo>: the plugin's one git call, on the host's git. Children first, every ref.
# One commit per line: %s is the subject's first line, so a record never spans two lines.
set -eu
exec git -C "$1" log --all --topo-order --format='%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s'
```

- [ ] **Step 2: Write `examples/plugins/git/export.mjs`**

```js
#!/usr/bin/env node
// export.mjs <name> <log> <out>: a `git log` file (map.mjs FORMAT) to `{"ingest": <contract>}`,
// the member `graph-cli ingest --member ingest` reads. Counts go to stderr.
// Exit 0 written; 2 refused, naming the log line or the adapter's path.
import { readFile, writeFile } from "node:fs/promises";
import { RowsAdapterError, rowsToIngest } from "../../../crates/graph-sdk-js/src/adapters/rows.ts";
import { LogRefusal, parseLog, toRows } from "./map.mjs";

const NAME = /^[A-Za-z0-9._-]{1,64}$/;
const [name, logPath, outPath] = process.argv.slice(2);
if (!NAME.test(name ?? "") || !logPath || !outPath) {
  process.stderr.write("usage: export.mjs <name: [A-Za-z0-9._-]{1,64}> <log> <out>\n");
  process.exit(2);
}
try {
  const commits = parseLog(await readFile(logPath, "utf8"));
  const { rows, dropped } = toRows(commits, name);
  const text = JSON.stringify({ ingest: rowsToIngest(rows) });
  await writeFile(outPath, text);
  const bytes = Buffer.byteLength(text);
  process.stderr.write(`${name}: ${commits.length} commits, ${dropped} parents outside the log dropped, ${bytes} bytes\n`);
} catch (error) {
  if (!(error instanceof LogRefusal || error instanceof RowsAdapterError)) throw error;
  process.stderr.write(`${logPath}: ${error.message}\n`);
  process.exit(2);
}
```

- [ ] **Step 3: Write `examples/plugins/git/decorate.mjs`**

```js
#!/usr/bin/env node
// decorate.mjs <ingest.json> <graph.json> <field> <out>: the derived graph with every record's
// `<field>` cell copied onto its node as `tags`, the member the studio's `tag:#x` query reads
// (packages/graph-studio/src/source/ingest.ts). It knows the contract's node id rule
// (`source:collection:record`, crates/graph-core/src/ingest/build.rs) and nothing about where
// the records came from. Exit 2 when a record has no node or a cell is not a list of strings.
import { readFile, writeFile } from "node:fs/promises";

const [ingestPath, graphPath, field, outPath] = process.argv.slice(2);
if (!ingestPath || !graphPath || !field || !outPath) {
  process.stderr.write("usage: decorate.mjs <ingest.json> <graph.json> <field> <out>\n");
  process.exit(2);
}
const { ingest } = JSON.parse(await readFile(ingestPath, "utf8"));
const graph = JSON.parse(await readFile(graphPath, "utf8"));
const tags = tagsById(ingest, field);
const nodes = graph.nodes.map((node) => (tags.has(node.id) ? { ...node, tags: take(tags, node.id) } : node));
if (tags.size !== 0) refuse(`${tags.size} records have no node, first ${tags.keys().next().value}`);
await writeFile(outPath, JSON.stringify({ version: 1, nodes, edges: graph.edges }));

function tagsById(doc, name) {
  const byId = new Map();
  for (const record of doc.records) {
    const cell = record.values[name];
    if (record.deleted || cell === undefined) continue;
    if (!Array.isArray(cell) || !cell.every((t) => typeof t === "string")) {
      refuse(`record ${record.id}: \`${name}\` is not a list of strings`);
    }
    byId.set(`${doc.source}:${record.collection}:${record.id}`, cell);
  }
  return byId;
}

function take(map, id) {
  const value = map.get(id);
  map.delete(id);
  return value;
}

function refuse(why) {
  process.stderr.write(`decorate: ${why}\n`);
  process.exit(2);
}
```

- [ ] **Step 4: Write `examples/plugins/git/run.sh`**

```sh
#!/usr/bin/env bash
# run.sh <repo> [<name>]: a repository to a studio document, printed as its path. <name> is the
# ingest source and defaults to the repository directory's name without `.git`. Work files go
# under target/git-plugin/<name>/ because node-slim and gr see only the worktree (/w).
set -euo pipefail
repo=$(cd "$1" && pwd)
name=${2:-$(basename "$repo" .git)}
root=$(git rev-parse --show-toplevel)
here=examples/plugins/git
work=target/git-plugin/$name
mkdir -p "$root/$work"
cd "$root"
"$here/git-log.sh" "$repo" > "$work/log"
scripts/orch/node-slim.sh node --experimental-strip-types "$here/export.mjs" "$name" "$work/log" "$work/ingest.json"
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo run -q --release -p graph-cli -- ingest \
  --from "$work/ingest.json" --member ingest --out "$work/graph.json"
scripts/orch/node-slim.sh node "$here/decorate.mjs" "$work/ingest.json" "$work/graph.json" refs "$work/$name.studio.json"
echo "$work/$name.studio.json"
```

Run: `chmod +x examples/plugins/git/*.sh`

- [ ] **Step 5: Generate the small fixtures and check that the motor accepts them**

Run:
- `scripts/orch/node-slim.sh node --experimental-strip-types examples/plugins/git/export.mjs small examples/plugins/git/test/small.gitlog examples/plugins/git/test/small.ingest.json`
- `CARGO_BUILD_JOBS=3 scripts/orch/gr cargo run -q --release -p graph-cli -- ingest --from examples/plugins/git/test/small.ingest.json --member ingest --out examples/plugins/git/test/small.graph.json`

Expected:
- exit 0, and stderr `small: 6 commits, 1 parents outside the log dropped, … bytes`;
- `small.graph.json` holds 6 nodes and 8 `relation` edges, with no `tag` node and no `tag` edge.
  Check with `jq '[.nodes|length, (.edges|length), ([.nodes[]|select(.kind=="tag")]|length)]'`,
  which should print `[6,8,0]`.

If `graph-cli ingest` refuses the document, the refusal is a fact about the contract. Stop and
report its text under "decisions needed". Do not edit the contract or the SDK.

- [ ] **Step 6: Write `examples/plugins/git/test/decorate.test.mjs`**

```js
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { H } from "./small.mjs";

const here = new URL(".", import.meta.url).pathname;
const decorate = (field) => {
  const out = join(mkdtempSync(join(tmpdir(), "decorate-")), "out.json");
  execFileSync(process.execPath, [join(here, "../decorate.mjs"), join(here, "small.ingest.json"), join(here, "small.graph.json"), field, out]);
  return JSON.parse(readFileSync(out, "utf8"));
};

test("every commit node carries its refs as tags, and nothing else changes", () => {
  const graph = JSON.parse(readFileSync(join(here, "small.graph.json"), "utf8"));
  const doc = decorate("refs");
  assert.equal(doc.version, 1);
  assert.deepEqual(doc.edges, graph.edges);
  const byId = new Map(doc.nodes.map((n) => [n.id, n]));
  assert.deepEqual(byId.get(`small:commit:${H("e")}`).tags, ["main", "origin/main", "merge"]);
  assert.deepEqual(byId.get(`small:commit:${H("a")}`).tags, ["root"]);
  assert.deepEqual(doc.nodes.map(({ tags, ...rest }) => rest), graph.nodes);
});

test("a field that is not a list of strings is refused", () => {
  assert.throws(() => decorate("subject"), (e) => e.status === 2);
});

test("the committed ingest document is what export.mjs writes today", () => {
  const out = join(mkdtempSync(join(tmpdir(), "export-")), "small.ingest.json");
  execFileSync(process.execPath, ["--experimental-strip-types", join(here, "../export.mjs"), "small", join(here, "small.gitlog"), out], { stdio: "pipe" });
  assert.equal(readFileSync(out, "utf8"), readFileSync(join(here, "small.ingest.json"), "utf8"));
});
```

- [ ] **Step 7: Write the negative-control input**

`examples/plugins/git/test/bad.gitlog`: one valid line, then one line whose hash is
abbreviated. Generate it, never hand-type `\x1f`:
`scripts/orch/node-slim.sh node --input-type=module -e 'import { H } from "./examples/plugins/git/test/small.mjs"; const ok = [H("a"), "", "Ana", "1", "", "ok"]; process.stdout.write([ok, ["abc1234", ...ok.slice(1)]].map((f) => f.join("\x1f")).join("\n") + "\n")' > examples/plugins/git/test/bad.gitlog`

`scripts/orch/rows/git-plugin.rows` is already on develop with five rows: `no-engine`, `plugin-test`,
`plugin-converges`, `negctl-refused` (expects `export.mjs` to exit 2 on `bad.gitlog`) and
`negctl-converges` (expects `--check` to exit 1 on a graph missing one node). Read it, do not edit it.

- [ ] **Step 8: Run the rows**

Run: `scripts/orch/gate.sh target/gate-git-plugin scripts/orch/rows/git-plugin.rows`, then
`cat target/gate-git-plugin/summary.txt`.
Expected: 5 PASS. (`no-engine` checks the working tree against the merge base, so it holds
before and after the commit.)

- [ ] **Step 9: Open a real history end to end**

Run: `examples/plugins/git/run.sh /tmp/gitviz/contributor-stats.git`
Expected:
- it prints `target/git-plugin/contributor-stats/contributor-stats.studio.json`;
- stderr shows 488 commits;
- `jq '.nodes|length'` on the file prints 488.

If the job cannot read `/tmp/gitviz` (permission layer), run it on this worktree instead
(`examples/plugins/git/run.sh . graph_render`) and say so.

- [ ] **Step 10: Commit**, as in Task 1.

### Task 3: bench, measurement, README

**Files:**
- Create: `examples/plugins/git/bench.mjs`, `examples/plugins/git/README.md`,
  `docs/measurements/git-plugin.md`

**Interfaces:**
- Consumes:
  - Task 1's `parseLog` and `toRows`;
  - the SDK's `rowsToIngest` and `createMotor` (`crates/graph-sdk-js/src/index.ts`);
  - `motor.buildContract(json)`, `motor.layouts()`, `motor.layout(handle, id)`,
    `motor.toJSON(handle)`, `motor.release(handle)`.

- [ ] **Step 1: Write `examples/plugins/git/bench.mjs`**

```js
#!/usr/bin/env node
// bench.mjs <wasm> <log>...: the plugin's in-process steps, timed, 3 rounds, medians. Per log:
// parse, map + rowsToIngest, stringify, the wasm motor's contract build (the same derivation
// as graph-cli ingest), then `layout.dag.sugiyama` and `layout.dag.lanes` (when registered) with their note counts.
// Caveat: wall clock on a shared host; a median under load is an upper bound. Container
// start-up (node-slim, gr) is outside every number here and is reported apart.
import { readFile } from "node:fs/promises";
import { rowsToIngest } from "../../../crates/graph-sdk-js/src/adapters/rows.ts";
import { createMotor } from "../../../crates/graph-sdk-js/src/index.ts";
import { parseLog, toRows } from "./map.mjs";

const ROUNDS = 3;
const [wasmPath, ...logs] = process.argv.slice(2);
const motor = await createMotor(await readFile(wasmPath));
// dag.dot is left out: 2.6 s at 2k commits in the probe, so it would dominate every round.
const layouts = motor.layouts().filter((id) => ["layout.dag.sugiyama", "layout.dag.lanes"].includes(id));
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const timed = (f) => { const t = performance.now(); const value = f(); return [value, performance.now() - t]; };

for (const path of logs) {
  const text = await readFile(path, "utf8");
  const times = new Map();
  const push = (key, ms) => times.set(key, [...(times.get(key) ?? []), ms]);
  let facts = "";
  for (let round = 0; round < ROUNDS; round += 1) facts = once(text, push);
  const cells = [...times].map(([key, ms]) => `${key}=${median(ms).toFixed(1)}ms`);
  console.log(`${path} ${facts} ${cells.join(" ")} load=${(await readFile("/proc/loadavg", "utf8")).split(" ")[0]}`);
}

function once(text, push) {
  const [commits, parse] = timed(() => parseLog(text));
  const [ingest, map] = timed(() => rowsToIngest(toRows(commits, "bench").rows));
  const [json, stringify] = timed(() => JSON.stringify(ingest));
  const [handle, build] = timed(() => motor.buildContract(json));
  [["parse", parse], ["map", map], ["stringify", stringify], ["build", build]].forEach(([k, ms]) => push(k, ms));
  const notes = layouts.map((id) => layoutOnce(handle, id, push));
  motor.release(handle);
  return `n=${commits.length} bytes=${Buffer.byteLength(json)} ${notes.join(" ")}`;
}

function layoutOnce(handle, id, push) {
  const [, ms] = timed(() => motor.layout(handle, id));
  push(id, ms);
  const codes = {};
  for (const code of JSON.parse(motor.toJSON(handle)).notes?.code ?? []) codes[code] = (codes[code] ?? 0) + 1;
  return `${id}.notes=${JSON.stringify(codes)}`;
}
```

If `motor.layout` throws for one layout (for example, a scale ceiling), catch it in
`layoutOnce`, print `<id>=REFUSED <message>`, and go on.

- [ ] **Step 2: Produce the logs and the wasm, then run the bench**

Run:

```sh
mkdir -p target/git-plugin/logs
for r in contributor-stats activitywatch aw-server-rust git; do
  examples/plugins/git/git-log.sh /tmp/gitviz/$r.git > target/git-plugin/logs/$r.log
done
examples/plugins/git/git-log.sh . > target/git-plugin/logs/graph_render.log
/usr/bin/time -f '%e s' examples/plugins/git/git-log.sh /tmp/gitviz/git.git > /dev/null   # three times; median
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo build -q -p graph-wasm --release --target wasm32-unknown-unknown
flock ~/goinfre/orch/bench.lock scripts/orch/node-slim.sh node --experimental-strip-types \
  examples/plugins/git/bench.mjs target/wasm32-unknown-unknown/release/graph_wasm.wasm \
  target/git-plugin/logs/{contributor-stats,aw-server-rust,activitywatch,graph_render,git}.log
```

Respect the bench preconditions in Global Constraints before the `flock` line. Then time one
`run.sh` end to end on git/git (`/usr/bin/time -f '%e s' examples/plugins/git/run.sh /tmp/gitviz/git.git`)
three times, after a first run that builds graph-cli.

- [ ] **Step 3: Write `docs/measurements/git-plugin.md`.** Include:
  - the command lines above;
  - the bench table: repo, commits, contract bytes, parse, map, stringify, build, one column per
    dag layout, notes;
  - git's own `git log` time;
  - `run.sh` end to end, with container start-up shown separately from the in-process sum;
  - the load at each run;
  - the Caveat line from `bench.mjs`;
  - the target "git/git end to end ≤ 3 s", met or missed with its number, never rounded into
    a pass.

  Findings, each as a fact with its number:
  - **The hub.** git/git's contract bytes against the hub's 16 MiB
    `GRAPH_HUB_MAX_PLUGIN_BYTES`.
  - **Missing generic tools.** The studio opens provisional node/edge JSON, not a contract
    document. That is why `graph-cli ingest` and `decorate.mjs` sit in the pipeline: a
    generic studio feature, not plugin code, would remove both.
  - **Note 4 counts.** The count for `layout.dag.sugiyama` on each history.

- [ ] **Step 4: Write `examples/plugins/git/README.md`.** Cover:
  - what it does (the pipeline diagram from this plan's Architecture);
  - `run.sh` usage and its output path;
  - how to open the file in the studio (the Source section's "Open a file");
  - the rules a user can apply with no new code: colour by group = author, `filter.group`,
    `tag:#main`, `tag:#merge`, `db:commit`;
  - what it does not do, copied from the spec;
  - one line saying the engine does not know this plugin exists.

  Copy commands from the runs above. Never compose them.

- [ ] **Step 5: Rerun the rows**, then commit

Run: `scripts/orch/gate.sh target/gate-git-plugin scripts/orch/rows/git-plugin.rows`
Expected: 5 PASS. Commit as in Task 1.

## Return block

```
status: done | partial | blocked
changed: <files>
rows: <each git-plugin.rows row -> PASS/FAIL>
bench: <repo -> commits, bytes, parse/map/stringify/build ms, dag layouts ms + notes>, load
end to end git/git: <s, with container start-up apart>
findings: <hub bytes vs 16 MiB; generic gaps>
deviations: <none | list>
decisions needed: <none | list>
```
