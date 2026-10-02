# Examples

Every example here is **copied from a test that passes**, not composed by hand
(`documenter.md`): the command under each one is the gate row that runs it, and if an
example stops matching the code the row goes red with it. Nothing below is aspirational.

Run them with Node ≥22.6, which strips the types in this package's `.ts` sources:

```sh
node --experimental-strip-types harness/sdk-smoke.mjs --adapter-convergence
node --experimental-strip-types harness/read-snapshot-raw.mjs
```

---

## 1. Map a source into the ingest contract

The SDK ships two adapters, deliberately unlike each other. Both are **pure mappings**:
source shape → contract document. Neither builds a node id, derives an edge, chooses a
strength or synthesises a tag hub — that is the motor's one derivation, in Rust, and an
adapter that grew graph logic would be the abstraction leaking.

```ts
import { rowsToIngest } from "@graph-motor/sdk-js/adapters/rows";

const ingest = rowsToIngest({
  source: "lib",
  tables: [
    {
      id: "task",
      name: "Tasks",
      titleColumn: "name",
      columns: [
        { name: "name", label: "Name", role: "title" },
        { name: "labels", label: "Labels", role: "tags" },
        { name: "up", label: "Up", role: "parent" },
        { name: "blocks", label: "Blocks", role: "link",
          link: { collection: "task", cardinality: "many", symmetric: false } },
      ],
      rows: [
        { id: "t1", updatedAt: 1700000001,
          values: { name: "Write the contract", labels: ["docs", "p0"] } },
        { id: "t2", updatedAt: 1700000002,
          values: { name: "Ship the adapters", labels: ["p0"], up: ["t1"], blocks: ["t3"] } },
      ],
    },
  ],
});
```

`ingest` is now a plain object matching `docs/contract/ingest-schema.json`: declared
roles, records, and nothing else. A role outside the eight is a refusal, not a default.

## 2. Map the *same* dataset from a Notion export

```ts
import { notionToIngest } from "@graph-motor/sdk-js/adapters/notion";

const ingest = notionToIngest(
  {
    source: "lib",
    databases: [
      {
        id: "task",
        title: [{ plain_text: "Tasks" }],
        properties: {
          name:   { id: "name",   name: "Name",   type: "title" },
          labels: { id: "labels", name: "Labels", type: "multi_select" },
          up:     { id: "up",     name: "Up",     type: "relation", relation: { database_id: "task" } },
          blocks: { id: "blocks", name: "Blocks", type: "relation", relation: { database_id: "task" } },
        },
      },
    ],
    pages: [
      { id: "t1", parent: { database_id: "task" },
        properties: {
          name:   { title: [{ plain_text: "Write the contract" }] },
          labels: { multi_select: [{ name: "docs" }, { name: "p0" }] },
          up:     { relation: [] },
          blocks: { relation: [] },
        } },
      { id: "t2", parent: { database_id: "task" },
        properties: {
          name:   { title: [{ plain_text: "Ship the adapters" }] },
          labels: { multi_select: [{ name: "p0" }] },
          up:     { relation: [{ id: "t1" }] },
          blocks: { relation: [{ id: "t3" }] },
        } },
    ],
  },
  // The declarations a vendor property type cannot give you. A `relation` says what
  // Notion stored, not what it *means*: two relations in one database are a parent and
  // a blocking dependency, and the type is identical for both. A declaration outranks
  // the adapter's table.
  {
    roles: { "task.up": "parent" },
    links: {
      "task.up":     { cardinality: "one",  symmetric: false },
      "task.blocks": { cardinality: "many", symmetric: false },
    },
  },
);
```

Both of these produce **byte-identical** documents. That is the point of the phase, and
it is a gate row rather than a claim:

```
$ node harness/sdk-smoke.mjs --adapter-convergence
ok - the rows adapter maps fixtures/ingest/rows.json
ok - the notion adapter maps fixtures/ingest/notion.json
ok - two adapters, one contract document, identical bytes
ok - both adapters produce the document expected-graph.json pins
# pass
```

The full pair is `fixtures/ingest/{rows.json,notion.json,expected-graph.json}`: the same
logical dataset in two source shapes, and the graph both must produce.

## 3. Watch a derivation, without writing Rust

The derivation is `graph_core::ingest::build` and it is the same call the convergence
test makes. `graph-cli ingest` runs it, so you can see what the motor does with a
document before committing to the ABI:

```sh
graph-cli ingest --from fixtures/ingest/expected-graph.json --member ingest
```

```
node lib:task:t1 Record label="Write the contract" group="write-the-contract" weight=3 version=1700000001
node lib:task:t2 Record label="Ship the adapters" group="ship-the-adapters" weight=5 version=1700000002
node lib:task:t3 Record label="Prove convergence" group="prove-convergence" weight=8 version=1700000003
node lib:person:p1 Record label="Ada" group=None weight=0.5 version=1700000010
node lib:person:p2 Record label="Grace" group=None weight=0.5 version=1700000011
node tag:docs Tag label="docs" group=None weight=0.5 version=0
node tag:p0 Tag label="p0" group=None weight=0.5 version=0
node tag:graph Tag label="graph" group=None weight=0.5 version=0
edge lib:task:t1--lib:task:t2:hierarchy: lib:task:t1 -> lib:task:t2 Hierarchy label="" strength=2 directed=false
edge lib:task:t2->lib:task:t3:relation:blocks lib:task:t2 -> lib:task:t3 Relation label="blocks" strength=1 directed=true
edge lib:task:t1--tag:docs:tag:docs lib:task:t1 -> tag:docs Tag label="docs" strength=0.75 directed=false
…
```

Three things to notice, because they are the contract working rather than the adapter
working:

- **`t4` is gone.** The record is `deleted` in both source shapes, and a deleted record
  derives nothing at all.
- **`tag:docs` exists** even though nobody declared a tag collection: one hub node per
  distinct value of a `tags` field, and one edge per record that carries it.
- **The strengths are `2`, `1`, `0.75`** — from the one table,
  `crates/graph-core/src/ingest/strength.rs`, which replaced three divergent sets the
  host had accumulated. That is a *chosen convention*, not a derived truth
  (`docs/decisions/edge-strength-table.md`), and changing it changes every layout.

## 4. Read the output with no SDK at all

The motor's promise is that its JSON face is a contract, not a private format. This is a
snapshot read with `JSON.parse` and nothing else — no import from this package, no wasm,
no column views:

```js
import { readFile } from "node:fs/promises";

const snapshot = JSON.parse(await readFile("snapshot.json", "utf8"));

// Nodes are addressed by stable string id; the dense index never crosses the wire.
const { id, x, y } = { id: snapshot.nodes.id, ...snapshot.geometry.nodes };
for (const [i, nodeId] of id.entries()) {
  console.log(nodeId, x[i], y[i]);
}

// An edge carries its endpoints as node *ids*, not positions.
for (const [i, edgeId] of snapshot.edges.id.entries()) {
  const from = snapshot.edges.source[i];
  const to = snapshot.edges.target[i];
  console.log(edgeId, from, "->", to);
}
```

`harness/read-snapshot-raw.mjs` is that reader, with the checks written out — every
`$ref` in the schema resolves, every object refuses an unknown member, every edge
endpoint names a node that exists, and the version is one the reader understands (a
reader meeting a **newer major** refuses rather than guessing). It is also a gate row:

```
$ node harness/read-snapshot-raw.mjs
ok - every $ref in the committed schema resolves inside it
ok - every object in the committed schema refuses an unknown member
ok - the snapshot carries every member the schema requires
…
# 4 nodes, 5 edges, Point nodes / Line edges, bounds x[-0.5, 0.5] y[-0.5, 0.5]
#   bench:db-0:0 at (-0.5, -0.5)
#   bench:db-1:1 at (0.5, -0.5)
#   bench:db-2:2 at (-0.5, 0.5)
#   bench:db-3:3 at (0.5, 0.5)
# pass
```

Two properties make that work, and both are worth stating because they are the reason
the format is portable at all:

- **The geometry kind is per-snapshot, not per-element.** `geometry.nodes.kind` is one
  tag for the whole payload, so a reader switches once: `Point` has `x`/`y`, `Circle`
  adds `r`, `Box` has `x`/`y`/`w`/`h`. There is no sniffing and no optional column.
- **Two reserved kinds are refused, not guessed at**: `Ribbon` and `Arc` are allocated
  tags with no implementation, and a reader that meets one must say so rather than
  rendering something plausible.

## 5. Build and lay out, through the SDK

The part that has not changed: `Motor` is the wasm ABI wrapper, and the layout registry
is read from the module rather than hard-coded.

```js
import { createMotor, ColumnId } from "@graph-motor/sdk-js";
import { readFile } from "node:fs/promises";

const motor = await createMotor(await readFile("graph_wasm.wasm"));
const handle = motor.build(await readFile("graph.ingest.json", "utf8"));

for (const layoutId of motor.layouts()) {
  const run = motor.layout(handle, layoutId);
  console.log(layoutId, run.nodeCount, run.nodeKind, run.edgeKind);
}

const x = motor.column(handle, ColumnId.NodeX); // a view over the motor's own memory
console.log(x[0], x[1], "…");
console.log(motor.toJSON(handle)); // the canonical JSON face, keys sorted, one line
motor.release(handle);
```

`Motor#layouts()` returns whatever the loaded module registered, so a layout added
after this README was written is reachable and discoverable with no change here. And
`motor.toJSON(handle)` is the same document the SDK's own readers above could have
produced without it — which is the point of having two faces.
