// The adapter-convergence proof (`--adapter-convergence`), split out of sdk-smoke.mjs.

import { readFile } from "node:fs/promises";
import {
  ContractRefusedError,
  createMotor,
} from "../../crates/graph-sdk-js/src/index.ts";
import { canonicalJson, expectedGraph, expectedIngest, ingestFromNotion, ingestFromRows } from "../adapter-convergence.mjs";
import { check, refusedWith, reportDifference } from "./lib.mjs";

export async function runConvergence(wasmPath) {
  const fromRows = canonicalJson(await ingestFromRows());
  const fromNotion = canonicalJson(await ingestFromNotion());
  const expected = canonicalJson(await expectedIngest());
  check("the rows adapter maps fixtures/ingest/rows.json", fromRows.length > 0);
  check("the notion adapter maps fixtures/ingest/notion.json", fromNotion.length > 0);
  check("two adapters, one contract document, identical bytes", fromRows === fromNotion);
  check("both adapters produce the document expected-graph.json pins", fromRows === expected);
  reportDifference(fromRows, { notion: fromNotion, expected });
  // The whole proof, end to end, in one command — the open item this mode existed to
  // close. The three steps a consumer actually performs, chained, with no step restated:
  //
  //   source shape --adapter--> contract document --Motor#buildContract--> handle
  //
  // The first two steps used to be provable only in this runtime (the adapters are
  // TypeScript) and the third only in Rust (the derivation is `graph_core::ingest`'s and
  // only there), so a reviewer had to read two runtimes to believe the loop was closed.
  // `gm_build_contract` means the third step is reachable from here too, so the committed
  // fixture's `graph` member is now checked *in the same process that produced `ingest`*:
  // if the derivation moved, or a node id or a strength changed, this fails.
  //
  // The command is one line, and it is in `docs/contract/wasm-abi.md` ("The convergence
  // proof, one command"): a cargo build and a node run, each in its own container,
  // because the gate image has no node and the node image has no cargo. Pass this mode a
  // wasm path to get the third step; run it with no path and the first two still run,
  // which is how the pre-`gm_build_contract` gate row is written and keeps working.
  if (wasmPath) {
    const contractMotor = await createMotor(await readFile(wasmPath));
    const handle = contractMotor.buildContract(fromRows);
    const snapshot = JSON.parse(contractMotor.toJSON(contractMotor.layout(handle, "layout.grid").handle));
    const graph = await expectedGraph();
    check("the document derives the committed graph's node count", contractMotor.nodeCount(handle) === graph.nodes.length);
    check(
      "the derived node ids are the committed ones, in derivation order",
      JSON.stringify(snapshot.nodes.id) === JSON.stringify(graph.nodes.map((n) => n.id)),
      `derived ${JSON.stringify(snapshot.nodes.id)}`,
    );
    // The snapshot's edge face is **columnar** (`{id, source, target}` are three parallel
    // arrays, `docs/contract/binary-layout.md`), so the comparison is column by column
    // rather than object by object. Checked rather than assumed: a snapshot that named
    // its columns differently would make this comparison vacuously true.
    for (const column of ["id", "source", "target"]) {
      check(
        `the derived edge ${column} column is the committed one, in derivation order`,
        JSON.stringify(snapshot.edges[column]) === JSON.stringify(graph.edges.map((e) => e[column])),
        `derived ${JSON.stringify(snapshot.edges[column])}`,
      );
    }
    contractMotor.release(handle);
    // Negative control: a mutated contract must be **refused**, not quietly
    // reinterpreted. Two mutations, each in a place only the strict reader can catch: an
    // unknown member, and a role outside the eight. Both are documents a lenient reader
    // would derive *something* from — the first by ignoring the extra, the second by
    // defaulting the role — which is why "it produced a graph" is not the assertion here;
    // the refusal is.
    const mutations = [
      ["an unknown member", (text) => text.replace('"source":"lib"', '"source":"lib","extra":1')],
      ["a role outside the eight", (text) => text.replace('"role":"title"', '"role":"Title"')],
      ["a version the contract does not name", (text) => text.replace('"version":1', '"version":2')],
      ["a tag value that cannot round-trip through the node-id grammar", (text) => text.replace('"docs"', '"do:cs"')],
    ];
    for (const [what, mutate] of mutations) {
      const mutated = mutate(fromRows);
      check(`the mutation is really a mutation: ${what}`, mutated !== fromRows);
      check(`${what} is refused, not derived`, await refusedWith(ContractRefusedError, () => contractMotor.buildContract(mutated)));
    }
    check("a contract document with no records at all is a legal empty graph, not a refusal", (() => {
      const empty = { version: 1, source: "lib", collections: [], records: [] };
      const built = contractMotor.buildContract(JSON.stringify(empty));
      const count = contractMotor.nodeCount(built);
      contractMotor.release(built);
      return count === 0;
    })());
  }
}
