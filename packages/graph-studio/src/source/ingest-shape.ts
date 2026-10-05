/**
 * The provisional ingest shape, as types: what a node is, what an edge is, what a document is,
 * and the refusal the readers throw. Its own module because both halves of the normaliser read
 * these — `ingest.ts` reads the document around its records, `ingest-record.ts` reads one record —
 * and a pair of modules that import each other is a cycle, not a design.
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