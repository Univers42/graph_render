// The provisional ingest document this arm hands `gm_build`, and the check that it is one
// the contract would accept.
//
// The record shapes live here rather than beside the timer so the timing code reads as
// timing code, and the member lists come from `contract.mjs` — the committed contract, not
// a copy of the writer.

import { memberRefusal } from "./contract.mjs";

/** One node record in the provisional ingest shape, every member present. */
export function nodeOf(node) {
  return {
    id: node.id,
    kind: node.kind,
    database_id: node.databaseId ?? null,
    source: node.source,
    label: node.label,
    group: node.group ?? null,
    weight: node.weight,
    version: node.version,
    has_note: node.hasNote,
    icon: node.icon ?? null,
  };
}

/** One edge record in the provisional ingest shape, every member present.
 *  `child_first` is the one member the contract makes optional in version 1; this arm
 *  states it explicitly so both sides of the comparison are the same list. */
export function edgeOf(edge) {
  return {
    id: edge.id,
    source: edge.source,
    target: edge.target,
    kind: edge.kind,
    label: edge.label,
    strength: edge.strength,
    directed: edge.directed,
    record_id: null,
    child_first: false,
  };
}

/** The model's ingest document, parsed: version 1, nodes then edges, nothing else. */
export function ingestOf(model) {
  return JSON.parse(
    JSON.stringify({
      version: 1,
      nodes: model.nodes.map(nodeOf),
      edges: model.edges.map(edgeOf),
    }),
  );
}

/** Why `document` is not one `gm_build` would accept, or `null` when it is. Every record
 *  is checked, so the timed workload is a document the module accepts rather than one it
 *  would refuse after the timer started. */
export function documentRefusal(document, members) {
  if (document.version !== 1) return `the ingest document's version is ${document.version}, not 1`;
  if (document.nodes.length === 0) return "the ingest document holds no node";
  if (document.edges.length === 0) return "the ingest document holds no edge";
  for (const [what, records, required] of [
    ["a node record", document.nodes, members.node],
    ["an edge record", document.edges, members.edge],
  ]) {
    for (const [i, record] of records.entries()) {
      const refusal = memberRefusal(`${what} (${i})`, Object.keys(record), required);
      if (refusal !== null) return refusal;
    }
  }
  return null;
}