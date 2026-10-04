/**
 * A generated graph, as the binary columnar document (`docs/contract/ingest-columns.md`).
 *
 * This is the path the studio opens a synthetic source through. It is `syntheticRecords` and
 * `JSON.stringify` removed from the way: no JSON text is built and no `IngestEdge` is made,
 * because both consumers of `synthetic-draw.ts` read the same draws and this one wants the
 * columns those draws already are.
 *
 * Two things the general encoder (`crates/graph-sdk-js/src/columns.ts`) pays for and this does
 * not. It deduped every string through a `Map` — 23 million probes over roughly seven million
 * distinct strings at 1M nodes, measured at 6.4 s in `docs/measurements/perf-open-columns.md` —
 * and the contract allows two table entries to hold the same bytes, so this table repeats
 * instead. It also resolved every endpoint by id; the generator drew a row for every endpoint
 * it made, so the endpoints are rows already and there is no id lookup at all.
 *
 * **Caveat:** the blob carries a repeat for every repeated value (`"studio"`, `db-3`, a shared
 * label), so it is larger than a deduped table's would be. The decoder's arena interns by
 * content, so the repeats cost bytes in transit and nothing downstream.
 *
 * # Panics
 *
 * None beyond the draws': a spec outside 2..MAX_NODES or 0..MAX_DEGREE is clamped exactly as
 * `syntheticRecords` clamps it, and both read the same `drawGraph`.
 */
import type { IngestNode } from "./ingest.ts";
import {
  DATABASE_COUNT,
  EDGE_KIND_NAMES,
  EDGE_STRENGTH,
  GROUP_NAMES,
  NODE_KIND_NAMES,
  SOURCE_NAME,
  applyDegreeWeights,
  drawGraph,
} from "./synthetic-draw.ts";
import type { Drawn, SyntheticSpec } from "./synthetic-draw.ts";

/** The SDK's `ColumnRows`, declared here rather than imported: `source/` may not import the
 *  motor's SDK (`app/eslint.config.js`), and TypeScript's structural typing means this *is* its
 *  shape — nothing here has to be kept in step but the field names. */
export interface ColumnRowsLike {
  readonly strings: readonly string[];
  readonly nodeCells: Uint32Array;
  readonly edgeCells: Uint32Array;
  readonly weights: Float64Array;
  readonly versions: Float64Array;
  readonly strengths: Float64Array;
}

export interface SyntheticColumns {
  /** The document, ready for the SDK's `assembleColumns`. */
  readonly rows: ColumnRowsLike;
  /** The same `IngestNode[]` `syntheticRecords` makes: the studio UI reads weights and kinds
   *  off it, and it is already built as a by-product of the draws. */
  readonly nodes: IngestNode[];
  readonly edgeCount: number;
}

/** `u32::MAX` in an optional column: the field is absent, exactly as the JSON reader's `null`. */
const ABSENT = 0xffff_ffff;
/** The `u32` columns on each side, per the contract's table. */
const NODE_COLUMNS = 8;
const EDGE_COLUMNS = 8;

// The fixed head, in index order. Every value the columns name repeatedly lives here, so the
// per-row part of the table is only the ids and labels — the two fields that are unique by
// construction. The bases are derived from the lists themselves, so reordering a list moves
// its head entries and nothing else.
const KIND_BASE = 0;
const EDGE_KIND_BASE = KIND_BASE + NODE_KIND_NAMES.length;
const GROUP_BASE = EDGE_KIND_BASE + EDGE_KIND_NAMES.length;
const DATABASE_BASE = GROUP_BASE + GROUP_NAMES.length;
const SOURCE_INDEX = DATABASE_BASE + DATABASE_COUNT;
const HEAD = SOURCE_INDEX + 1;

/** Read off the same lists the head is built from, so a reorder cannot desynchronise them. */
const NOTE_KIND = NODE_KIND_NAMES.indexOf("note");
const HIERARCHY_KIND = EDGE_KIND_NAMES.indexOf("hierarchy");

/** The table: the fixed head, then each node's id and label, then each edge's id. The node
 *  strings are the very objects `nodes` holds — a second copy per field would be the cost this
 *  path exists to avoid. */
function table(nodes: readonly IngestNode[], count: number, edges: number): string[] {
  // Sized up front and then written by index: a `push` per row would be the same number of
  // stores plus the growth, and at 1M nodes the array is three million entries.
  const strings = Array.from({ length: HEAD + 2 * count + edges }, (): string => "");
  NODE_KIND_NAMES.forEach((name, i) => { strings[i] = name; });
  EDGE_KIND_NAMES.forEach((name, i) => { strings[EDGE_KIND_BASE + i] = name; });
  GROUP_NAMES.forEach((name, i) => { strings[GROUP_BASE + i] = name; });
  for (let d = 0; d < DATABASE_COUNT; d += 1) strings[DATABASE_BASE + d] = `db-${d}`;
  strings[SOURCE_INDEX] = SOURCE_NAME;
  for (let r = 0; r < count; r += 1) {
    strings[HEAD + 2 * r] = nodes[r]?.id ?? `n-${r}`;
    strings[HEAD + 2 * r + 1] = nodes[r]?.label ?? "";
  }
  for (let j = 0; j < edges; j += 1) strings[HEAD + 2 * count + j] = `e-${j}`;
  return strings;
}

/** The eight node columns, one sequential pass each. `has_note` is the kind the draw settled
 *  on — including `vault`'s override of a topic's first node to `database`. */
function nodeColumns(rows: ColumnRowsLike, drawn: Drawn): void {
  const cells = rows.nodeCells;
  const count = rows.weights.length;
  const kinds = drawn.kinds;
  for (let r = 0; r < count; r += 1) cells[r] = HEAD + 2 * r;
  for (let r = 0; r < count; r += 1) cells[count + r] = KIND_BASE + (kinds[r] ?? 0);
  for (let r = 0; r < count; r += 1) cells[2 * count + r] = DATABASE_BASE + (r % DATABASE_COUNT);
  cells.fill(SOURCE_INDEX, 3 * count, 4 * count);
  for (let r = 0; r < count; r += 1) cells[4 * count + r] = HEAD + 2 * r + 1;
  for (let r = 0; r < count; r += 1) cells[5 * count + r] = GROUP_BASE + (drawn.groups[r] ?? 0);
  cells.fill(ABSENT, 6 * count, 7 * count);
  for (let r = 0; r < count; r += 1) {
    cells[7 * count + r] = (kinds[r] ?? 0) === NOTE_KIND ? 1 : 0;
  }
}

/** The eight edge columns. `source` and `target` are the draws' own arrays, copied straight in:
 *  an endpoint is a node row the generator made, so there is nothing to resolve. The label is
 *  the kind, because that is what the JSON record's label holds. */
function edgeColumns(rows: ColumnRowsLike, drawn: Drawn): void {
  const cells = rows.edgeCells;
  const count = rows.strengths.length;
  const kinds = drawn.edgeKinds;
  const first = HEAD + 2 * rows.weights.length;
  for (let j = 0; j < count; j += 1) cells[j] = first + j;
  cells.set(drawn.from, count);
  cells.set(drawn.to, 2 * count);
  for (let j = 0; j < count; j += 1) cells[3 * count + j] = EDGE_KIND_BASE + (kinds[j] ?? 0);
  cells.set(cells.subarray(3 * count, 4 * count), 4 * count);
  cells.fill(ABSENT, 5 * count, 6 * count);
  for (let j = 0; j < count; j += 1) {
    cells[6 * count + j] = (kinds[j] ?? 0) === HIERARCHY_KIND ? 1 : 0;
  }
  cells.fill(0, 7 * count);
}

/** The three `f64` columns. The weights come off the nodes the weight pass just wrote, so the
 *  pass is `applyDegreeWeights` and not a second copy of it; the other two are constants the
 *  generator has never varied. */
function floatColumns(rows: ColumnRowsLike, nodes: readonly IngestNode[]): void {
  for (let r = 0; r < rows.weights.length; r += 1) rows.weights[r] = nodes[r]?.weight ?? 0;
  rows.versions.fill(0);
  rows.strengths.fill(EDGE_STRENGTH);
}

/** One spec, as the columns `gm_build_columns` reads. O(nodes + edges) time, one pass per
 *  column, no map and no per-row object. */
export function syntheticColumns(spec: SyntheticSpec): SyntheticColumns {
  const drawn = drawGraph(spec);
  applyDegreeWeights(drawn.nodes, drawn.counts);
  const count = drawn.nodes.length;
  const edges = drawn.edgeCount;
  const rows: ColumnRowsLike = {
    strings: table(drawn.nodes, count, edges),
    nodeCells: new Uint32Array(NODE_COLUMNS * count),
    edgeCells: new Uint32Array(EDGE_COLUMNS * edges),
    weights: new Float64Array(count),
    versions: new Float64Array(count),
    strengths: new Float64Array(edges),
  };
  nodeColumns(rows, drawn);
  edgeColumns(rows, drawn);
  floatColumns(rows, drawn.nodes);
  return { rows, nodes: drawn.nodes, edgeCount: edges };
}
