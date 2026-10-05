/**
 * Getting a document the wasm motor will accept.
 *
 * The provisional ingest shape (`crates/graph-wasm/src/ingest.rs`) names all
 * ten node members and all nine edge members, refuses an unknown one, and
 * refuses a duplicate id or a dangling endpoint. Two things arrive at the studio
 * that are not in that shape: the motor's own `fixtures/*.json`, which are
 * shorthand (`{about, nodes:[{id}], edges:[{id,source,target}]}`) with
 * annotations, and whatever JSON a user drops on the window.
 *
 * So this module FILLS the missing members with documented defaults, records
 * every default it filled and every annotation it dropped (the UI shows them —
 * a silent fill would be a lie about what the engine was given), and refuses
 * everything the contract refuses. Normalising is idempotent: a document this
 * module produced passes through with no notes at all. `doc` carries `tags` and
 * `path`, which `json` — the text the motor reads — never does.
 *
 * This file reads the document around its records; one record at a time is
 * `ingest-record.ts`, which owns the contract's member names and the defaults.
 */
import { isObject, readEdge, readNode, wireNode } from "./ingest-record.ts";
import { type IngestDoc, type IngestEdge, type IngestNode, IngestRefusal } from "./ingest-shape.ts";
import { documentRefusal } from "./limits.ts";

export { EDGE_KINDS, IngestRefusal, NODE_KINDS } from "./ingest-shape.ts";
export type { EdgeKind, IngestDoc, IngestEdge, IngestNode, NodeKind } from "./ingest-shape.ts";

export interface Normalised {
  /** The ingest text to hand `gm_build`, member order per the contract. */
  readonly json: string;
  /** The contract's shape plus `tags` and `path` per node. */
  readonly doc: IngestDoc;
  /** Every default filled and annotation dropped, in the order they happened. */
  readonly notes: readonly string[];
}

function parseDocument(source: string, text: string): Record<string, unknown> {
  const refused = documentRefusal(text.length);
  if (refused !== null) throw new IngestRefusal(source, refused);
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    throw new IngestRefusal(source, error instanceof RangeError ? "too large to parse in this page" : "not JSON");
  }
  if (!isObject(parsed)) throw new IngestRefusal(source, "the document is not a JSON object");
  return parsed;
}

function readVersion(source: string, root: Record<string, unknown>, notes: string[]): 1 {
  const version = root["version"];
  if (version === undefined) {
    notes.push("no `version` member: assumed 1");
    return 1;
  }
  if (version !== 1) throw new IngestRefusal(source, `unsupported version ${JSON.stringify(version)}`);
  return 1;
}

function readArray(source: string, root: Record<string, unknown>, key: "nodes" | "edges"): unknown[] {
  const value = root[key];
  if (!Array.isArray(value)) throw new IngestRefusal(source, `no \`${key}\` array`);
  return value;
}

function readNodes(source: string, values: unknown[], notes: string[]): IngestNode[] {
  const nodes: IngestNode[] = [];
  const seen = new Set<string>();
  for (let i = 0; i < values.length; i += 1) {
    const node = readNode(source, `nodes[${i}]`, values[i], notes);
    if (seen.has(node.id)) throw new IngestRefusal(source, `duplicate node id ${JSON.stringify(node.id)}`);
    seen.add(node.id);
    nodes.push(node);
  }
  return nodes;
}

/** The ids the edges' ends must name, filled in from the nodes already read: the `nodes.map`
 *  this replaces was 85 928 strings and an array per document. */
function idSet(nodes: readonly IngestNode[]): ReadonlySet<string> {
  const ids = new Set<string>();
  for (const node of nodes) ids.add(node.id);
  return ids;
}

/** `source` before `target`, and each refusal names the end it is about. */
function checkEnd(source: string, edge: IngestEdge, end: "source" | "target", ids: ReadonlySet<string>): void {
  const id = end === "source" ? edge.source : edge.target;
  if (ids.has(id)) return;
  throw new IngestRefusal(
    source, `edge ${JSON.stringify(edge.id)} ${end}s ${JSON.stringify(id)}, which is not a node`,
  );
}

function readEdges(source: string, values: unknown[], nodes: readonly IngestNode[], notes: string[]): IngestEdge[] {
  const ids = idSet(nodes);
  const edges: IngestEdge[] = [];
  const seen = new Set<string>();
  for (let i = 0; i < values.length; i += 1) {
    const edge = readEdge(source, `edges[${i}]`, values[i], notes);
    if (seen.has(edge.id)) throw new IngestRefusal(source, `duplicate edge id ${JSON.stringify(edge.id)}`);
    seen.add(edge.id);
    checkEnd(source, edge, "source", ids);
    checkEnd(source, edge, "target", ids);
    edges.push(edge);
  }
  return edges;
}

/** The wire document: the ten members per node, the nine per edge, and the version. */
function wireDocument(version: 1, nodes: readonly IngestNode[], edges: IngestEdge[]): string {
  const wire = new Array<Record<string, unknown>>(nodes.length);
  for (let i = 0; i < nodes.length; i += 1) {
    const node = nodes[i];
    if (node !== undefined) wire[i] = wireNode(node);
  }
  return JSON.stringify({ version, nodes: wire, edges });
}

/** Normalise `text` into the ingest document `gm_build` reads, or refuse it with
 *  the reason and the file it came from. */
export function normaliseIngest(text: string, source: string): Normalised {
  const notes: string[] = [];
  const root = parseDocument(source, text);
  for (const key in root) {
    if (key !== "version" && key !== "nodes" && key !== "edges") notes.push(`dropped annotation \`${key}\``);
  }
  const version = readVersion(source, root, notes);
  const nodes = readNodes(source, readArray(source, root, "nodes"), notes);
  const edges = readEdges(source, readArray(source, root, "edges"), nodes, notes);
  return { json: wireDocument(version, nodes, edges), doc: { version, nodes, edges }, notes };
}