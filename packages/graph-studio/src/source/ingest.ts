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
 */

export type NodeKind = "record" | "note" | "database" | "tag";
export type EdgeKind = "relation" | "tag" | "note_of" | "note_link" | "hierarchy";

export const NODE_KINDS: readonly NodeKind[] = ["record", "note", "database", "tag"];
export const EDGE_KINDS: readonly EdgeKind[] = ["relation", "tag", "note_of", "note_link", "hierarchy"];

export interface IngestNode {
  id: string;
  kind: NodeKind;
  database_id: string | null;
  source: string;
  label: string;
  group: string | null;
  weight: number;
  version: number;
  has_note: boolean;
  icon: string | null;
  /** The node's own tags, in document order. Optional on input, always present after. */
  tags?: readonly string[];
  /** Where the node sits in the source tree. Optional on input, always present after. */
  path?: string;
}

export interface IngestEdge {
  id: string;
  source: string;
  target: string;
  kind: EdgeKind;
  label: string;
  strength: number;
  directed: boolean;
  record_id: string | null;
  child_first: boolean;
}

export interface IngestDoc {
  version: 1;
  nodes: IngestNode[];
  edges: IngestEdge[];
}

/** A document the contract would refuse, named by where it came from. */
export class IngestRefusal extends Error {
  constructor(source: string, message: string) {
    super(`${source}: ${message}`);
    this.name = "IngestRefusal";
  }
}

export interface Normalised {
  /** The ingest text to hand `gm_build`, member order per the contract. */
  readonly json: string;
  /** The contract's shape plus `tags` and `path` per node. */
  readonly doc: IngestDoc;
  /** Every default filled and annotation dropped, in the order they happened. */
  readonly notes: readonly string[];
}

type Record_ = Record<string, unknown>;

// The ten the wasm contract names. `tags` and `path` are the two it does not name but
// the studio needs for `tag:#x` and `path:` queries, so they are kept rather than
// dropped — and they are NOT defaulted members: a document that never mentioned them is
// complete as it stands.
// Ponytail: `gm_build` refuses an unknown member, so the wire text (`json`) still
// carries only the ten; the two ride in `doc`, which is what `metaOf` reads. Failing
// input: a document whose nodes carry `tags`, built against the contract shape. It errs
// toward dropping tags at the motor, never toward a failed build; escape hatch: the
// query reads `doc`, so nothing here depends on the wire.
const CONTRACT_MEMBERS = [
  "id", "kind", "database_id", "source", "label", "group",
  "weight", "version", "has_note", "icon",
] as const;
const NODE_MEMBERS = [...CONTRACT_MEMBERS, "tags", "path"] as const;

const EDGE_MEMBERS = [
  "id", "source", "target", "kind", "label", "strength",
  "directed", "record_id", "child_first",
] as const;

function isObject(value: unknown): value is Record_ {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireString(source: string, at: string, record: Record_, key: string): string {
  const value = record[key];
  if (typeof value !== "string" || value === "") {
    throw new IngestRefusal(source, `${at} has no string \`${key}\``);
  }
  return value;
}

function optionalString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

// Ponytail: `tags` must be a list of strings, or `tag:#x` would have to guess at a
// number. A list holding one non-string is dropped whole rather than kept in part:
// half a tag list is a lie about the document. It errs toward a document losing all its
// tags. Escape hatch: the note names the member, so the user sees what was dropped.
function stringList(value: unknown): readonly string[] {
  return isStringList(value) ? [...value] : [];
}

function isStringList(value: unknown): value is readonly string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function optionalNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function optionalBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function parseDocument(source: string, text: string): Record_ {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new IngestRefusal(source, "not JSON");
  }
  if (!isObject(parsed)) throw new IngestRefusal(source, "the document is not a JSON object");
  return parsed;
}

function readVersion(source: string, root: Record_, notes: string[]): 1 {
  const version = root["version"];
  if (version === undefined) {
    notes.push("no `version` member: assumed 1");
    return 1;
  }
  if (version !== 1) throw new IngestRefusal(source, `unsupported version ${JSON.stringify(version)}`);
  return 1;
}

function readArray(source: string, root: Record_, key: "nodes" | "edges"): unknown[] {
  const value = root[key];
  if (!Array.isArray(value)) throw new IngestRefusal(source, `no \`${key}\` array`);
  return value;
}

/** Members outside the contract's set are annotations (an `about`, a `role`, an
 *  `x`): dropped, and named in the notes. */
function noteDropped(notes: string[], at: string, record: Record_, keep: readonly string[]): void {
  for (const key of Object.keys(record)) {
    if (!keep.includes(key)) notes.push(`dropped annotation \`${at}.${key}\``);
  }
}

function nodeKindOf(source: string, at: string, value: unknown): NodeKind {
  if (value === undefined) return "record";
  const kind = NODE_KINDS.find((candidate) => candidate === value);
  if (kind === undefined) {
    throw new IngestRefusal(source, `${at}.kind ${JSON.stringify(value)} is not a node kind`);
  }
  return kind;
}

/** The analysis fixtures name a node by its id alone. */
function nodeRecordOf(source: string, at: string, value: unknown, notes: string[]): Record_ {
  if (isObject(value)) return value;
  if (typeof value !== "string") throw new IngestRefusal(source, `${at} is neither an object nor an id`);
  if (value !== "") notes.push(`node ${JSON.stringify(value)} was given as a bare id`);
  return { id: value };
}

function readNode(source: string, at: string, given: unknown, notes: string[]): IngestNode {
  const value = nodeRecordOf(source, at, given, notes);
  noteDropped(notes, at, value, NODE_MEMBERS);
  const kind = nodeKindOf(source, at, value.kind);
  const id = requireString(source, at, value, "id");
  const filled = CONTRACT_MEMBERS.filter((member) => value[member] === undefined).length;
  if (filled > 0) notes.push(`defaulted ${filled} member(s) on node "${id}"`);
  if (value.tags !== undefined && !isStringList(value.tags)) notes.push(`dropped \`${at}.tags\`: not a list of strings`);
  return {
    id, kind,
    database_id: optionalString(value.database_id),
    source: optionalString(value.source) ?? "file",
    label: optionalString(value.label) ?? id,
    group: optionalString(value.group),
    weight: optionalNumber(value.weight, 0.5),
    version: optionalNumber(value.version, 0),
    has_note: optionalBoolean(value.has_note, kind === "note"),
    icon: optionalString(value.icon),
    tags: stringList(value.tags),
    path: optionalString(value.path) ?? "",
  };
}

/** The node as the wasm contract names it: the ten, in the contract's own order. */
function wireNode(node: IngestNode): Record_ {
  return Object.fromEntries(CONTRACT_MEMBERS.map((member) => [member, node[member]]));
}

/** A fixture's `type` is its wire spelling of the kind; only the hierarchy spellings
 *  are honoured here, and anything else keeps the default. */
function edgeKindOf(source: string, at: string, record: Record_, notes: string[]): EdgeKind {
  if (record.kind !== undefined) {
    const kind = EDGE_KINDS.find((candidate) => candidate === record.kind);
    if (kind === undefined) {
      throw new IngestRefusal(source, `${at}.kind ${JSON.stringify(record.kind)} is not an edge kind`);
    }
    return kind;
  }
  const spelling = optionalString(record.type) ?? "";
  const wireType = spelling.toLowerCase();
  if (wireType.includes("hierarchy") || wireType === "parent" || wireType === "child_of") {
    notes.push(`mapped edge \`type\` "${spelling}" to kind "hierarchy"`);
    return "hierarchy";
  }
  return "relation";
}

function readEdge(source: string, at: string, value: unknown, notes: string[]): IngestEdge {
  if (!isObject(value)) throw new IngestRefusal(source, `${at} is not an object`);
  noteDropped(notes, at, value, [...EDGE_MEMBERS, "type"]);
  const id = requireString(source, at, value, "id");
  const from = requireString(source, at, value, "source");
  const to = requireString(source, at, value, "target");
  const kind = edgeKindOf(source, at, value, notes);
  const filled = EDGE_MEMBERS.filter((member) => value[member] === undefined).length;
  if (filled > 0) notes.push(`defaulted ${filled} member(s) on edge "${id}"`);
  return {
    id, source: from, target: to, kind,
    label: optionalString(value.label) ?? kind,
    strength: optionalNumber(value.strength, 0.5),
    directed: optionalBoolean(value.directed, kind === "hierarchy"),
    record_id: optionalString(value.record_id),
    child_first: optionalBoolean(value.child_first, false),
  };
}

function readNodes(source: string, values: unknown[], notes: string[]): IngestNode[] {
  const nodes: IngestNode[] = [];
  const seen = new Set<string>();
  for (const [i, value] of values.entries()) {
    const node = readNode(source, `nodes[${i}]`, value, notes);
    if (seen.has(node.id)) throw new IngestRefusal(source, `duplicate node id ${JSON.stringify(node.id)}`);
    seen.add(node.id);
    nodes.push(node);
  }
  return nodes;
}

function readEdges(source: string, values: unknown[], nodes: readonly IngestNode[], notes: string[]): IngestEdge[] {
  const ids = new Set(nodes.map((node) => node.id));
  const edges: IngestEdge[] = [];
  const seen = new Set<string>();
  for (const [i, value] of values.entries()) {
    const edge = readEdge(source, `edges[${i}]`, value, notes);
    if (seen.has(edge.id)) throw new IngestRefusal(source, `duplicate edge id ${JSON.stringify(edge.id)}`);
    seen.add(edge.id);
    for (const [end, id] of [["source", edge.source], ["target", edge.target]] as const) {
      if (!ids.has(id)) {
        throw new IngestRefusal(source, `edge ${JSON.stringify(edge.id)} ${end}s ${JSON.stringify(id)}, which is not a node`);
      }
    }
    edges.push(edge);
  }
  return edges;
}

/** Normalise `text` into the ingest document `gm_build` reads, or refuse it with
 *  the reason and the file it came from. */
export function normaliseIngest(text: string, source: string): Normalised {
  const notes: string[] = [];
  const root = parseDocument(source, text);
  for (const key of Object.keys(root)) {
    if (key !== "version" && key !== "nodes" && key !== "edges") notes.push(`dropped annotation \`${key}\``);
  }
  const version = readVersion(source, root, notes);
  const nodes = readNodes(source, readArray(source, root, "nodes"), notes);
  const edges = readEdges(source, readArray(source, root, "edges"), nodes, notes);
  const doc: IngestDoc = { version, nodes, edges };
  const wire = { version, nodes: nodes.map(wireNode), edges };
  return { json: JSON.stringify(wire), doc, notes };
}
