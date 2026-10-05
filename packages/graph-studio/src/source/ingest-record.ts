/**
 * One record, read: the ten members the wasm contract names for a node, the nine for an edge,
 * and the wire projection of a node. Split out of `ingest.ts`, which reads the document around
 * these, so that file stays inside the house's line limit. The types both halves read are
 * `ingest-shape.ts`; the three are a chain, not a cycle.
 *
 * Everything here runs once per node and once per edge of the document being opened, so the
 * shapes it allocates per record are the ones that add up: at 85 928 nodes and 107 694 edges
 * (the 62 MB git document, `docs/measurements/studio-open-large.md`) an `Object.keys` copy, a
 * `CONTRACT_MEMBERS.filter`, a `CONTRACT_MEMBERS.map` and a `[...EDGE_MEMBERS, "type"]` per
 * record were a measured part of the normaliser's 594 ms. The forms below allocate the record
 * itself and nothing else, and the wire text is byte-identical — `tests/ingest-hot.test.ts` pins it.
 */
import {
  EDGE_KINDS, type EdgeKind, type IngestEdge, type IngestNode, IngestRefusal, NODE_KINDS, type NodeKind,
} from "./ingest-shape.ts";

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

// The membership test the hot path makes once per node and once per edge: a `Set`, hoisted out
// of the loops, where the code before it built the key list per record and scanned it linearly.
const NODE_SET: ReadonlySet<string> = new Set(NODE_MEMBERS);
const EDGE_SET: ReadonlySet<string> = new Set([...EDGE_MEMBERS, "type"]);

export function isObject(value: unknown): value is Record_ {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function requireString(source: string, at: string, record: Record_, key: string): string {
  const value = record[key];
  if (typeof value !== "string" || value === "") {
    throw new IngestRefusal(source, `${at} has no string \`${key}\``);
  }
  return value;
}

export function optionalString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

// Ponytail: `tags` must be a list of strings, or `tag:#x` would have to guess at a
// number. A list holding one non-string is dropped whole rather than kept in part:
// half a tag list is a lie about the document. It errs toward a document losing all its
// tags. Escape hatch: the note names the member, so the user sees what was dropped.
export function stringList(value: unknown): readonly string[] {
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

/** How many of `members` the record leaves undefined: the count the "defaulted" note carries. */
function missingOf(record: Record_, members: readonly string[]): number {
  let missing = 0;
  for (const member of members) {
    if (record[member] === undefined) missing += 1;
  }
  return missing;
}

/** Members outside the contract's set are annotations (an `about`, a `role`, an
 *  `x`): dropped, and named in the notes. `for...in` rather than `Object.keys`, so the keys are
 *  not copied into a fresh array per node and per edge. */
function noteDropped(notes: string[], at: string, record: Record_, keep: ReadonlySet<string>): void {
  for (const key in record) {
    if (!keep.has(key)) notes.push(`dropped annotation \`${at}.${key}\``);
  }
}

function nodeKindOf(source: string, at: string, value: unknown): NodeKind {
  if (value === undefined) return "record";
  if (typeof value === "string") {
    for (const kind of NODE_KINDS) {
      if (kind === value) return kind;
    }
  }
  throw new IngestRefusal(source, `${at}.kind ${JSON.stringify(value)} is not a node kind`);
}

/** The analysis fixtures name a node by its id alone. */
function nodeRecordOf(source: string, at: string, value: unknown, notes: string[]): Record_ {
  if (isObject(value)) return value;
  if (typeof value !== "string") throw new IngestRefusal(source, `${at} is neither an object nor an id`);
  if (value !== "") notes.push(`node ${JSON.stringify(value)} was given as a bare id`);
  return { id: value };
}

/** One node, filled out to the contract's shape, with every default filled and annotation
 *  dropped noted, in the order they happened. */
export function readNode(source: string, at: string, given: unknown, notes: string[]): IngestNode {
  const value = nodeRecordOf(source, at, given, notes);
  noteDropped(notes, at, value, NODE_SET);
  const kind = nodeKindOf(source, at, value.kind);
  const id = requireString(source, at, value, "id");
  const filled = missingOf(value, CONTRACT_MEMBERS);
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

/**
 * The node as the wasm contract names it: the ten, in the contract's own order, as an object
 * literal. `Object.fromEntries(CONTRACT_MEMBERS.map(...))` spelled the same object as two
 * throwaway arrays per node — 172 856 of them on the 62 MB document, and 94 ms of the normaliser
 * (`docs/measurements/studio-open-large.md`). Same ten keys, same order, same bytes.
 */
export function wireNode(node: IngestNode): Record_ {
  return {
    id: node.id, kind: node.kind, database_id: node.database_id, source: node.source,
    label: node.label, group: node.group, weight: node.weight, version: node.version,
    has_note: node.has_note, icon: node.icon,
  };
}

/** A fixture's `type` is its wire spelling of the kind; only the hierarchy spellings
 *  are honoured here, and anything else keeps the default. */
function edgeKindOf(source: string, at: string, record: Record_, notes: string[]): EdgeKind {
  if (record.kind !== undefined) {
    if (typeof record.kind === "string") {
      for (const kind of EDGE_KINDS) {
        if (kind === record.kind) return kind;
      }
    }
    throw new IngestRefusal(source, `${at}.kind ${JSON.stringify(record.kind)} is not an edge kind`);
  }
  const spelling = optionalString(record.type) ?? "";
  // The three hierarchy spellings are tested on the spelling as it stands, so the common edge —
  // one that names `kind`, or a `type` that already is one of them — never pays for a lowercased
  // copy: 107 694 of them on the 62 MB document.
  if (hierarchySpelling(spelling)) {
    notes.push(`mapped edge \`type\` "${spelling}" to kind "hierarchy"`);
    return "hierarchy";
  }
  return hierarchySpelling(spelling.toLowerCase()) ? "hierarchy" : "relation";
}

/** Whether a `type` spelling is one of the three that name a hierarchy. */
function hierarchySpelling(wireType: string): boolean {
  return wireType.includes("hierarchy") || wireType === "parent" || wireType === "child_of";
}

/** One edge, filled out to the contract's shape, with every default filled and annotation
 *  dropped noted, in the order they happened. */
export function readEdge(source: string, at: string, value: unknown, notes: string[]): IngestEdge {
  if (!isObject(value)) throw new IngestRefusal(source, `${at} is not an object`);
  noteDropped(notes, at, value, EDGE_SET);
  const id = requireString(source, at, value, "id");
  const from = requireString(source, at, value, "source");
  const to = requireString(source, at, value, "target");
  const kind = edgeKindOf(source, at, value, notes);
  const filled = missingOf(value, EDGE_MEMBERS);
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